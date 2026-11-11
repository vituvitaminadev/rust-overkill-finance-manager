//! Os comandos expostos ao frontend. Cada um só traduz a chamada para a
//! interface de aplicação: regras e permissões ficam em `finance-core`.
//!
//! Todos são `async` para rodar fora da thread principal, já que conferir uma
//! senha leva tempo suficiente para travar a janela.

use std::sync::{Mutex, MutexGuard};

use finance_core::{
    Category, CategoryId, Dashboard, Entry, EntryFilter, EntryId, EntryInput, HouseholdId, Member,
    Month, NewCategory,
};
use tauri::State;

use crate::desk::{Desk, Status};
use crate::error::CommandError;

type Result<T> = std::result::Result<T, CommandError>;

pub struct AppState(pub Mutex<Desk>);

impl AppState {
    fn desk(&self) -> MutexGuard<'_, Desk> {
        // Um comando que entrou em pânico não deve inutilizar o app inteiro.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[tauri::command(async)]
pub fn status(state: State<'_, AppState>) -> Result<Status> {
    state.desk().status()
}

#[tauri::command(async)]
pub fn login(
    state: State<'_, AppState>,
    username: String,
    password: String,
    remember: bool,
) -> Result<Status> {
    let mut desk = state.desk();
    desk.login(&username, &password, remember)?;
    desk.status()
}

#[tauri::command(async)]
pub fn logout(state: State<'_, AppState>) -> Result<Status> {
    let mut desk = state.desk();
    desk.logout();
    desk.status()
}

#[tauri::command(async)]
pub fn create_household(
    state: State<'_, AppState>,
    name: String,
    username: Option<String>,
    password: Option<String>,
    remember: bool,
) -> Result<Status> {
    let mut desk = state.desk();
    desk.create_household(name, username.zip(password), remember)?;
    desk.status()
}

#[tauri::command(async)]
pub fn select_household(state: State<'_, AppState>, id: HouseholdId) -> Result<Status> {
    let mut desk = state.desk();
    desk.select_household(id)?;
    desk.status()
}

#[tauri::command(async)]
pub fn close_household(state: State<'_, AppState>) -> Result<Status> {
    let mut desk = state.desk();
    desk.close_household();
    desk.status()
}

#[tauri::command(async)]
pub fn members(state: State<'_, AppState>) -> Result<Vec<Member>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.members(actor)?)
}

#[tauri::command(async)]
pub fn categories(state: State<'_, AppState>) -> Result<Vec<Category>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.categories(actor)?)
}

#[tauri::command(async)]
pub fn create_category(state: State<'_, AppState>, input: NewCategory) -> Result<Category> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.create_category(actor, input)?)
}

#[tauri::command(async)]
pub fn rename_category(
    state: State<'_, AppState>,
    id: CategoryId,
    name: String,
) -> Result<Category> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.rename_category(actor, id, &name)?)
}

#[tauri::command(async)]
pub fn set_category_archived(
    state: State<'_, AppState>,
    id: CategoryId,
    archived: bool,
) -> Result<Category> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.set_category_archived(actor, id, archived)?)
}

#[tauri::command(async)]
pub fn entries(state: State<'_, AppState>, filter: EntryFilter) -> Result<Vec<Entry>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.entries(actor, &filter)?)
}

#[tauri::command(async)]
pub fn create_entry(state: State<'_, AppState>, input: EntryInput) -> Result<Entry> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.create_entry(actor, input)?)
}

#[tauri::command(async)]
pub fn update_entry(state: State<'_, AppState>, id: EntryId, input: EntryInput) -> Result<Entry> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.update_entry(actor, id, input)?)
}

#[tauri::command(async)]
pub fn delete_entry(state: State<'_, AppState>, id: EntryId) -> Result<()> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.delete_entry(actor, id)?)
}

#[tauri::command(async)]
pub fn dashboard(state: State<'_, AppState>, month: Month) -> Result<Dashboard> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.dashboard(actor, month)?)
}
