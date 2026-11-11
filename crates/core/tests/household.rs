mod common;

use std::sync::Arc;

use chrono::Duration;
use common::{PASSWORD, fixture};
use finance_core::{Error, Household, NewHousehold, NewUser, Role, SystemClock};
use rusqlite::Connection;

fn create(name: &str, username: &str, password: &str) -> finance_core::Result<Household> {
    Household::create(
        Connection::open_in_memory().unwrap(),
        Arc::new(SystemClock),
        NewHousehold {
            name: name.into(),
            user: NewUser::New {
                username: username.into(),
                password: password.into(),
            },
        },
    )
    .map(|(household, _)| household)
}

#[test]
fn creator_becomes_the_first_admin() {
    let f = fixture();

    assert_eq!(f.admin.username, "vitu");
    assert_eq!(f.admin.role, Role::Admin);
    assert_eq!(f.household.info().unwrap().name, "Casa");
    assert_eq!(f.household.members(f.admin.id).unwrap(), vec![f.admin]);
}

#[test]
fn new_household_comes_with_default_categories() {
    let f = fixture();

    let names: Vec<String> = f
        .household
        .categories(f.admin.id)
        .unwrap()
        .into_iter()
        .map(|category| category.name)
        .collect();

    assert!(names.contains(&"Mercado".to_owned()));
    assert!(names.contains(&"Salário".to_owned()));
}

#[test]
fn household_needs_a_name_and_a_reasonable_password() {
    assert!(matches!(
        create("  ", "vitu", PASSWORD),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        create("Casa", "vitu", "curta"),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        create("Casa", " ", PASSWORD),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn authenticates_with_the_right_password_only() {
    let f = fixture();

    assert_eq!(f.household.authenticate("vitu", PASSWORD).unwrap(), f.admin);
    assert_eq!(f.household.authenticate("VITU", PASSWORD).unwrap(), f.admin);
    assert!(matches!(
        f.household.authenticate("vitu", "senha-errada"),
        Err(Error::InvalidCredentials)
    ));
    assert!(matches!(
        f.household.authenticate("ninguem", PASSWORD),
        Err(Error::InvalidCredentials)
    ));
}

#[test]
fn household_survives_being_closed_and_reopened() {
    let path = std::env::temp_dir().join(format!("finance-core-{}.db", uuid::Uuid::new_v4()));
    let (household, admin) = Household::create(
        Connection::open(&path).unwrap(),
        Arc::new(SystemClock),
        NewHousehold {
            name: "Casa".into(),
            user: NewUser::New {
                username: "vitu".into(),
                password: PASSWORD.into(),
            },
        },
    )
    .unwrap();
    drop(household);

    let reopened =
        Household::open(Connection::open(&path).unwrap(), Arc::new(SystemClock)).unwrap();

    assert_eq!(reopened.info().unwrap().name, "Casa");
    assert_eq!(reopened.authenticate("vitu", PASSWORD).unwrap(), admin);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn user_can_take_their_credentials_to_a_new_household() {
    let f = fixture();
    let credentials = f.household.user_credentials(f.admin.id).unwrap();

    let (second, admin) = Household::create(
        Connection::open_in_memory().unwrap(),
        f.clock.clone(),
        NewHousehold {
            name: "Casa da praia".into(),
            user: NewUser::Existing(credentials),
        },
    )
    .unwrap();

    assert_eq!(second.authenticate("vitu", PASSWORD).unwrap(), admin);
    assert_eq!(admin.role, Role::Admin);
}

#[test]
fn invite_brings_in_a_common_member() {
    let f = fixture();
    let invite = f.household.create_invite(f.admin.id).unwrap();

    let ana = f
        .household
        .accept_invite(&invite.code.to_lowercase(), "ana", PASSWORD)
        .unwrap();

    assert_eq!(ana.role, Role::Common);
    assert_eq!(f.household.authenticate("ana", PASSWORD).unwrap(), ana);
    assert_eq!(f.household.members(ana.id).unwrap().len(), 2);
}

#[test]
fn invite_works_only_once() {
    let f = fixture();
    let invite = f.household.create_invite(f.admin.id).unwrap();
    f.household
        .accept_invite(&invite.code, "ana", PASSWORD)
        .unwrap();

    assert!(matches!(
        f.household.accept_invite(&invite.code, "bia", PASSWORD),
        Err(Error::InviteUsed)
    ));
}

#[test]
fn invite_expires_after_24_hours() {
    let f = fixture();
    let invite = f.household.create_invite(f.admin.id).unwrap();

    f.clock.advance(Duration::hours(23));
    let still_valid = f.household.create_invite(f.admin.id).unwrap();
    f.clock.advance(Duration::hours(1));

    assert!(matches!(
        f.household.accept_invite(&invite.code, "ana", PASSWORD),
        Err(Error::InviteExpired)
    ));
    assert!(
        f.household
            .accept_invite(&still_valid.code, "ana", PASSWORD)
            .is_ok()
    );
}

#[test]
fn unknown_invite_code_is_rejected() {
    let f = fixture();

    assert!(matches!(
        f.household.accept_invite("NAOEXISTE", "ana", PASSWORD),
        Err(Error::InviteInvalid)
    ));
}

#[test]
fn failed_invite_acceptance_does_not_consume_the_invite() {
    let f = fixture();
    let invite = f.household.create_invite(f.admin.id).unwrap();

    assert!(matches!(
        f.household.accept_invite(&invite.code, "vitu", PASSWORD),
        Err(Error::UsernameTaken)
    ));
    assert!(matches!(
        f.household.accept_invite(&invite.code, "ana", "curta"),
        Err(Error::Invalid(_))
    ));
    assert!(
        f.household
            .accept_invite(&invite.code, "ana", PASSWORD)
            .is_ok()
    );
}

#[test]
fn only_admins_create_invites() {
    let f = fixture();
    let ana = f.join("ana");

    assert!(matches!(
        f.household.create_invite(ana.id),
        Err(Error::Forbidden)
    ));
}

#[test]
fn someone_who_is_not_a_member_sees_nothing() {
    let f = fixture();
    let stranger = uuid::Uuid::new_v4();

    assert!(matches!(
        f.household.members(stranger),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.categories(stranger),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.entries(stranger, &Default::default()),
        Err(Error::Forbidden)
    ));
}
