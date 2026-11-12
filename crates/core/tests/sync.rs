mod common;

use common::{Fixture, PASSWORD, day, fixture};
use finance_core::{
    Connection, Entry, EntryChange, EntryInput, EntryKind, EntryStatus, Error, HostCommand,
    Household, Member, NewCategory, PushOutcome, RecurrenceInput, SyncStatus,
};

/// Uma cópia nova da residência, já com tudo o que o Anfitrião tem.
fn replica_of(f: &Fixture) -> Household {
    let replica =
        Household::create_replica(Connection::open_in_memory().unwrap(), f.clock.clone()).unwrap();
    pull(f, &replica);
    replica
}

fn pull(f: &Fixture, replica: &Household) {
    let changes = f
        .household
        .changes_since(replica.cursor().unwrap())
        .unwrap();
    replica.apply_changes(&changes).unwrap();
}

/// Envia o que a cópia tem pendente em nome de `member` e depois recebe o
/// que mudou no Anfitrião, como faz a sincronização de verdade.
fn sync(f: &Fixture, replica: &Household, member: &Member) {
    let pending = replica.pending_changes().unwrap();
    let changes: Vec<EntryChange> = pending.iter().map(|p| p.change.clone()).collect();
    let outcomes = f.household.apply_push(member.id, &changes).unwrap();
    replica.record_push(&pending, &outcomes).unwrap();
    pull(f, replica);
}

fn all(household: &Household, viewer: &Member) -> Vec<Entry> {
    household.entries(viewer.id, &Default::default()).unwrap()
}

fn status(pending: i64, conflicts: i64) -> SyncStatus {
    SyncStatus { pending, conflicts }
}

fn edit(f: &Fixture, amount_cents: i64, description: &str) -> EntryInput {
    EntryInput {
        description: description.into(),
        ..f.expense_input(amount_cents, "2026-11-10", "Mercado")
    }
}

#[test]
fn new_replica_receives_the_whole_household() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&f.admin, 100, "2026-11-10", "Mercado");
    f.income(&ana, 500_000, "2026-11-05");

    let replica = replica_of(&f);

    assert!(replica.is_replica());
    assert_eq!(replica.info().unwrap(), f.household.info().unwrap());
    assert_eq!(all(&replica, &ana), all(&f.household, &ana));
    assert_eq!(
        replica.categories(ana.id).unwrap(),
        f.household.categories(ana.id).unwrap()
    );
    assert_eq!(
        replica.members(ana.id).unwrap(),
        f.household.members(ana.id).unwrap()
    );
    // Com os usuários replicados, dá para entrar com o Anfitrião desligado.
    assert_eq!(replica.authenticate("ana", PASSWORD).unwrap(), ana);
    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
}

#[test]
fn later_pulls_bring_only_what_changed() {
    let f = fixture();
    f.expense(&f.admin, 100, "2026-11-10", "Mercado");
    let replica = replica_of(&f);

    let nothing = f
        .household
        .changes_since(replica.cursor().unwrap())
        .unwrap();
    let added = f.expense(&f.admin, 200, "2026-11-10", "Lazer");
    let one_entry = f
        .household
        .changes_since(replica.cursor().unwrap())
        .unwrap();

    assert!(nothing.tables.is_empty());
    assert_eq!(one_entry.tables.len(), 1);
    assert_eq!(one_entry.tables[0].table, "entry");
    assert_eq!(one_entry.tables[0].rows.len(), 1);
    replica.apply_changes(&one_entry).unwrap();
    assert!(all(&replica, &f.admin).contains(&added));
}

#[test]
fn entry_created_offline_reaches_the_host_and_the_other_replicas() {
    let f = fixture();
    let ana = f.join("ana");
    let anas = replica_of(&f);
    let other = replica_of(&f);

    let created = anas
        .create_entry(ana.id, f.expense_input(8_450, "2026-11-10", "Mercado"))
        .unwrap();
    assert_eq!(anas.sync_status().unwrap(), status(1, 0));
    assert!(all(&f.household, &ana).is_empty());

    sync(&f, &anas, &ana);
    sync(&f, &other, &f.admin);

    assert_eq!(anas.sync_status().unwrap(), status(0, 0));
    let on_host = all(&f.household, &ana);
    assert_eq!(on_host.len(), 1);
    assert_eq!(on_host[0].id, created.id);
    assert_eq!(on_host[0].amount_cents, 8_450);
    assert_eq!(on_host[0].assignee_id, ana.id);
    assert_eq!(all(&other, &ana), on_host);
    assert_eq!(all(&anas, &ana), on_host);
}

#[test]
fn changes_to_different_entries_merge_without_asking() {
    let f = fixture();
    let first = f.expense(&f.admin, 100, "2026-11-10", "Mercado");
    let second = f.expense(&f.admin, 200, "2026-11-10", "Mercado");
    let replica = replica_of(&f);

    replica
        .update_entry(f.admin.id, first.id, edit(&f, 111, "na cópia"))
        .unwrap();
    f.household
        .update_entry(f.admin.id, second.id, edit(&f, 222, "no anfitrião"))
        .unwrap();
    sync(&f, &replica, &f.admin);

    let amounts = |household: &Household| {
        let mut amounts: Vec<i64> = all(household, &f.admin)
            .iter()
            .map(|e| e.amount_cents)
            .collect();
        amounts.sort();
        amounts
    };
    assert_eq!(amounts(&f.household), vec![111, 222]);
    assert_eq!(amounts(&replica), vec![111, 222]);
    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
}

#[test]
fn same_entry_changed_on_both_sides_becomes_a_conflict() {
    let f = fixture();
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let untouched = f.expense(&f.admin, 500, "2026-11-10", "Lazer");
    let replica = replica_of(&f);

    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 12_000, "valor corrigido"))
        .unwrap();
    replica
        .update_entry(f.admin.id, untouched.id, edit(&f, 600, "outro lançamento"))
        .unwrap();
    f.household
        .update_entry(
            f.admin.id,
            entry.id,
            f.expense_input(10_000, "2026-11-10", "Lazer"),
        )
        .unwrap();
    sync(&f, &replica, &f.admin);

    // Ninguém perde nada: cada lado segue com a sua versão até alguém escolher.
    assert_eq!(replica.sync_status().unwrap(), status(0, 1));
    let conflicts = replica.conflicts(f.admin.id).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].entry_id, entry.id);
    assert_eq!(conflicts[0].reason, None);
    let mine = conflicts[0].mine.as_ref().unwrap();
    let theirs = conflicts[0].theirs.as_ref().unwrap();
    assert_eq!(
        (mine.amount_cents, mine.category_name.as_str()),
        (12_000, "Mercado")
    );
    assert_eq!(
        (theirs.amount_cents, theirs.category_name.as_str()),
        (10_000, "Lazer")
    );

    // O conflito não trava o resto da sincronização.
    let host_untouched = all(&f.household, &f.admin)
        .into_iter()
        .find(|e| e.id == untouched.id)
        .unwrap();
    assert_eq!(host_untouched.amount_cents, 600);
}

#[test]
fn keeping_my_version_sends_it_to_the_host() {
    let f = fixture();
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let replica = replica_of(&f);
    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 12_000, "minha"))
        .unwrap();
    f.household
        .update_entry(f.admin.id, entry.id, edit(&f, 9_000, "do anfitrião"))
        .unwrap();
    sync(&f, &replica, &f.admin);

    replica
        .resolve_conflict(f.admin.id, entry.id, true)
        .unwrap();
    assert_eq!(replica.sync_status().unwrap(), status(1, 0));
    sync(&f, &replica, &f.admin);

    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
    assert_eq!(all(&f.household, &f.admin)[0].description, "minha");
    assert_eq!(all(&f.household, &f.admin)[0].amount_cents, 12_000);
    assert_eq!(all(&replica, &f.admin), all(&f.household, &f.admin));
}

#[test]
fn keeping_the_host_version_discards_mine() {
    let f = fixture();
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let replica = replica_of(&f);
    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 12_000, "minha"))
        .unwrap();
    f.household
        .update_entry(f.admin.id, entry.id, edit(&f, 9_000, "do anfitrião"))
        .unwrap();
    sync(&f, &replica, &f.admin);

    replica
        .resolve_conflict(f.admin.id, entry.id, false)
        .unwrap();
    sync(&f, &replica, &f.admin);

    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
    assert_eq!(all(&replica, &f.admin)[0].description, "do anfitrião");
    assert_eq!(all(&replica, &f.admin), all(&f.household, &f.admin));
}

#[test]
fn editing_here_what_was_deleted_there_is_a_conflict() {
    let f = fixture();
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let replica = replica_of(&f);
    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 12_000, "ainda vale"))
        .unwrap();
    f.household.delete_entry(f.admin.id, entry.id).unwrap();
    sync(&f, &replica, &f.admin);

    let conflicts = replica.conflicts(f.admin.id).unwrap();
    assert!(conflicts[0].mine.is_some());
    assert!(conflicts[0].theirs.is_none());

    // Ficar com a minha versão traz o lançamento de volta no Anfitrião.
    replica
        .resolve_conflict(f.admin.id, entry.id, true)
        .unwrap();
    sync(&f, &replica, &f.admin);
    assert_eq!(all(&f.household, &f.admin)[0].description, "ainda vale");
    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
}

#[test]
fn deleting_here_what_was_edited_there_is_a_conflict() {
    let f = fixture();
    let kept = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let dropped = f.expense(&f.admin, 20_000, "2026-11-10", "Mercado");
    let replica = replica_of(&f);
    for entry in [&kept, &dropped] {
        replica.delete_entry(f.admin.id, entry.id).unwrap();
        f.household
            .update_entry(
                f.admin.id,
                entry.id,
                edit(&f, entry.amount_cents + 1, "editado lá"),
            )
            .unwrap();
    }
    sync(&f, &replica, &f.admin);

    let conflicts = replica.conflicts(f.admin.id).unwrap();
    assert_eq!(conflicts.len(), 2);
    assert!(
        conflicts
            .iter()
            .all(|c| c.mine.is_none() && c.theirs.is_some())
    );

    // Um volta como o Anfitrião o tem; o outro é excluído lá também.
    replica
        .resolve_conflict(f.admin.id, kept.id, false)
        .unwrap();
    replica
        .resolve_conflict(f.admin.id, dropped.id, true)
        .unwrap();
    sync(&f, &replica, &f.admin);

    let left = all(&f.household, &f.admin);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, kept.id);
    assert_eq!(left[0].amount_cents, 10_001);
    assert_eq!(all(&replica, &f.admin), left);
}

#[test]
fn change_refused_by_the_host_comes_back_with_the_reason() {
    let f = fixture();
    let replica = replica_of(&f);
    // A categoria é arquivada no Anfitrião sem que a cópia saiba.
    f.household
        .set_category_archived(f.admin.id, f.category("Lazer"), true)
        .unwrap();
    let local = replica
        .create_entry(f.admin.id, f.expense_input(5_000, "2026-11-10", "Lazer"))
        .unwrap();

    sync(&f, &replica, &f.admin);

    let conflicts = replica.conflicts(f.admin.id).unwrap();
    assert_eq!(conflicts[0].entry_id, local.id);
    assert_eq!(
        conflicts[0].reason.as_deref(),
        Some("Esta categoria está arquivada.")
    );
    assert!(conflicts[0].theirs.is_none());
    assert!(all(&f.household, &f.admin).is_empty());

    // Desistir da alteração remove o lançamento que só existia aqui.
    replica
        .resolve_conflict(f.admin.id, local.id, false)
        .unwrap();
    assert!(all(&replica, &f.admin).is_empty());
    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
}

#[test]
fn host_enforces_permissions_on_pushed_changes() {
    let f = fixture();
    let ana = f.join("ana");
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");

    // Uma cópia adulterada tenta alterar, como ana, o lançamento de outra pessoa.
    let outcomes = f
        .household
        .apply_push(
            ana.id,
            &[
                EntryChange::Upsert {
                    id: entry.id,
                    base_version: entry.version,
                    input: edit(&f, 1, "adulterado"),
                },
                EntryChange::Delete {
                    id: entry.id,
                    base_version: entry.version,
                },
            ],
        )
        .unwrap();

    assert!(outcomes.iter().all(|outcome| matches!(
        outcome,
        PushOutcome::Conflict {
            reason: Some(_),
            ..
        }
    )));
    assert_eq!(all(&f.household, &f.admin), vec![entry]);
}

#[test]
fn edit_made_while_a_push_is_in_flight_stays_pending() {
    let f = fixture();
    let entry = f.expense(&f.admin, 10_000, "2026-11-10", "Mercado");
    let replica = replica_of(&f);
    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 11_000, "primeira edição"))
        .unwrap();

    // O envio parte, e antes da resposta chegar o lançamento é editado de novo.
    let pending = replica.pending_changes().unwrap();
    let changes: Vec<EntryChange> = pending.iter().map(|p| p.change.clone()).collect();
    let outcomes = f.household.apply_push(f.admin.id, &changes).unwrap();
    replica
        .update_entry(f.admin.id, entry.id, edit(&f, 12_000, "segunda edição"))
        .unwrap();
    replica.record_push(&pending, &outcomes).unwrap();
    pull(&f, &replica);

    assert_eq!(replica.sync_status().unwrap(), status(1, 0));
    assert_eq!(all(&replica, &f.admin)[0].description, "segunda edição");

    sync(&f, &replica, &f.admin);
    assert_eq!(replica.sync_status().unwrap(), status(0, 0));
    assert_eq!(all(&f.household, &f.admin)[0].description, "segunda edição");
}

#[test]
fn settling_offline_syncs_like_any_other_edit() {
    let f = fixture();
    let bill = f.expense(&f.admin, 20_000, "2026-11-20", "Contas e serviços");
    let replica = replica_of(&f);

    replica
        .settle_entry(f.admin.id, bill.id, Some(23_100))
        .unwrap();
    sync(&f, &replica, &f.admin);

    let on_host = &all(&f.household, &f.admin)[0];
    assert_eq!(on_host.status, EntryStatus::Settled);
    assert_eq!(on_host.amount_cents, 23_100);
}

#[test]
fn structural_changes_are_refused_on_a_replica() {
    let f = fixture();
    let replica = replica_of(&f);
    let pets = || NewCategory {
        name: "Pets".into(),
        kind: EntryKind::Expense,
    };
    let rent = RecurrenceInput {
        kind: EntryKind::Expense,
        amount_cents: 150_000,
        description: "Aluguel".into(),
        category_id: f.category("Moradia"),
        assignee_id: None,
        first_date: day("2026-11-20"),
        end_month: None,
    };

    assert!(matches!(
        replica.create_category(f.admin.id, pets()),
        Err(Error::NeedsHost)
    ));
    assert!(matches!(
        replica.create_recurrence(f.admin.id, rent.clone()),
        Err(Error::NeedsHost)
    ));
    assert!(matches!(
        replica.create_invite(f.admin.id),
        Err(Error::NeedsHost)
    ));
    assert!(matches!(
        replica.remove_member(f.admin.id, f.admin.id),
        Err(Error::NeedsHost)
    ));

    // Feitas no Anfitrião, chegam à cópia como qualquer outra alteração.
    f.household.create_category(f.admin.id, pets()).unwrap();
    f.household.create_recurrence(f.admin.id, rent).unwrap();
    pull(&f, &replica);

    assert!(
        replica
            .categories(f.admin.id)
            .unwrap()
            .iter()
            .any(|c| c.name == "Pets")
    );
    assert_eq!(replica.recurrences(f.admin.id).unwrap().len(), 1);
    assert_eq!(all(&replica, &f.admin).len(), 13);
}

#[test]
fn member_changes_reach_the_replicas() {
    let f = fixture();
    let ana = f.join("ana");
    let replica = replica_of(&f);

    f.household
        .reset_password(f.admin.id, ana.id, "senha-nova-da-ana")
        .unwrap();
    pull(&f, &replica);
    assert!(replica.authenticate("ana", "senha-nova-da-ana").is_ok());

    f.household.remove_member(f.admin.id, ana.id).unwrap();
    pull(&f, &replica);
    assert!(matches!(
        replica.authenticate("ana", "senha-nova-da-ana"),
        Err(Error::InvalidCredentials)
    ));
}

#[test]
fn host_commands_run_as_the_member_who_sent_them() {
    let f = fixture();
    let ana = f.join("ana");
    let pets = || {
        HostCommand::CreateCategory(NewCategory {
            name: "Pets".into(),
            kind: EntryKind::Expense,
        })
    };

    let created = f.household.execute(f.admin.id, pets()).unwrap();

    assert_eq!(created["name"], "Pets");
    assert!(matches!(
        f.household.execute(ana.id, pets()),
        Err(Error::Forbidden)
    ));
    let invite = f
        .household
        .execute(f.admin.id, HostCommand::CreateInvite)
        .unwrap();
    assert_eq!(invite["code"].as_str().unwrap().len(), 8);
}
