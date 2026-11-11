mod common;

use common::fixture;
use finance_core::Month;

fn month(text: &str) -> Month {
    text.parse().unwrap()
}

#[test]
fn month_cards_show_income_expenses_and_balance() {
    let f = fixture();
    let ana = f.join("ana");
    f.income(&f.admin, 500_000, "2026-11-05");
    f.income(&ana, 300_000, "2026-11-06");
    f.expense(&f.admin, 150_000, "2026-11-10", "Moradia");
    f.expense(&ana, 45_050, "2026-11-30", "Mercado");
    f.expense(&ana, 99_999, "2026-12-01", "Mercado");
    f.income(&ana, 99_999, "2026-10-31");

    let dashboard = f.household.dashboard(ana.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.income_cents, 800_000);
    assert_eq!(dashboard.expense_cents, 195_050);
    assert_eq!(dashboard.balance_cents, 604_950);
}

#[test]
fn balance_goes_negative_when_expenses_exceed_income() {
    let f = fixture();
    f.income(&f.admin, 1_000, "2026-11-05");
    f.expense(&f.admin, 2_500, "2026-11-10", "Mercado");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.balance_cents, -1_500);
}

#[test]
fn expenses_are_broken_down_by_category_largest_first() {
    let f = fixture();
    f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    f.expense(&f.admin, 700, "2026-11-02", "Mercado");
    f.expense(&f.admin, 300, "2026-11-03", "Mercado");
    f.expense(&f.admin, 500, "2026-11-04", "Moradia");
    f.income(&f.admin, 9_000, "2026-11-05");
    f.expense(&f.admin, 9_000, "2026-10-04", "Transporte");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    let breakdown: Vec<(&str, i64)> = dashboard
        .expenses_by_category
        .iter()
        .map(|c| (c.name.as_str(), c.total_cents))
        .collect();
    assert_eq!(
        breakdown,
        vec![("Mercado", 1_000), ("Moradia", 500), ("Lazer", 100)]
    );
}

#[test]
fn expenses_are_broken_down_by_assignee() {
    let f = fixture();
    let ana = f.join("ana");
    f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    f.expense(&ana, 700, "2026-11-02", "Mercado");
    f.expense(&ana, 50, "2026-11-03", "Lazer");
    f.income(&f.admin, 9_000, "2026-11-05");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    let breakdown: Vec<(&str, i64)> = dashboard
        .expenses_by_assignee
        .iter()
        .map(|a| (a.name.as_str(), a.total_cents))
        .collect();
    assert_eq!(breakdown, vec![("ana", 750), ("vitu", 100)]);
}

#[test]
fn flow_covers_the_six_months_ending_in_the_selected_one() {
    let f = fixture();
    f.income(&f.admin, 1_000, "2025-09-15");
    f.expense(&f.admin, 400, "2025-09-20", "Mercado");
    // Outubro e novembro ficam sem lançamentos.
    f.expense(&f.admin, 900, "2025-12-31", "Mercado");
    f.income(&f.admin, 2_000, "2026-01-01");
    f.income(&f.admin, 500, "2026-02-10");
    f.expense(&f.admin, 100, "2026-02-11", "Lazer");
    // Fora da janela, dos dois lados.
    f.income(&f.admin, 77_777, "2025-08-31");
    f.income(&f.admin, 77_777, "2026-03-01");

    let dashboard = f.household.dashboard(f.admin.id, month("2026-02")).unwrap();

    let flow: Vec<(String, i64, i64, i64)> = dashboard
        .flow
        .iter()
        .map(|m| {
            (
                m.month.to_string(),
                m.income_cents,
                m.expense_cents,
                m.accumulated_balance_cents,
            )
        })
        .collect();
    assert_eq!(
        flow,
        vec![
            ("2025-09".to_owned(), 1_000, 400, 600),
            ("2025-10".to_owned(), 0, 0, 600),
            ("2025-11".to_owned(), 0, 0, 600),
            ("2025-12".to_owned(), 0, 900, -300),
            ("2026-01".to_owned(), 2_000, 0, 1_700),
            ("2026-02".to_owned(), 500, 100, 2_100),
        ]
    );
}

#[test]
fn deleted_entries_do_not_count() {
    let f = fixture();
    let kept = f.expense(&f.admin, 100, "2026-11-01", "Lazer");
    let deleted = f.expense(&f.admin, 900, "2026-11-02", "Mercado");
    f.household.delete_entry(f.admin.id, deleted.id).unwrap();

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.expense_cents, kept.amount_cents);
    assert_eq!(dashboard.expenses_by_category.len(), 1);
    assert_eq!(dashboard.flow.last().unwrap().expense_cents, 100);
}

#[test]
fn empty_month_shows_zeros() {
    let f = fixture();

    let dashboard = f.household.dashboard(f.admin.id, month("2026-11")).unwrap();

    assert_eq!(dashboard.income_cents, 0);
    assert_eq!(dashboard.expense_cents, 0);
    assert_eq!(dashboard.balance_cents, 0);
    assert!(dashboard.expenses_by_category.is_empty());
    assert!(dashboard.expenses_by_assignee.is_empty());
    assert_eq!(dashboard.flow.len(), 6);
}
