//! O primeiro contato de um computador com um Anfitrião: conferir quem ele é,
//! entrar com um Convite e trazer a cópia inicial da residência.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use finance_core::{
    ChangeSet, Clock, Connection, Household, HouseholdInfo, Member, MemberId, PROTOCOL_VERSION,
};

use crate::client::{Client, ClientError};
use crate::identity::Pin;
use crate::protocol::{PROTOCOL_MISMATCH, Request, Response, WireError};

pub(crate) const HOST_FINGERPRINT: &str = "host_fingerprint";
pub(crate) const HOST_ADDRESS: &str = "host_address";

pub(crate) fn token_key(member: MemberId) -> String {
    format!("token:{member}")
}

/// O que um Anfitrião diz de si antes de qualquer credencial.
#[derive(Debug, Clone)]
pub struct HostInfo {
    pub household: HouseholdInfo,
    /// A impressão digital do certificado dele, a conferir e fixar.
    pub fingerprint: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Core(#[from] finance_core::Error),
}

impl PairingError {
    pub fn code(&self) -> &str {
        match self {
            PairingError::Client(error) => error.code().unwrap_or("unreachable"),
            PairingError::Core(error) => error.code(),
        }
    }
}

/// Pergunta a quem atende em `addr` que residência hospeda. Não confia em
/// nada ainda: só relata a impressão digital apresentada.
pub async fn inspect(addr: SocketAddr) -> Result<HostInfo, PairingError> {
    let mut client = Client::connect(addr, Pin::TrustFirst).await?;
    let household = hello(&mut client, PROTOCOL_VERSION).await?;
    Ok(HostInfo {
        household,
        fingerprint: client.fingerprint,
    })
}

/// Entra na residência com um Convite e cria em `db` a cópia local dela,
/// pareada com o Anfitrião de impressão digital `fingerprint`.
pub async fn join(
    addr: SocketAddr,
    fingerprint: &str,
    code: &str,
    username: &str,
    password: &str,
    db: &Path,
    clock: Arc<dyn Clock>,
) -> Result<Member, PairingError> {
    let mut client = Client::connect(addr, Pin::Fingerprint(fingerprint.to_owned())).await?;
    hello(&mut client, PROTOCOL_VERSION).await?;
    let (token, member) = session(
        &mut client,
        Request::Join {
            code: code.to_owned(),
            username: username.to_owned(),
            password: password.to_owned(),
        },
    )
    .await?;
    let changes = pull(&mut client, 0).await?;

    let created = (|| {
        let conn = Connection::open(db).map_err(finance_core::Error::from)?;
        let household = Household::create_replica(conn, clock)?;
        household.apply_changes(&changes)?;
        household.set_local(HOST_FINGERPRINT, fingerprint)?;
        household.set_local(HOST_ADDRESS, &addr.to_string())?;
        household.set_local(&token_key(member.id), &token)
    })();
    if let Err(error) = created {
        // Uma cópia pela metade não pode ficar para trás como se fosse uma residência.
        let _ = std::fs::remove_file(db);
        return Err(error.into());
    }
    Ok(member)
}

/// Obtém, para um membro que já existe, a credencial com que esta cópia fala
/// em nome dele. É o que acontece quando alguém entra pela primeira vez num
/// computador pareado por outra pessoa.
pub async fn sign_in(
    addr: SocketAddr,
    db: &Path,
    clock: Arc<dyn Clock>,
    username: &str,
    password: &str,
) -> Result<Member, PairingError> {
    let fingerprint = {
        let conn = Connection::open(db).map_err(finance_core::Error::from)?;
        Household::open(conn, clock.clone())?
            .local(HOST_FINGERPRINT)?
            .ok_or(finance_core::Error::NeedsHost)?
    };
    let mut client = Client::connect(addr, Pin::Fingerprint(fingerprint)).await?;
    hello(&mut client, PROTOCOL_VERSION).await?;
    let (token, member) = session(
        &mut client,
        Request::Login {
            username: username.to_owned(),
            password: password.to_owned(),
        },
    )
    .await?;

    let conn = Connection::open(db).map_err(finance_core::Error::from)?;
    Household::open(conn, clock)?.set_local(&token_key(member.id), &token)?;
    Ok(member)
}

/// Apresenta-se ao Anfitrião. Com versões de protocolo diferentes, falha com
/// o mesmo erro que ele daria a qualquer pedido seguinte.
pub(crate) async fn hello(
    client: &mut Client,
    protocol: u32,
) -> Result<HouseholdInfo, ClientError> {
    match client.request(Request::Hello { protocol }).await? {
        Response::Hello {
            protocol: theirs,
            household,
        } if theirs == protocol => Ok(household),
        Response::Hello { .. } => Err(ClientError::Host(WireError {
            code: PROTOCOL_MISMATCH.into(),
            message: "Este computador e o Anfitrião estão em versões diferentes do app. \
                      Atualize os dois para sincronizar."
                .into(),
        })),
        _ => Err(ClientError::Unreachable),
    }
}

pub(crate) async fn pull(client: &mut Client, since: i64) -> Result<ChangeSet, ClientError> {
    match client.request(Request::Pull { since }).await? {
        Response::Changes { changes } => Ok(changes),
        _ => Err(ClientError::Unreachable),
    }
}

async fn session(client: &mut Client, request: Request) -> Result<(String, Member), ClientError> {
    match client.request(request).await? {
        Response::Session { token, member } => Ok((token, member)),
        _ => Err(ClientError::Unreachable),
    }
}

/// O último endereço em que esta cópia falou com o seu Anfitrião.
pub fn stored_address(household: &Household) -> Option<SocketAddr> {
    household.local(HOST_ADDRESS).ok()??.parse().ok()
}

/// A impressão digital do Anfitrião com que esta cópia foi pareada.
pub fn stored_fingerprint(household: &Household) -> Option<String> {
    household.local(HOST_FINGERPRINT).ok()?
}

/// Se esta cópia já tem a credencial para falar em nome do membro.
pub fn has_credentials(household: &Household, member: MemberId) -> bool {
    matches!(household.local(&token_key(member)), Ok(Some(_)))
}
