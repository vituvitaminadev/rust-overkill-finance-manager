mod common;

use chrono::Duration;
use common::{Fixture, day, fixture, month};
use finance_core::{
    Entry, EntryFilter, EntryInput, EntryKind, EntryStatus, Error, InstallmentPlanInput, Member,
    RecurrenceInput,
};

// O relógio dos testes marca 11/11/2026.

fn rent(f: &Fixture) -> RecurrenceInput {
    RecurrenceInput {
        kind: EntryKind::Expense,
        amount_cents: 150_000,
        description: "Aluguel".into(),
        category_id: f.category("Moradia"),
        assignee_id: None,
        first_date: day("2026-11-20"),
        end_month: None,
    }
}

fn sofa(f: &Fixture) -> InstallmentPlanInput {
    InstallmentPlanInput {
        description: "Sofá".into(),
        total_cents: 100_000,
        count: 3,
        first_date: day("2026-11-25"),
        category_id: f.category("Moradia"),
        assignee_id: None,
    }
}

/// Os lançamentos com essa descrição, do mais antigo ao mais recente.
fn entries_named(f: &Fixture, viewer: &Member, description: &str) -> Vec<Entry> {
    let mut entries = f
        .household
        .entries(
            viewer.id,
            &EntryFilter {
                search: Some(description.into()),
                ..Default::default()
            },
        )
        .unwrap();
    entries.reverse();
    entries
}

#[test]
fn recurrence_generates_planned_entries_twelve_months_ahead() {
    let f = fixture();

    let recurrence = f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();

    let entries = entries_named(&f, &f.admin, "Aluguel");
    assert_eq!(recurrence.day_of_month, 20);
    assert_eq!(entries.len(), 13);
    assert_eq!(entries[0].date, day("2026-11-20"));
    assert_eq!(entries[12].date, day("2027-11-20"));
    assert!(entries.iter().all(|e| e.status == EntryStatus::Planned));
    assert!(entries.iter().all(|e| e.amount_cents == 150_000));
    assert!(
        entries
            .iter()
            .all(|e| e.recurrence_id == Some(recurrence.id))
    );
    assert!(entries.iter().all(|e| e.assignee_id == f.admin.id));
}

#[test]
fn recurrence_keeps_the_horizon_as_time_passes() {
    let f = fixture();
    f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();

    f.clock.advance(Duration::days(61));

    let entries = entries_named(&f, &f.admin, "Aluguel");
    assert_eq!(f.clock_month(), month("2027-01"));
    assert_eq!(entries.len(), 15);
    assert_eq!(entries.last().unwrap().date, day("2028-01-20"));
    // Os que ficaram para trás continuam previstos, agora atrasados.
    assert!(entries[0].overdue);
}

#[test]
fn recurrence_stops_at_its_end_month() {
    let f = fixture();

    f.household
        .create_recurrence(
            f.admin.id,
            RecurrenceInput {
                end_month: Some(month("2027-02")),
                ..rent(&f)
            },
        )
        .unwrap();
    f.clock.advance(Duration::days(200));

    let entries = entries_named(&f, &f.admin, "Aluguel");
    assert_eq!(entries.len(), 4);
    assert_eq!(entries.last().unwrap().date, day("2027-02-20"));
    // Passado o fim, ela sai da lista das que estão em vigor.
    assert!(f.household.recurrences(f.admin.id).unwrap().is_empty());
}

#[test]
fn recurrence_on_day_31_falls_on_the_last_day_of_shorter_months() {
    let f = fixture();

    f.household
        .create_recurrence(
            f.admin.id,
            RecurrenceInput {
                first_date: day("2026-12-31"),
                ..rent(&f)
            },
        )
        .unwrap();

    let dates: Vec<_> = entries_named(&f, &f.admin, "Aluguel")
        .iter()
        .take(4)
        .map(|e| e.date)
        .collect();
    assert_eq!(
        dates,
        vec![
            day("2026-12-31"),
            day("2027-01-31"),
            day("2027-02-28"),
            day("2027-03-31")
        ]
    );
}

#[test]
fn recurrence_cannot_end_before_it_starts() {
    let f = fixture();

    let result = f.household.create_recurrence(
        f.admin.id,
        RecurrenceInput {
            end_month: Some(month("2026-10")),
            ..rent(&f)
        },
    );

    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[test]
fn editing_only_one_occurrence_leaves_the_others_alone() {
    let f = fixture();
    f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();
    let december = entries_named(&f, &f.admin, "Aluguel")[1].clone();

    f.household
        .update_entry(
            f.admin.id,
            december.id,
            f.expense_input(170_000, "2026-12-22", "Moradia"),
        )
        .unwrap();

    let amounts: Vec<i64> = f
        .household
        .entries(
            f.admin.id,
            &EntryFilter {
                category_id: Some(f.category("Moradia")),
                ..Default::default()
            },
        )
        .unwrap()
        .iter()
        .rev()
        .take(3)
        .map(|e| e.amount_cents)
        .collect();
    assert_eq!(amounts, vec![150_000, 170_000, 150_000]);
}

#[test]
fn editing_this_and_following_changes_the_rule_from_there_on() {
    let f = fixture();
    f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();
    let all = entries_named(&f, &f.admin, "Aluguel");
    // Novembro já foi pago, e janeiro foi combinado à parte.
    f.household
        .settle_entry(f.admin.id, all[0].id, None)
        .unwrap();
    f.household
        .update_entry(
            f.admin.id,
            all[2].id,
            EntryInput {
                description: "Aluguel".into(),
                ..f.expense_input(140_000, "2027-01-20", "Moradia")
            },
        )
        .unwrap();

    // Reajuste a partir de dezembro, com vencimento no dia 5.
    let december = f
        .household
        .update_entry_and_following(
            f.admin.id,
            all[1].id,
            EntryInput {
                description: "Aluguel".into(),
                ..f.expense_input(165_000, "2026-12-05", "Moradia")
            },
        )
        .unwrap();

    let after = entries_named(&f, &f.admin, "Aluguel");
    let summary: Vec<(String, i64)> = after
        .iter()
        .take(4)
        .map(|e| (e.date.to_string(), e.amount_cents))
        .collect();
    assert_eq!(december.amount_cents, 165_000);
    assert_eq!(
        summary,
        vec![
            ("2026-11-20".to_owned(), 150_000),
            ("2026-12-05".to_owned(), 165_000),
            ("2027-01-20".to_owned(), 140_000),
            ("2027-02-05".to_owned(), 165_000),
        ]
    );
    assert!(after[3..].iter().all(|e| e.amount_cents == 165_000));

    // Os lançamentos gerados depois já nascem com a regra nova.
    f.clock.advance(Duration::days(31));
    let newest = entries_named(&f, &f.admin, "Aluguel").pop().unwrap();
    assert_eq!(newest.date, day("2027-12-05"));
    assert_eq!(newest.amount_cents, 165_000);
    assert_eq!(
        f.household.recurrences(f.admin.id).unwrap()[0].day_of_month,
        5
    );
}

#[test]
fn this_and_following_needs_an_entry_from_a_recurrence() {
    let f = fixture();
    let loose = f.expense(&f.admin, 100, "2026-11-20", "Mercado");

    let result = f.household.update_entry_and_following(
        f.admin.id,
        loose.id,
        f.expense_input(200, "2026-11-20", "Mercado"),
    );

    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[test]
fn deleted_occurrence_is_not_generated_again() {
    let f = fixture();
    f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();
    let december = entries_named(&f, &f.admin, "Aluguel")[1].clone();

    f.household.delete_entry(f.admin.id, december.id).unwrap();
    f.clock.advance(Duration::days(40));

    let dates: Vec<_> = entries_named(&f, &f.admin, "Aluguel")
        .iter()
        .map(|e| e.date)
        .collect();
    assert!(!dates.contains(&day("2026-12-20")));
    assert_eq!(dates.len(), 13);
}

#[test]
fn ending_a_recurrence_removes_future_planned_entries_only() {
    let f = fixture();
    let recurrence = f
        .household
        .create_recurrence(
            f.admin.id,
            RecurrenceInput {
                first_date: day("2026-09-20"),
                ..rent(&f)
            },
        )
        .unwrap();
    let all = entries_named(&f, &f.admin, "Aluguel");
    // Setembro foi pago; outubro ficou atrasado; novembro em diante é futuro.
    f.household
        .settle_entry(f.admin.id, all[0].id, None)
        .unwrap();

    f.household
        .end_recurrence(f.admin.id, recurrence.id)
        .unwrap();
    f.clock.advance(Duration::days(90));

    let left: Vec<(String, EntryStatus)> = entries_named(&f, &f.admin, "Aluguel")
        .iter()
        .map(|e| (e.date.to_string(), e.status))
        .collect();
    assert_eq!(
        left,
        vec![
            ("2026-09-20".to_owned(), EntryStatus::Settled),
            ("2026-10-20".to_owned(), EntryStatus::Planned),
        ]
    );
    assert!(f.household.recurrences(f.admin.id).unwrap().is_empty());
}

#[test]
fn recurrences_follow_the_assignee_rules() {
    let f = fixture();
    let ana = f.join("ana");

    let for_someone_else = f.household.create_recurrence(
        ana.id,
        RecurrenceInput {
            assignee_id: Some(f.admin.id),
            ..rent(&f)
        },
    );
    let admins = f.household.create_recurrence(f.admin.id, rent(&f)).unwrap();
    let anas = f
        .household
        .create_recurrence(
            f.admin.id,
            RecurrenceInput {
                description: "Academia".into(),
                assignee_id: Some(ana.id),
                ..rent(&f)
            },
        )
        .unwrap();

    assert!(matches!(for_someone_else, Err(Error::Forbidden)));
    assert!(matches!(
        f.household.end_recurrence(ana.id, admins.id),
        Err(Error::Forbidden)
    ));
    let occurrence = entries_named(&f, &ana, "Aluguel")[0].clone();
    assert!(matches!(
        f.household.update_entry_and_following(
            ana.id,
            occurrence.id,
            f.expense_input(1, "2026-11-20", "Moradia")
        ),
        Err(Error::Forbidden)
    ));

    let seen_by_ana = f.household.recurrences(ana.id).unwrap();
    let editable: Vec<(&str, bool)> = seen_by_ana
        .iter()
        .map(|r| (r.description.as_str(), r.can_edit))
        .collect();
    assert_eq!(editable, vec![("Academia", true), ("Aluguel", false)]);
    assert!(f.household.end_recurrence(ana.id, anas.id).is_ok());
}

#[test]
fn installment_plan_spreads_the_purchase_over_consecutive_months() {
    let f = fixture();

    let plan = f
        .household
        .create_installment_plan(f.admin.id, sofa(&f))
        .unwrap();

    let parcels = entries_named(&f, &f.admin, "Sofá");
    let summary: Vec<(String, i64, u32, u32)> = parcels
        .iter()
        .map(|e| {
            let installment = e.installment.as_ref().unwrap();
            (
                e.date.to_string(),
                e.amount_cents,
                installment.number,
                installment.count,
            )
        })
        .collect();
    // Os centavos que sobram da divisão ficam na primeira parcela.
    assert_eq!(
        summary,
        vec![
            ("2026-11-25".to_owned(), 33_334, 1, 3),
            ("2026-12-25".to_owned(), 33_333, 2, 3),
            ("2027-01-25".to_owned(), 33_333, 3, 3),
        ]
    );
    assert!(parcels.iter().all(|e| e.kind == EntryKind::Expense));
    assert!(parcels.iter().all(|e| e.status == EntryStatus::Planned));
    assert_eq!(plan.planned_count, 3);
    assert_eq!(plan.planned_cents, 100_000);
}

#[test]
fn installment_plan_tracks_what_is_still_planned() {
    let f = fixture();
    let plan = f
        .household
        .create_installment_plan(f.admin.id, sofa(&f))
        .unwrap();
    let first = entries_named(&f, &f.admin, "Sofá")[0].clone();

    f.household
        .settle_entry(f.admin.id, first.id, None)
        .unwrap();

    let plans = f.household.installment_plans(f.admin.id).unwrap();
    assert_eq!(plans[0].id, plan.id);
    assert_eq!(plans[0].planned_count, 2);
    assert_eq!(plans[0].planned_cents, 66_666);
}

#[test]
fn deleting_an_installment_plan_keeps_settled_parcels() {
    let f = fixture();
    let plan = f
        .household
        .create_installment_plan(f.admin.id, sofa(&f))
        .unwrap();
    let first = entries_named(&f, &f.admin, "Sofá")[0].clone();
    f.household
        .settle_entry(f.admin.id, first.id, None)
        .unwrap();

    f.household
        .delete_installment_plan(f.admin.id, plan.id)
        .unwrap();

    let left = entries_named(&f, &f.admin, "Sofá");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].status, EntryStatus::Settled);
    assert_eq!(left[0].installment.as_ref().unwrap().number, 1);
    assert!(
        f.household
            .installment_plans(f.admin.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_installment_plans_are_rejected() {
    let f = fixture();
    let create = |input| f.household.create_installment_plan(f.admin.id, input);

    let single = InstallmentPlanInput {
        count: 1,
        ..sofa(&f)
    };
    let too_small = InstallmentPlanInput {
        total_cents: 2,
        ..sofa(&f)
    };
    let income_category = InstallmentPlanInput {
        category_id: f.category("Salário"),
        ..sofa(&f)
    };

    for input in [single, too_small, income_category] {
        assert!(matches!(create(input), Err(Error::Invalid(_))));
    }
    assert!(entries_named(&f, &f.admin, "Sofá").is_empty());
}

#[test]
fn installment_plans_follow_the_assignee_rules() {
    let f = fixture();
    let ana = f.join("ana");
    let plan = f
        .household
        .create_installment_plan(f.admin.id, sofa(&f))
        .unwrap();

    let for_someone_else = f.household.create_installment_plan(
        ana.id,
        InstallmentPlanInput {
            assignee_id: Some(f.admin.id),
            ..sofa(&f)
        },
    );

    assert!(matches!(for_someone_else, Err(Error::Forbidden)));
    assert!(matches!(
        f.household.delete_installment_plan(ana.id, plan.id),
        Err(Error::Forbidden)
    ));
    assert!(!f.household.installment_plans(ana.id).unwrap()[0].can_edit);
}
