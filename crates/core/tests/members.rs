mod common;

use chrono::Duration;
use common::{PASSWORD, fixture};
use finance_core::{Error, Role};

#[test]
fn admin_promotes_and_demotes_members() {
    let f = fixture();
    let ana = f.join("ana");

    let promoted = f
        .household
        .set_member_role(f.admin.id, ana.id, Role::Admin)
        .unwrap();
    // Agora ana também administra, inclusive o papel de quem a promoveu.
    let demoted = f
        .household
        .set_member_role(ana.id, f.admin.id, Role::Common)
        .unwrap();

    assert_eq!(promoted.role, Role::Admin);
    assert_eq!(demoted.role, Role::Common);
    assert!(matches!(
        f.household
            .set_member_role(f.admin.id, ana.id, Role::Common),
        Err(Error::Forbidden)
    ));
}

#[test]
fn household_never_loses_its_last_admin() {
    let f = fixture();
    let ana = f.join("ana");

    assert!(matches!(
        f.household
            .set_member_role(f.admin.id, f.admin.id, Role::Common),
        Err(Error::LastAdmin)
    ));
    assert!(matches!(
        f.household.remove_member(f.admin.id, f.admin.id),
        Err(Error::LastAdmin)
    ));

    f.household
        .set_member_role(f.admin.id, ana.id, Role::Admin)
        .unwrap();
    assert!(f.household.remove_member(f.admin.id, f.admin.id).is_ok());
}

#[test]
fn removed_member_loses_access_but_keeps_their_entries() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&ana, 500, "2026-11-10", "Mercado");
    let token = f.household.issue_token(ana.id).unwrap();

    f.household.remove_member(f.admin.id, ana.id).unwrap();

    assert!(matches!(
        f.household.authenticate("ana", PASSWORD),
        Err(Error::InvalidCredentials)
    ));
    assert!(matches!(
        f.household.authenticate_token(&token),
        Err(Error::InvalidSession)
    ));
    assert!(matches!(
        f.household.entries(ana.id, &Default::default()),
        Err(Error::Forbidden)
    ));
    let entries = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap();
    assert_eq!(entries[0].assignee_name, "ana");
    let members = f.household.members(f.admin.id).unwrap();
    assert!(!members.iter().find(|m| m.id == ana.id).unwrap().active);
}

#[test]
fn only_admins_manage_members() {
    let f = fixture();
    let ana = f.join("ana");
    let bia = f.join("bia");

    assert!(matches!(
        f.household.set_member_role(ana.id, ana.id, Role::Admin),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.remove_member(ana.id, bia.id),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household
            .reset_password(ana.id, bia.id, "outra-senha-boa"),
        Err(Error::Forbidden)
    ));
}

#[test]
fn admin_resets_a_forgotten_password() {
    let f = fixture();
    f.join("ana");
    let ana = f.household.authenticate("ana", PASSWORD).unwrap();

    f.household
        .reset_password(f.admin.id, ana.id, "senha-nova-da-ana")
        .unwrap();

    assert!(f.household.authenticate("ana", "senha-nova-da-ana").is_ok());
    assert!(matches!(
        f.household.authenticate("ana", PASSWORD),
        Err(Error::InvalidCredentials)
    ));
    assert!(matches!(
        f.household.reset_password(f.admin.id, ana.id, "curta"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn member_changes_their_own_password_knowing_the_current_one() {
    let f = fixture();
    let ana = f.join("ana");

    assert!(matches!(
        f.household
            .change_password(ana.id, "senha-errada", "senha-nova-da-ana"),
        Err(Error::InvalidCredentials)
    ));
    f.household
        .change_password(ana.id, PASSWORD, "senha-nova-da-ana")
        .unwrap();

    assert!(f.household.authenticate("ana", "senha-nova-da-ana").is_ok());
}

#[test]
fn pending_invites_can_be_listed_and_revoked() {
    let f = fixture();
    let used = f.household.create_invite(f.admin.id).unwrap();
    f.household
        .accept_invite(&used.code, "ana", PASSWORD)
        .unwrap();
    let expired = f.household.create_invite(f.admin.id).unwrap();
    f.clock.advance(Duration::hours(25));
    let revoked = f.household.create_invite(f.admin.id).unwrap();
    let open = f.household.create_invite(f.admin.id).unwrap();

    f.household.revoke_invite(f.admin.id, revoked.id).unwrap();

    let pending: Vec<String> = f
        .household
        .invites(f.admin.id)
        .unwrap()
        .into_iter()
        .map(|invite| invite.code)
        .collect();
    assert_eq!(pending, vec![open.code]);
    assert!(!pending.contains(&expired.code));
    assert!(matches!(
        f.household.accept_invite(&revoked.code, "bia", PASSWORD),
        Err(Error::InviteInvalid)
    ));
    assert!(matches!(
        f.household.revoke_invite(f.admin.id, used.id),
        Err(Error::NotFound)
    ));
}

#[test]
fn token_identifies_the_member_it_was_issued_to() {
    let f = fixture();
    let ana = f.join("ana");

    let token = f.household.issue_token(ana.id).unwrap();

    assert_eq!(f.household.authenticate_token(&token).unwrap(), ana);
    assert!(matches!(
        f.household.authenticate_token("inventado"),
        Err(Error::InvalidSession)
    ));
}
