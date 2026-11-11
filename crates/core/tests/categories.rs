mod common;

use common::fixture;
use finance_core::{EntryKind, Error, NewCategory};

fn pets() -> NewCategory {
    NewCategory {
        name: "Pets".into(),
        kind: EntryKind::Expense,
    }
}

#[test]
fn admin_creates_and_renames_categories() {
    let f = fixture();

    let created = f.household.create_category(f.admin.id, pets()).unwrap();
    let renamed = f
        .household
        .rename_category(f.admin.id, created.id, " Animais ")
        .unwrap();

    assert_eq!(renamed.name, "Animais");
    assert_eq!(f.category("Animais"), created.id);
}

#[test]
fn category_names_are_unique_within_a_kind() {
    let f = fixture();

    let duplicate = NewCategory {
        name: "mercado".into(),
        kind: EntryKind::Expense,
    };
    let same_name_other_kind = NewCategory {
        name: "Mercado".into(),
        kind: EntryKind::Income,
    };

    assert!(matches!(
        f.household.create_category(f.admin.id, duplicate),
        Err(Error::Invalid(_))
    ));
    assert!(
        f.household
            .create_category(f.admin.id, same_name_other_kind)
            .is_ok()
    );
    assert!(matches!(
        f.household
            .rename_category(f.admin.id, f.category("Lazer"), "Moradia"),
        Err(Error::Invalid(_))
    ));
    assert!(
        f.household
            .rename_category(f.admin.id, f.category("Lazer"), "LAZER")
            .is_ok()
    );
}

#[test]
fn common_member_sees_categories_but_cannot_change_them() {
    let f = fixture();
    let ana = f.join("ana");
    let lazer = f.category("Lazer");

    assert!(!f.household.categories(ana.id).unwrap().is_empty());
    assert!(matches!(
        f.household.create_category(ana.id, pets()),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.rename_category(ana.id, lazer, "Diversão"),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.household.set_category_archived(ana.id, lazer, true),
        Err(Error::Forbidden)
    ));
}

#[test]
fn archived_category_stays_on_old_entries_but_takes_no_new_ones() {
    let f = fixture();
    let old = f.expense(&f.admin, 5000, "2026-11-01", "Lazer");

    f.household
        .set_category_archived(f.admin.id, f.category("Lazer"), true)
        .unwrap();

    let listed = f
        .household
        .entries(f.admin.id, &Default::default())
        .unwrap();
    assert_eq!(listed[0].category_name, "Lazer");
    assert!(matches!(
        f.household
            .create_entry(f.admin.id, f.expense_input(100, "2026-11-02", "Lazer")),
        Err(Error::Invalid(_))
    ));

    // Corrigir o valor de um lançamento antigo não obriga a trocar a categoria.
    let edited = f
        .household
        .update_entry(
            f.admin.id,
            old.id,
            f.expense_input(6000, "2026-11-01", "Lazer"),
        )
        .unwrap();
    assert_eq!(edited.amount_cents, 6000);
}

#[test]
fn archived_category_can_be_restored() {
    let f = fixture();
    let lazer = f.category("Lazer");
    f.household
        .set_category_archived(f.admin.id, lazer, true)
        .unwrap();

    let restored = f
        .household
        .set_category_archived(f.admin.id, lazer, false)
        .unwrap();

    assert!(!restored.archived);
    f.expense(&f.admin, 100, "2026-11-02", "Lazer");
}
