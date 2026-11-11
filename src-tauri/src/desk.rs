//! O estado do app nesta máquina: quem está conectado, de quais residências
//! locais essa pessoa é membro e qual delas está aberta.

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

    /// Cria uma residência nesta máquina e já a abre.
    ///
    /// Quem está conectado leva o próprio usuário; na primeira abertura,
    /// `new_user` traz o usuário e a senha a criar.
    pub fn create_household(
        &mut self,
        name: String,
        new_user: Option<(String, String)>,
        remember: bool,
    ) -> Result<()> {
        let user = match (&self.identity, new_user) {
            (Some(identity), _) => {
                let membership = identity.memberships.first().ok_or_else(no_session)?;
                let credentials = open(&membership.path)?.user_credentials(membership.member_id)?;
                NewUser::Existing(credentials)
            }
            (None, Some((username, password))) => NewUser::New { username, password },
            (None, None) => return Err(no_session()),
        };

        let path = self.dir.join(format!("{}.db", Uuid::new_v4()));
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

        let info = household.info()?;
        let identity = self.identity.get_or_insert_with(|| Identity {
            username: member.username.clone(),
            remember,
            memberships: Vec::new(),
        });
        identity.memberships.push(Membership {
            household: info.clone(),
            path,
            member_id: member.id,
        });
        self.open = Some(OpenHousehold {
            household,
            info,
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
