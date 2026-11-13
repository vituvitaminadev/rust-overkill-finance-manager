//! O estado do app nesta máquina: quem está conectado, de quais residências
//! locais essa pessoa é membro e qual delas está aberta.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use finance_core::{
    Connection, Household, HouseholdId, HouseholdInfo, Member, MemberId, NewHousehold, NewUser,
    SystemClock,
};
use serde::Serialize;
use uuid::Uuid;

use crate::error::CommandError;
use crate::remember::{self, Remembered};

type Result<T> = std::result::Result<T, CommandError>;

pub struct Desk {
    dir: PathBuf,
    identity: Option<Identity>,
    open: Option<OpenHousehold>,
}

struct Identity {
    username: String,
    remember: bool,
    memberships: Vec<Membership>,
}

struct Membership {
    household: HouseholdInfo,
    path: PathBuf,
    member_id: MemberId,
}

struct OpenHousehold {
    household: Household,
    info: HouseholdInfo,
    path: PathBuf,
    member_id: MemberId,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Falso só na primeira abertura, quando ainda não existe residência nesta máquina.
    has_households: bool,
    username: Option<String>,
    households: Vec<HouseholdInfo>,
    current: Option<Current>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Current {
    household: HouseholdInfo,
    member: Member,
    /// Verdadeiro quando esta máquina tem só uma cópia e o Anfitrião é outra.
    replica: bool,
}

/// O que a rede precisa saber da residência aberta.
pub struct OpenDetails {
    pub path: PathBuf,
    pub household: HouseholdId,
    pub member: MemberId,
    pub replica: bool,
    /// Na cópia, o último endereço em que o Anfitrião foi encontrado.
    pub host_address: Option<SocketAddr>,
}

/// Uma cópia em que o membro conectado ainda não tem credencial para sincronizar.
pub struct MissingCredentials {
    pub path: PathBuf,
    pub household: HouseholdId,
    pub host_address: Option<SocketAddr>,
}

impl Desk {
    /// Abre a pasta de residências e retoma a sessão lembrada, se houver.
    pub fn new(dir: PathBuf) -> std::io::Result<Desk> {
        std::fs::create_dir_all(&dir)?;
        let mut desk = Desk {
            dir,
            identity: None,
            open: None,
        };
        if let Some(remembered) = remember::load() {
            desk.identity = desk.restore(remembered);
        }
        Ok(desk)
    }

    pub fn status(&self) -> Result<Status> {
        let current = match &self.open {
            Some(open) => Some(Current {
                household: open.info.clone(),
                member: open.household.member(open.member_id)?,
                replica: open.household.is_replica(),
            }),
            None => None,
        };
        Ok(Status {
            has_households: !self.household_files().is_empty(),
            username: self.identity.as_ref().map(|i| i.username.clone()),
            households: self
                .identity
                .iter()
                .flat_map(|identity| &identity.memberships)
                .map(|membership| membership.household.clone())
                .collect(),
            current,
        })
    }

    pub fn login(&mut self, username: &str, password: &str, remember: bool) -> Result<()> {
        let mut memberships = Vec::new();
        for path in self.household_files() {
            let household = open(&path)?;
            if let Ok(member) = household.authenticate(username, password) {
                memberships.push(Membership {
                    household: household.info()?,
                    path,
                    member_id: member.id,
                });
            }
        }
        if memberships.is_empty() {
            return Err(finance_core::Error::InvalidCredentials.into());
        }
        memberships.sort_by(|a, b| a.household.name.cmp(&b.household.name));
        self.open = None;
        self.identity = Some(Identity {
            username: username.trim().to_owned(),
            remember,
            memberships,
        });
        self.persist_identity();
        Ok(())
    }

    pub fn logout(&mut self) {
        self.open = None;
        self.identity = None;
        remember::clear();
    }

    /// Cria uma residência nesta máquina, que passa a ser o Anfitrião dela, e
    /// já a abre. Devolve o arquivo criado.
    ///
    /// Quem está conectado leva o próprio usuário; na primeira abertura,
    /// `new_user` traz o usuário e a senha a criar.
    pub fn create_household(
        &mut self,
        name: String,
        new_user: Option<(String, String)>,
        remember: bool,
    ) -> Result<PathBuf> {
        let user = match (&self.identity, new_user) {
            (Some(identity), _) => {
                // Qualquer residência de quem está conectado serve de origem.
                let credentials = identity
                    .memberships
                    .iter()
                    .filter_map(|membership| {
                        let household = open(&membership.path).ok()?;
                        household.user_credentials(membership.member_id).ok()
                    })
                    .next()
                    .ok_or_else(no_session)?;
                NewUser::Existing(credentials)
            }
            (None, Some((username, password))) => NewUser::New { username, password },
            (None, None) => return Err(no_session()),
        };

        let path = self.new_file();
        let created = Connection::open(&path)
            .map_err(finance_core::Error::from)
            .and_then(|conn| {
                Household::create(conn, Arc::new(SystemClock), NewHousehold { name, user })
            });
        let (household, member) = match created {
            Ok(created) => created,
            Err(error) => {
                // Uma criação recusada não pode deixar um arquivo vazio para trás.
                let _ = std::fs::remove_file(&path);
                return Err(error.into());
            }
        };
        self.enter(household, path.clone(), member, remember)?;
        Ok(path)
    }

    /// Onde gravar a próxima residência desta máquina.
    pub fn new_file(&self) -> PathBuf {
        self.dir.join(format!("{}.db", Uuid::new_v4()))
    }

    /// Passa a usar a cópia recém-pareada em `path`, já aberta em nome de `member`.
    pub fn adopt(&mut self, path: PathBuf, member: Member, remember: bool) -> Result<()> {
        let household = open(&path)?;
        self.enter(household, path, member, remember)
    }

    fn enter(
        &mut self,
        household: Household,
        path: PathBuf,
        member: Member,
        remember: bool,
    ) -> Result<()> {
        let info = household.info()?;
        let identity = self.identity.get_or_insert_with(|| Identity {
            username: member.username.clone(),
            remember,
            memberships: Vec::new(),
        });
        identity.memberships.push(Membership {
            household: info.clone(),
            path: path.clone(),
            member_id: member.id,
        });
        self.open = Some(OpenHousehold {
            household,
            info,
            path,
            member_id: member.id,
        });
        self.persist_identity();
        Ok(())
    }

    pub fn select_household(&mut self, id: HouseholdId) -> Result<()> {
        let identity = self.identity.as_ref().ok_or_else(no_session)?;
        let membership = identity
            .memberships
            .iter()
            .find(|membership| membership.household.id == id)
            .ok_or(finance_core::Error::NotFound)?;
        self.open = Some(OpenHousehold {
            household: open(&membership.path)?,
            info: membership.household.clone(),
            path: membership.path.clone(),
            member_id: membership.member_id,
        });
        Ok(())
    }

    pub fn close_household(&mut self) {
        self.open = None;
    }

    /// A residência aberta e o membro em nome de quem os comandos rodam.
    pub fn current(&self) -> Result<(&Household, MemberId)> {
        let open = self.open.as_ref().ok_or_else(no_session)?;
        Ok((&open.household, open.member_id))
    }

    pub fn open_details(&self) -> Option<OpenDetails> {
        let open = self.open.as_ref()?;
        Some(OpenDetails {
            path: open.path.clone(),
            household: open.info.id,
            member: open.member_id,
            replica: open.household.is_replica(),
            host_address: finance_server::stored_address(&open.household),
        })
    }

    /// As residências desta máquina de que ela é o Anfitrião.
    pub fn hosted_files(&self) -> Vec<PathBuf> {
        self.household_files()
            .into_iter()
            .filter(|path| open(path).is_ok_and(|household| !household.is_replica()))
            .collect()
    }

    /// As cópias em que quem está conectado é membro mas ainda não pode
    /// sincronizar, por ter sido outra pessoa a parear este computador.
    pub fn missing_credentials(&self) -> Vec<MissingCredentials> {
        self.identity
            .iter()
            .flat_map(|identity| &identity.memberships)
            .filter_map(|membership| {
                let household = open(&membership.path).ok()?;
                let missing = household.is_replica()
                    && !finance_server::has_credentials(&household, membership.member_id);
                missing.then(|| MissingCredentials {
                    path: membership.path.clone(),
                    household: membership.household.id,
                    host_address: finance_server::stored_address(&household),
                })
            })
            .collect()
    }

    fn household_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "db"))
            .collect()
    }

    fn restore(&self, remembered: Remembered) -> Option<Identity> {
        let mut memberships = Vec::new();
        for path in self.household_files() {
            let Ok(household) = open(&path) else { continue };
            let Ok(info) = household.info() else { continue };
            let member = remembered
                .memberships
                .iter()
                .find(|(household_id, _)| *household_id == info.id)
                .and_then(|(_, member_id)| household.member(*member_id).ok());
            // Quem foi removido da residência desde a última sessão não volta a entrar.
            if let Some(member) = member.filter(|member| member.active) {
                memberships.push(Membership {
                    household: info,
                    path,
                    member_id: member.id,
                });
            }
        }
        if memberships.is_empty() {
            return None;
        }
        memberships.sort_by(|a, b| a.household.name.cmp(&b.household.name));
        Some(Identity {
            username: remembered.username,
            remember: true,
            memberships,
        })
    }

    fn persist_identity(&self) {
        match &self.identity {
            Some(identity) if identity.remember => remember::save(&Remembered {
                username: identity.username.clone(),
                memberships: identity
                    .memberships
                    .iter()
                    .map(|membership| (membership.household.id, membership.member_id))
                    .collect(),
            }),
            _ => remember::clear(),
        }
    }
}

fn open(path: &Path) -> Result<Household> {
    let conn = Connection::open(path).map_err(finance_core::Error::from)?;
    Ok(Household::open(conn, Arc::new(SystemClock))?)
}

fn no_session() -> CommandError {
    CommandError::new("no_session", "Entre com seu usuário para continuar.")
}
