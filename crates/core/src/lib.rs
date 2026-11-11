//! Regras de negócio e persistência de uma Residência.
//!
//! [`Household`] é a interface de aplicação: todo acesso aos dados passa por
//! seus comandos e consultas, sempre em nome de um membro, e é aqui que as
//! permissões são verificadas.

mod categories;
mod clock;
mod dashboard;
mod entries;
mod error;
mod household;
mod month;
mod schema;

pub use categories::{Category, NewCategory};
pub use clock::{Clock, SystemClock};
pub use dashboard::{AssigneeTotal, CategoryTotal, Dashboard, MonthFlow};
pub use entries::{Entry, EntryFilter, EntryInput, EntryKind};
pub use error::{Error, Result};
pub use household::{
    Household, HouseholdInfo, Invite, Member, NewHousehold, NewUser, Role, UserCredentials,
};
pub use month::Month;
pub use rusqlite::Connection;

pub type HouseholdId = uuid::Uuid;
pub type MemberId = uuid::Uuid;
pub type CategoryId = uuid::Uuid;
pub type EntryId = uuid::Uuid;
