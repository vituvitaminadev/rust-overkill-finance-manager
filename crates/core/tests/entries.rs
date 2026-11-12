mod common;

use common::{day, fixture};
use finance_core::{EntryFilter, EntryInput, EntryKind, EntryStatus, Error, Month};

#[test]
fn entry_is_assigned_to_whoever_created_it() {
    let f = fixture();
    let ana = f.join("ana");

    let entry = f.expense(&ana, 12_345, "2026-11-10", "Mercado");

    assert_eq!(entry.kind, EntryKind::Expense);
    assert_eq!(entry.amount_cents, 12_345);
    assert_eq!(entry.date, day("2026-11-10"));
    assert_eq!(entry.category_name, "Mercado");
    assert_eq!(entry.assignee_id, ana.id);
    assert_eq!(entry.assignee_name, "ana");
    assert_eq!(entry.version, 1);
}

#[test]
fn admin_can_assign_an_entry_to_any_member() {
    let f = fixture();
    let ana = f.join("ana");

    let entry = f
        .household
        .create_entry(
            f.admin.id,
            EntryInput {
                assignee_id: Some(ana.id),
                ..f.expense_input(100, "2026-11-10", "Mercado")
            },
        )
        .unwrap();

    assert_eq!(entry.assignee_id, ana.id);
}

#[test]
fn common_member_cannot_assign_an_entry_to_someone_else() {
    let f = fixture();
    let ana = f.join("ana");

    let result = f.household.create_entry(
        ana.id,
        EntryInput {
            assignee_id: Some(f.admin.id),
            ..f.expense_input(100, "2026-11-10", "Mercado")
        },
    );

    assert!(matches!(result, Err(Error::Forbidden)));
}

#[test]
fn assignee_must_be_a_member() {
    let f = fixture();

    let result = f.household.create_entry(
        f.admin.id,
        EntryInput {
            assignee_id: Some(uuid::Uuid::new_v4()),
            ..f.expense_input(100, "2026-11-10", "Mercado")
        },
    );

    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[test]
fn everyone_sees_every_entry_but_edits_only_what_they_may() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&f.admin, 100, "2026-11-10", "Mercado");
    f.expense(&ana, 200, "2026-11-11", "Lazer");

    let seen_by_ana = f.household.entries(ana.id, &Default::default()).unwrap();
    let seen_by_admin = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap();

    assert_eq!(seen_by_ana.len(), 2);
    let editable_by_ana: Vec<bool> = seen_by_ana.iter().map(|e| e.can_edit).collect();
    assert_eq!(editable_by_ana, vec![true, false]);
    assert!(seen_by_admin.iter().all(|e| e.can_edit));
}

#[test]
fn common_member_cannot_edit_or_delete_entries_of_others() {
    let f = fixture();
    let ana = f.join("ana");
    let entry = f.expense(&f.admin, 100, "2026-11-10", "Mercado");

    assert!(matches!(
        f.household.update_entry(
            ana.id,
            entry.id,
            f.expense_input(1, "2026-11-10", "Mercado")
        ),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.delete_entry(ana.id, entry.id),
        Err(Error::Forbidden)
    ));
    let listed = f.household.entries(ana.id, &Default::default()).unwrap();
    assert_eq!(listed[0].amount_cents, 100);
}

#[test]
fn common_member_edits_their_own_entry_and_stays_assigned() {
    let f = fixture();
    let ana = f.join("ana");
    let entry = f.expense(&ana, 100, "2026-11-10", "Mercado");

    let edited = f
        .household
        .update_entry(
            ana.id,
            entry.id,
            f.expense_input(250, "2026-11-12", "Lazer"),
        )
        .unwrap();

    assert_eq!(edited.amount_cents, 250);
    assert_eq!(edited.date, day("2026-11-12"));
    assert_eq!(edited.category_name, "Lazer");
    assert_eq!(edited.assignee_id, ana.id);
    assert_eq!(edited.version, 2);

    let handed_over = f.household.update_entry(
        ana.id,
        entry.id,
        EntryInput {
            assignee_id: Some(f.admin.id),
            ..f.expense_input(250, "2026-11-12", "Lazer")
        },
    );
    assert!(matches!(handed_over, Err(Error::Forbidden)));
}

#[test]
fn admin_edits_and_deletes_entries_of_others() {
    let f = fixture();
    let ana = f.join("ana");
    let first = f.expense(&ana, 100, "2026-11-10", "Mercado");
    let second = f.expense(&ana, 200, "2026-11-11", "Mercado");

    let edited = f
        .household
        .update_entry(
            f.admin.id,
            first.id,
            f.expense_input(150, "2026-11-10", "Mercado"),
        )
        .unwrap();
    f.household.delete_entry(f.admin.id, second.id).unwrap();

    // Editar sem informar o Responsável não tira o lançamento de quem ele era.
    assert_eq!(edited.assignee_id, ana.id);
    let listed = f.household.entries(ana.id, &Default::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].amount_cents, 150);
}

#[test]
fn deleted_entry_is_gone_for_good() {
    let f = fixture();
    let entry = f.expense(&f.admin, 100, "2026-11-10", "Mercado");

    f.household.delete_entry(f.admin.id, entry.id).unwrap();

    assert!(
        f.household
            .entries(f.admin.id, &Default::default())
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        f.household.delete_entry(f.admin.id, entry.id),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        f.household.update_entry(
            f.admin.id,
            entry.id,
            f.expense_input(1, "2026-11-10", "Mercado")
        ),
        Err(Error::NotFound)
    ));
}

#[test]
fn invalid_entries_are_rejected() {
    let f = fixture();
    let create = |input| f.household.create_entry(f.admin.id, input);

    let zero = f.expense_input(0, "2026-11-10", "Mercado");
    let negative = f.expense_input(-500, "2026-11-10", "Mercado");
    let blank = EntryInput {
        description: "   ".into(),
        ..f.expense_input(100, "2026-11-10", "Mercado")
    };
    let income_in_expense_category = EntryInput {
        kind: EntryKind::Income,
        ..f.expense_input(100, "2026-11-10", "Mercado")
    };
    let unknown_category = EntryInput {
        category_id: uuid::Uuid::new_v4(),
        ..f.expense_input(100, "2026-11-10", "Mercado")
    };

    for input in [
        zero,
        negative,
        blank,
        income_in_expense_category,
        unknown_category,
    ] {
        assert!(matches!(create(input), Err(Error::Invalid(_))));
    }
    assert!(
        f.household
            .entries(f.admin.id, &Default::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn entries_are_listed_from_newest_to_oldest() {
    let f = fixture();
    f.expense(&f.admin, 1, "2026-11-05", "Mercado");
    f.expense(&f.admin, 2, "2026-11-20", "Mercado");
    f.expense(&f.admin, 3, "2026-10-31", "Mercado");

    let amounts: Vec<i64> = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap()
        .iter()
        .map(|e| e.amount_cents)
        .collect();

    assert_eq!(amounts, vec![2, 1, 3]);
}

#[test]
fn entries_can_be_filtered() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&f.admin, 1, "2026-10-31", "Mercado");
    f.expense(&f.admin, 2, "2026-11-01", "Mercado");
    f.expense(&ana, 3, "2026-11-30", "Lazer");
    f.income(&ana, 4, "2026-11-05");
    f.expense(&f.admin, 5, "2026-12-01", "Mercado");

    let amounts = |filter: EntryFilter| -> Vec<i64> {
        let mut amounts: Vec<i64> = f
            .household
            .entries(f.admin.id, &filter)
            .unwrap()
            .iter()
            .map(|e| e.amount_cents)
            .collect();
        amounts.sort();
        amounts
    };
    let november: Month = "2026-11".parse().unwrap();

    assert_eq!(
        amounts(EntryFilter {
            month: Some(november),
            ..Default::default()
        }),
        vec![2, 3, 4]
    );
    assert_eq!(
        amounts(EntryFilter {
            kind: Some(EntryKind::Income),
            ..Default::default()
        }),
        vec![4]
    );
    assert_eq!(
        amounts(EntryFilter {
            category_id: Some(f.category("Mercado")),
            ..Default::default()
        }),
        vec![1, 2, 5]
    );
    assert_eq!(
        amounts(EntryFilter {
            assignee_id: Some(ana.id),
            ..Default::default()
        }),
        vec![3, 4]
    );
    assert_eq!(
        amounts(EntryFilter {
            month: Some(november),
            assignee_id: Some(ana.id),
            kind: Some(EntryKind::Expense),
            ..Default::default()
        }),
        vec![3]
    );
}

#[test]
fn search_ignores_case_including_accented_letters() {
    let f = fixture();
    f.household
        .create_entry(
            f.admin.id,
            EntryInput {
                description: "Conta de Água".into(),
                ..f.expense_input(100, "2026-11-10", "Contas e serviços")
            },
        )
        .unwrap();
    f.expense(&f.admin, 200, "2026-11-10", "Mercado");

    let found = f
        .household
        .entries(
            f.admin.id,
            &EntryFilter {
                search: Some(" água ".into()),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].description, "Conta de Água");
}

#[test]
fn new_entry_is_planned_when_dated_in_the_future_and_settled_otherwise() {
    let f = fixture();

    let yesterday = f.expense(&f.admin, 100, "2026-11-10", "Mercado");
    let today = f.expense(&f.admin, 100, "2026-11-11", "Mercado");
    let tomorrow = f.expense(&f.admin, 100, "2026-11-12", "Mercado");

    assert_eq!(yesterday.status, EntryStatus::Settled);
    assert_eq!(today.status, EntryStatus::Settled);
    assert_eq!(tomorrow.status, EntryStatus::Planned);
    assert!(!tomorrow.overdue);
}

#[test]
fn status_can_be_chosen_explicitly() {
    let f = fixture();

    let planned_in_the_past = f
        .household
        .create_entry(
            f.admin.id,
            EntryInput {
                status: Some(EntryStatus::Planned),
                ..f.expense_input(100, "2026-11-01", "Mercado")
            },
        )
        .unwrap();

    assert_eq!(planned_in_the_past.status, EntryStatus::Planned);
    assert!(planned_in_the_past.overdue);
}

#[test]
fn planned_entry_becomes_overdue_instead_of_settling_itself() {
    let f = fixture();
    let entry = f.expense(&f.admin, 20_000, "2026-11-15", "Contas e serviços");

    f.clock.advance(chrono::Duration::days(4));
    let due_today = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap();
    f.clock.advance(chrono::Duration::days(1));
    let day_after = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap();

    assert_eq!(due_today[0].id, entry.id);
    assert!(!due_today[0].overdue);
    assert_eq!(day_after[0].status, EntryStatus::Planned);
    assert!(day_after[0].overdue);
}

#[test]
fn settling_records_the_real_amount() {
    let f = fixture();
    let bill = f.expense(&f.admin, 20_000, "2026-11-15", "Contas e serviços");
    let rent = f.expense(&f.admin, 150_000, "2026-11-20", "Moradia");

    let bill = f
        .household
        .settle_entry(f.admin.id, bill.id, Some(23_100))
        .unwrap();
    let rent = f.household.settle_entry(f.admin.id, rent.id, None).unwrap();

    assert_eq!(bill.status, EntryStatus::Settled);
    assert_eq!(bill.amount_cents, 23_100);
    assert!(!bill.overdue);
    assert_eq!(bill.version, 2);
    assert_eq!(rent.amount_cents, 150_000);
    assert!(matches!(
        f.household.settle_entry(f.admin.id, rent.id, Some(0)),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn common_member_cannot_settle_entries_of_others() {
    let f = fixture();
    let ana = f.join("ana");
    let entry = f.expense(&f.admin, 100, "2026-11-20", "Mercado");

    assert!(matches!(
        f.household.settle_entry(ana.id, entry.id, None),
        Err(Error::Forbidden)
    ));
}

#[test]
fn editing_keeps_the_status_unless_told_otherwise() {
    let f = fixture();
    let entry = f.expense(&f.admin, 100, "2026-11-20", "Mercado");

    let moved = f
        .household
        .update_entry(
            f.admin.id,
            entry.id,
            f.expense_input(100, "2026-11-05", "Mercado"),
        )
        .unwrap();
    let settled = f
        .household
        .update_entry(
            f.admin.id,
            entry.id,
            EntryInput {
                status: Some(EntryStatus::Settled),
                ..f.expense_input(100, "2026-11-05", "Mercado")
            },
        )
        .unwrap();

    assert_eq!(moved.status, EntryStatus::Planned);
    assert!(moved.overdue);
    assert_eq!(settled.status, EntryStatus::Settled);
}

#[test]
fn entries_can_be_filtered_by_status() {
    let f = fixture();
    f.expense(&f.admin, 1, "2026-11-10", "Mercado");
    f.expense(&f.admin, 2, "2026-11-20", "Mercado");

    let planned = f
        .household
        .entries(
            f.admin.id,
            &EntryFilter {
                status: Some(EntryStatus::Planned),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].amount_cents, 2);
}
