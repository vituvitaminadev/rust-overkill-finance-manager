//! Os comandos expostos ao frontend. Cada um só traduz a chamada para a
//! interface de aplicação: regras e permissões ficam em `finance-core`.
//!
//! Todos são `async` para rodar fora da thread principal, já que conferir uma
//! senha leva tempo suficiente para travar a janela.

use std::sync::{Mutex, MutexGuard};

use finance_core::{
    Category, Dashboard, Entry, EntryFilter, EntryId, EntryInput, HostCommand, HouseholdId,
    InstallmentPlan, Member, Month, Recurrence,
};
use serde_json::Value;
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

/// Tudo o que mexe na estrutura da residência (categorias, membros,
/// recorrências, parcelamentos) entra por aqui, como um comando só.
#[tauri::command(async)]
pub fn host_command(state: State<'_, AppState>, command: HostCommand) -> Result<Value> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.execute(actor, command)?)
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
pub fn entries(state: State<'_, AppState>, filter: EntryFilter) -> Result<Vec<Entry>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.entries(actor, &filter)?)
}

#[tauri::command(async)]
pub fn dashboard(state: State<'_, AppState>, month: Month) -> Result<Dashboard> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.dashboard(actor, month)?)
}

#[tauri::command(async)]
pub fn recurrences(state: State<'_, AppState>) -> Result<Vec<Recurrence>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.recurrences(actor)?)
}

#[tauri::command(async)]
pub fn installment_plans(state: State<'_, AppState>) -> Result<Vec<InstallmentPlan>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.installment_plans(actor)?)
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
pub fn settle_entry(
    state: State<'_, AppState>,
    id: EntryId,
    amount_cents: Option<i64>,
) -> Result<Entry> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.settle_entry(actor, id, amount_cents)?)
}

#[tauri::command(async)]
pub fn delete_entry(state: State<'_, AppState>, id: EntryId) -> Result<()> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.delete_entry(actor, id)?)
}
