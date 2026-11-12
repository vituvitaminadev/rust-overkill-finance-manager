mod common;

use chrono::Duration;
use common::{fixture, month};
use finance_core::{EntryInput, EntryStatus, Totals};

fn totals(settled_cents: i64, planned_cents: i64) -> Totals {
    Totals {
        settled_cents,
        planned_cents,
    }
}

// O relógio dos testes marca 11/11/2026: até esse dia um lançamento nasce
// Efetivado, e depois dele, Previsto.

#[test]
fn month_cards_separate_what_happened_from_what_is_planned() {
    let f = fixture();
    let ana = f.join("ana");
    f.income(&f.admin, 500_000, "2026-11-05");
    f.income(&ana, 300_000, "2026-11-30");
    f.expense(&f.admin, 150_000, "2026-11-10", "Moradia");
    f.expense(&ana, 45_050, "2026-11-11", "Mercado");
    f.expense(&ana, 20_000, "2026-11-25", "Contas e serviços");
    f.expense(&ana, 99_999, "2026-12-01", "Mercado");
    f.income(&ana, 99_999, "2026-10-31");

    let dashboard = f.household.dashboard(ana.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.income, totals(500_000, 300_000));
    assert_eq!(dashboard.expense, totals(195_050, 20_000));
    assert_eq!(dashboard.balance, totals(304_950, 280_000));
    assert_eq!(dashboard.balance.total_cents(), 584_950);
}

#[test]
fn balance_goes_negative_when_expenses_exceed_income() {
    let f = fixture();
    f.income(&f.admin, 1_000, "2026-11-05");
    f.expense(&f.admin, 2_500, "2026-11-10", "Mercado");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.balance, totals(-1_500, 0));
}

#[test]
fn expenses_are_broken_down_by_category_largest_first() {
    let f = fixture();
    f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    f.expense(&f.admin, 700, "2026-11-02", "Mercado");
    f.expense(&f.admin, 300, "2026-11-28", "Mercado");
    f.expense(&f.admin, 500, "2026-11-04", "Moradia");
    f.expense(&f.admin, 450, "2026-11-29", "Transporte");
    f.income(&f.admin, 9_000, "2026-11-05");
    f.expense(&f.admin, 9_000, "2026-10-04", "Transporte");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    let breakdown: Vec<(&str, Totals)> = dashboard
        .expenses_by_category
        .iter()
        .map(|c| (c.name.as_str(), c.totals))
        .collect();
    assert_eq!(
        breakdown,
        vec![
            ("Mercado", totals(700, 300)),
            ("Moradia", totals(500, 0)),
            ("Transporte", totals(0, 450)),
            ("Lazer", totals(100, 0)),
        ]
    );
}

#[test]
fn expenses_are_broken_down_by_assignee() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    f.expense(&ana, 700, "2026-11-02", "Mercado");
    f.expense(&ana, 50, "2026-11-20", "Lazer");
    f.income(&f.admin, 9_000, "2026-11-05");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    let breakdown: Vec<(&str, Totals)> = dashboard
        .expenses_by_assignee
        .iter()
        .map(|a| (a.name.as_str(), a.totals))
        .collect();
    assert_eq!(
        breakdown,
        vec![("ana", totals(700, 50)), ("vitu", totals(100, 0))]
    );
}

#[test]
fn flow_covers_five_months_back_and_six_ahead_of_the_selected_one() {
    let f = fixture();
    f.income(&f.admin, 1_000, "2026-06-15");
    f.expense(&f.admin, 400, "2026-06-20", "Mercado");
    // Julho a setembro ficam sem lançamentos.
    f.expense(&f.admin, 900, "2026-10-31", "Mercado");
    f.income(&f.admin, 2_000, "2026-11-01");
    f.expense(&f.admin, 500, "2026-11-30", "Lazer");
    f.income(&f.admin, 3_000, "2027-05-10");
    // Fora da janela, dos dois lados.
    f.income(&f.admin, 77_777, "2026-05-31");
    f.income(&f.admin, 77_777, "2027-06-01");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    let months: Vec<String> = dashboard.flow.iter().map(|m| m.month.to_string()).collect();
    assert_eq!(months.first().unwrap(), "2026-06");
    assert_eq!(months.last().unwrap(), "2027-05");
    assert_eq!(months.len(), 12);

    let at = |text: &str| {
        dashboard
            .flow
            .iter()
            .find(|m| m.month == month(text))
            .unwrap()
    };
    assert_eq!(at("2026-06").income, totals(1_000, 0));
    assert_eq!(at("2026-06").expense, totals(400, 0));
    assert_eq!(at("2026-06").accumulated_balance_cents, 600);
    assert_eq!(at("2026-08").income, totals(0, 0));
    assert_eq!(at("2026-08").accumulated_balance_cents, 600);
    assert_eq!(at("2026-10").accumulated_balance_cents, -300);
    assert_eq!(at("2026-11").income, totals(2_000, 0));
    assert_eq!(at("2026-11").expense, totals(0, 500));
    // O saldo acumulado conta com o que está previsto.
    assert_eq!(at("2026-11").accumulated_balance_cents, 1_200);
    assert_eq!(at("2027-05").income, totals(0, 3_000));
    assert_eq!(at("2027-05").accumulated_balance_cents, 4_200);
}

#[test]
fn upcoming_and_overdue_lists_follow_the_calendar() {
    let f = fixture();
    let settled = f.expense(&f.admin, 1, "2026-11-10", "Mercado");
    let soon = f.expense(&f.admin, 2, "2026-11-12", "Mercado");
    let later = f.expense(&f.admin, 3, "2026-12-20", "Mercado");
    let forgotten = f
        .household
        .create_entry(
            f.admin.id,
            EntryInput {
                status: Some(EntryStatus::Planned),
                ..f.expense_input(4, "2026-11-03", "Mercado")
            },
        )
        .unwrap();

    let ids = |entries: &[finance_core::Entry]| entries.iter().map(|e| e.id).collect::<Vec<_>>();
    // As listas não dependem do mês consultado.
    let before = f.household.dashboard(f.admin.id, month("2026-01")).unwrap();
    assert_eq!(ids(&before.upcoming), vec![soon.id, later.id]);
    assert_eq!(ids(&before.overdue), vec![forgotten.id]);

    f.clock.advance(Duration::days(2));
    let after = f.household.dashboard(f.admin.id, month("2026-01")).unwrap();
    assert_eq!(ids(&after.upcoming), vec![later.id]);
    assert_eq!(ids(&after.overdue), vec![forgotten.id, soon.id]);

    f.household.settle_entry(f.admin.id, soon.id, None).unwrap();
    let settled_one = f.household.dashboard(f.admin.id, month("2026-01")).unwrap();
    assert_eq!(ids(&settled_one.overdue), vec![forgotten.id]);
    assert!(!ids(&settled_one.upcoming).contains(&settled.id));
}

#[test]
fn deleted_entries_do_not_count() {
    let f = fixture();
    let kept = f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    let deleted = f.expense(&f.admin, 900, "2026-11-02", "Mercado");
    let deleted_planned = f.expense(&f.admin, 900, "2026-11-20", "Mercado");
    f.household.delete_entry(f.admin.id, deleted.id).unwrap();
    f.household
        .delete_entry(f.admin.id, deleted_planned.id)
        .unwrap();

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.expense, totals(kept.amount_cents, 0));
    assert_eq!(dashboard.expenses_by_category.len(), 1);
    assert!(dashboard.upcoming.is_empty());
}

#[test]
fn empty_month_shows_zeros() {
    let f = fixture();

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.income, totals(0, 0));
    assert_eq!(dashboard.expense, totals(0, 0));
    assert_eq!(dashboard.balance, totals(0, 0));
    assert!(dashboard.expenses_by_category.is_empty());
    assert!(dashboard.expenses_by_assignee.is_empty());
    assert!(dashboard.upcoming.is_empty());
    assert!(dashboard.overdue.is_empty());
    assert_eq!(dashboard.flow.len(), 12);
}
