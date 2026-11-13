//! A ligação de uma cópia com o seu Anfitrião: reconecta sozinha, envia o que
//! está pendente, recebe o que mudou e repassa os comandos que só ele executa.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use finance_core::{Clock, Connection, HostCommand, Household, MemberId, PROTOCOL_VERSION};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, watch};

use crate::client::{Client, ClientError};
use crate::identity::Pin;
use crate::pairing::{HOST_ADDRESS, HOST_FINGERPRINT, hello, pull, token_key};
use crate::protocol::{PROTOCOL_MISMATCH, Request, Response, UNAUTHENTICATED};

pub struct LinkConfig {
    /// O arquivo da cópia local.
    pub db: PathBuf,
    pub clock: Arc<dyn Clock>,
    /// O membro em nome de quem esta cópia fala.
    pub member: MemberId,
    /// Onde o Anfitrião está agora, se alguém sabe. É consultado a cada
    /// tentativa de conexão, porque o endereço dele muda de um dia para o outro.
    pub locate: Arc<dyn Fn() -> Option<SocketAddr> + Send + Sync>,
    pub on_event: Arc<dyn Fn(LinkEvent) + Send + Sync>,
    pub protocol: u32,
    pub retry_every: Duration,
}

impl LinkConfig {
    pub fn new(
        db: PathBuf,
        clock: Arc<dyn Clock>,
        member: MemberId,
        locate: Arc<dyn Fn() -> Option<SocketAddr> + Send + Sync>,
    ) -> LinkConfig {
        LinkConfig {
            db,
            clock,
            member,
            locate,
            on_event: Arc::new(|_| {}),
            protocol: PROTOCOL_VERSION,
            retry_every: Duration::from_secs(3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// Sem contato com o Anfitrião; as alterações locais esperam.
    Offline,
    Online,
    /// O Anfitrião está em outra versão do app: nada sincroniza até atualizar.
    ProtocolMismatch,
    /// O Anfitrião não reconhece mais este membro neste computador.
    Unauthorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkEvent {
    State(LinkState),
    /// A cópia local mudou ou ficou em dia: vale reler os dados.
    Synced,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum LinkError {
    #[error("Esta ação precisa de conexão com o Anfitrião.")]
    Offline,
    #[error("{message}")]
    Host { code: String, message: String },
}

impl LinkError {
    pub fn code(&self) -> &str {
        match self {
            LinkError::Offline => "needs_host",
            LinkError::Host { code, .. } => code,
        }
    }
}

enum Inbox {
    Poke,
    Command(HostCommand, oneshot::Sender<Result<Value, LinkError>>),
}

/// A ligação em andamento. Ao ser descartada, a sincronização para.
pub struct ReplicaLink {
    inbox: mpsc::UnboundedSender<Inbox>,
    state: watch::Receiver<LinkState>,
}

impl ReplicaLink {
    pub fn start(config: LinkConfig) -> ReplicaLink {
        let (inbox, receiver) = mpsc::unbounded_channel();
        let (state_tx, state) = watch::channel(LinkState::Offline);
        tokio::spawn(run(config, receiver, state_tx));
        ReplicaLink { inbox, state }
    }

    /// Avisa que há alteração local a enviar.
    pub fn poke(&self) {
        let _ = self.inbox.send(Inbox::Poke);
    }

    /// Executa no Anfitrião o que só ele pode fazer e espera o resultado
    /// chegar à cópia local.
    pub async fn command(&self, command: HostCommand) -> Result<Value, LinkError> {
        let (reply, response) = oneshot::channel();
        self.inbox
            .send(Inbox::Command(command, reply))
            .map_err(|_| LinkError::Offline)?;
        response.await.unwrap_or(Err(LinkError::Offline))
    }

    pub fn state(&self) -> LinkState {
        *self.state.borrow()
    }
}

/// Por que uma conexão com o Anfitrião terminou.
enum Ended {
    Disconnected,
    ProtocolMismatch,
    Unauthorized,
    /// Ninguém mais usa esta ligação.
    Stopped,
}

impl From<ClientError> for Ended {
    fn from(error: ClientError) -> Ended {
        match error.code() {
            Some(PROTOCOL_MISMATCH) => Ended::ProtocolMismatch,
            Some(UNAUTHENTICATED) => Ended::Unauthorized,
            _ => Ended::Disconnected,
        }
    }
}

impl From<finance_core::Error> for Ended {
    fn from(_: finance_core::Error) -> Ended {
        Ended::Disconnected
    }
}

async fn run(
    config: LinkConfig,
    mut inbox: mpsc::UnboundedReceiver<Inbox>,
    state: watch::Sender<LinkState>,
) {
    let opened = Connection::open(&config.db)
        .map_err(finance_core::Error::from)
        .and_then(|conn| Household::open(conn, config.clock.clone()));
    let Ok(mut household) = opened else { return };
    let publish = |next: LinkState| {
        if state.send_replace(next) != next {
            (config.on_event)(LinkEvent::State(next));
        }
    };

    loop {
        let ended = session(&config, &mut household, &mut inbox, &publish).await;
        let (next, wait) = match ended {
            Ended::Stopped => return,
            Ended::Disconnected => (LinkState::Offline, config.retry_every),
            // Só uma atualização ou um novo login resolvem: não adianta insistir depressa.
            Ended::ProtocolMismatch => (LinkState::ProtocolMismatch, config.retry_every * 5),
            Ended::Unauthorized => (LinkState::Unauthorized, config.retry_every * 5),
        };
        publish(next);

        // Enquanto espera para tentar de novo, quem pede algo ao Anfitrião
        // fica sabendo na hora que ele está fora de alcance.
        let retry = tokio::time::sleep(wait);
        tokio::pin!(retry);
        loop {
            tokio::select! {
                _ = &mut retry => break,
                message = inbox.recv() => match message {
                    None => return,
                    Some(Inbox::Poke) => {}
                    Some(Inbox::Command(_, reply)) => {
                        let _ = reply.send(Err(LinkError::Offline));
                    }
                },
            }
        }
    }
}

/// Uma conexão com o Anfitrião, do aperto de mão até ela cair.
async fn session(
    config: &LinkConfig,
    household: &mut Household,
    inbox: &mut mpsc::UnboundedReceiver<Inbox>,
    publish: &impl Fn(LinkState),
) -> Ended {
    let Some(addr) = (config.locate)() else {
        return Ended::Disconnected;
    };
    let Ok(Some(fingerprint)) = household.local(HOST_FINGERPRINT) else {
        return Ended::Disconnected;
    };
    let Ok(Some(token)) = household.local(&token_key(config.member)) else {
        return Ended::Unauthorized;
    };

    let mut client = match Client::connect(addr, Pin::Fingerprint(fingerprint)).await {
        Ok(client) => client,
        Err(error) => return error.into(),
    };
    if let Err(error) = hello(&mut client, config.protocol).await {
        return error.into();
    }
    if let Err(error) = client.request(Request::Resume { token }).await {
        return error.into();
    }
    let _ = household.set_local(HOST_ADDRESS, &addr.to_string());
    publish(LinkState::Online);

    if let Err(ended) = sync_once(&mut client, household, config).await {
        return ended;
    }
    loop {
        enum Woke {
            Changed(Result<i64, ClientError>),
            Inbox(Option<Inbox>),
        }
        let woke = tokio::select! {
            changed = client.changed() => Woke::Changed(changed),
            message = inbox.recv() => Woke::Inbox(message),
        };
        match woke {
            Woke::Inbox(None) => return Ended::Stopped,
            Woke::Changed(Err(error)) => return error.into(),
            Woke::Changed(Ok(_)) | Woke::Inbox(Some(Inbox::Poke)) => {}
            Woke::Inbox(Some(Inbox::Command(command, reply))) => {
                let result = client.request(Request::Command { command }).await;
                let lost = matches!(
                    result,
                    Err(ClientError::Unreachable | ClientError::WrongHost)
                );
                // Recebe o efeito do comando antes de responder, para que quem
                // pediu já encontre o resultado na cópia local.
                let synced = sync_once(&mut client, household, config).await;
                let _ = reply.send(match result {
                    Ok(Response::Done { value }) => Ok(value),
                    Ok(_) => Err(LinkError::Offline),
                    Err(ClientError::Host(error)) => Err(LinkError::Host {
                        code: error.code,
                        message: error.message,
                    }),
                    Err(_) => Err(LinkError::Offline),
                });
                match synced {
                    Err(ended) => return ended,
                    Ok(()) if lost => return Ended::Disconnected,
                    Ok(()) => continue,
                }
            }
        }
        if let Err(ended) = sync_once(&mut client, household, config).await {
            return ended;
        }
    }
}

/// Envia as alterações locais e depois recebe as do Anfitrião, nessa ordem:
/// assim o que acabou de ser aceito já volta com a versão nova.
async fn sync_once(
    client: &mut Client,
    household: &mut Household,
    config: &LinkConfig,
) -> Result<(), Ended> {
    let pending = household.pending_changes()?;
    if !pending.is_empty() {
        let changes = pending.iter().map(|p| p.change.clone()).collect();
        match client.request(Request::Push { changes }).await? {
            Response::Pushed { outcomes } => household.record_push(&pending, &outcomes)?,
            _ => return Err(Ended::Disconnected),
        }
    }
    let changes = pull(client, household.cursor()?).await?;
    household.apply_changes(&changes)?;
    (config.on_event)(LinkEvent::Synced);
    Ok(())
}
