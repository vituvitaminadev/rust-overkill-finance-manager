#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use finance_core::{
    CategoryId, Clock, Entry, EntryInput, EntryKind, Household, Member, NewHousehold, NewUser,
};
use rusqlite::Connection;

pub const PASSWORD: &str = "senha-secreta";

pub struct TestClock(Mutex<DateTime<Utc>>);

impl TestClock {
    pub fn advance(&self, duration: Duration) {
        *self.0.lock().unwrap() += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

/// Uma residência em memória, criada por "vitu" em 11/11/2026.
pub struct Fixture {
    pub household: Household,
    pub admin: Member,
    pub clock: Arc<TestClock>,
}

pub fn fixture() -> Fixture {
    let clock = Arc::new(TestClock(Mutex::new(
        Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap(),
    )));
    let (household, admin) = Household::create(
        Connection::open_in_memory().unwrap(),
        clock.clone(),
        NewHousehold {
            name: "Casa".into(),
            user: NewUser::New {
                username: "vitu".into(),
                password: PASSWORD.into(),
            },
        },
    )
    .unwrap();
    Fixture {
        household,
        admin,
        clock,
    }
}

impl Fixture {
    /// Um novo membro Comum, entrando por convite.
    pub fn join(&self, username: &str) -> Member {
        let invite = self.household.create_invite(self.admin.id).unwrap();
        self.household
            .accept_invite(&invite.code, username, PASSWORD)
            .unwrap()
    }

    pub fn category(&self, name: &str) -> CategoryId {
        self.household
            .categories(self.admin.id)
            .unwrap()
            .into_iter()
            .find(|category| category.name == name)
            .unwrap_or_else(|| panic!("categoria {name} não existe"))
            .id
    }

    pub fn expense_input(&self, amount_cents: i64, date: &str, category: &str) -> EntryInput {
        EntryInput {
            kind: EntryKind::Expense,
            amount_cents,
            date: day(date),
            description: format!("{category} de teste"),
            category_id: self.category(category),
            assignee_id: None,
        }
    }

    pub fn income_input(&self, amount_cents: i64, date: &str) -> EntryInput {
        EntryInput {
            kind: EntryKind::Income,
            amount_cents,
            date: day(date),
            description: "Salário de teste".into(),
            category_id: self.category("Salário"),
            assignee_id: None,
        }
    }

    pub fn expense(&self, actor: &Member, amount_cents: i64, date: &str, category: &str) -> Entry {
        self.household
            .create_entry(actor.id, self.expense_input(amount_cents, date, category))
            .unwrap()
    }

    pub fn income(&self, actor: &Member, amount_cents: i64, date: &str) -> Entry {
        self.household
            .create_entry(actor.id, self.income_input(amount_cents, date))
            .unwrap()
    }
}

pub fn day(date: &str) -> NaiveDate {
    date.parse().unwrap()
}
