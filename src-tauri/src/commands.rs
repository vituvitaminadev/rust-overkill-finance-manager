//! Os comandos expostos ao frontend. Cada um só traduz a chamada para a
//! interface de aplicação ou para a rede: regras e permissões ficam em
//! `finance-core`.
//!
//! Todos são assíncronos para rodar fora da thread principal, já que conferir
//! uma senha leva tempo suficiente para travar a janela.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use finance_core::{
    Category, Conflict, Dashboard, Entry, EntryFilter, EntryId, EntryInput, HostCommand,
    HouseholdId, HouseholdInfo, InstallmentPlan, Member, Month, Recurrence, SystemClock,
};
use finance_server::{LinkState, security_code};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::desk::{Desk, Status};
use crate::error::CommandError;
use crate::net::{Discovered, Network, local_ip};

type Result<T> = std::result::Result<T, CommandError>;

pub struct AppState {
    pub desk: Mutex<Desk>,
    pub net: Network,
}

impl AppState {
    fn desk(&self) -> MutexGuard<'_, Desk> {
        // Um comando que entrou em pânico não deve inutilizar o app inteiro.
        self.desk
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Ajusta a rede à residência aberta: uma cópia ganha a ligação com o seu
    /// Anfitrião, e qualquer outro caso fica sem ligação.
    fn follow_session(&self) -> Result<Status> {
        let desk = self.desk();
        match desk.open_details() {
            Some(open) if open.replica => {
                self.net
                    .start_link(open.path, open.household, open.member, open.host_address);
            }
            _ => self.net.stop_link(),
        }
        desk.status()
    }
}

// ------------------------------------------------------------------ sessão

#[tauri::command]
pub async fn status(state: State<'_, AppState>) -> Result<Status> {
    state.follow_session()
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    username: String,
    password: String,
    remember: bool,
) -> Result<Status> {
    let missing = {
        let mut desk = state.desk();
        desk.login(&username, &password, remember)?;
        desk.missing_credentials()
    };
    // Num computador pareado por outra pessoa, a senha recém-conferida serve
    // para pedir ao Anfitrião a credencial deste membro. Se ele estiver fora
    // de alcance, fica para o próximo login.
    for copy in missing {
        if let Some(addr) = state.net.locate(copy.household, copy.host_address) {
            let _ = finance_server::sign_in(
                addr,
                &copy.path,
                Arc::new(SystemClock),
                &username,
                &password,
            )
            .await;
        }
    }
    state.follow_session()
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<Status> {
    state.desk().logout();
    state.follow_session()
}

#[tauri::command]
pub async fn create_household(
    state: State<'_, AppState>,
    name: String,
    username: Option<String>,
    password: Option<String>,
    remember: bool,
) -> Result<Status> {
    let path = state
        .desk()
        .create_household(name, username.zip(password), remember)?;
    // Sem rede a residência continua utilizável nesta máquina, então a falha
    // em servi-la não desfaz a criação.
    let _ = state.net.host(&path).await;
    state.follow_session()
}

#[tauri::command]
pub async fn select_household(state: State<'_, AppState>, id: HouseholdId) -> Result<Status> {
    state.desk().select_household(id)?;
    state.follow_session()
}

#[tauri::command]
pub async fn close_household(state: State<'_, AppState>) -> Result<Status> {
    state.desk().close_household();
    state.follow_session()
}

// -------------------------------------------------------------- pareamento

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPreview {
    household: HouseholdInfo,
    fingerprint: String,
    security_code: String,
}

/// Os Anfitriões anunciados na rede local.
#[tauri::command]
pub async fn discover(state: State<'_, AppState>) -> Result<Vec<Discovered>> {
    Ok(state.net.discovered())
}

/// Pergunta a quem atende em `address` que residência hospeda, sem confiar em nada ainda.
#[tauri::command]
pub async fn inspect_host(address: String) -> Result<HostPreview> {
    let info = finance_server::inspect(parse_address(&address)?).await?;
    Ok(HostPreview {
        household: info.household,
        security_code: security_code(&info.fingerprint),
        fingerprint: info.fingerprint,
    })
}

#[tauri::command]
pub async fn join_household(
    state: State<'_, AppState>,
    address: String,
    fingerprint: String,
    code: String,
    username: String,
    password: String,
    remember: bool,
) -> Result<Status> {
    let path = state.desk().new_file();
    let member = finance_server::join(
        parse_address(&address)?,
        &fingerprint,
        &code,
        &username,
        &password,
        &path,
        Arc::new(SystemClock),
    )
    .await?;
    state.desk().adopt(path, member, remember)?;
    state.follow_session()
}

fn parse_address(address: &str) -> Result<SocketAddr> {
    address.trim().parse().map_err(|_| {
        CommandError::new(
            "invalid",
            "Informe o endereço como aparece no Anfitrião, por exemplo 192.168.0.10:47800.",
        )
    })
}

// ----------------------------------------------------------- sincronização

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncInfo {
    /// Verdadeiro quando esta máquina tem só uma cópia da residência aberta.
    replica: bool,
    /// Na cópia: `online`, `offline`, `protocol_mismatch` ou `unauthorized`.
    link: Option<&'static str>,
    pending: i64,
    conflicts: i64,
    /// No Anfitrião: quantas cópias estão conectadas agora.
    connected: usize,
    /// No Anfitrião: onde as cópias o encontram, para quem precisar digitar.
    address: Option<String>,
    /// O código a conferir nos dois computadores ao parear.
    security_code: Option<String>,
}

#[tauri::command]
pub async fn sync_info(state: State<'_, AppState>) -> Result<SyncInfo> {
    let desk = state.desk();
    let (household, _) = desk.current()?;
    let status = household.sync_status()?;
    let replica = household.is_replica();
    let host = desk
        .open_details()
        .and_then(|open| state.net.host_status(open.household));
    let fingerprint = match &host {
        Some(host) => Some(host.fingerprint.clone()),
        None => finance_server::stored_fingerprint(household),
    };
    Ok(SyncInfo {
        replica,
        link: replica.then(|| match state.net.link_state() {
            Some(LinkState::Online) => "online",
            Some(LinkState::ProtocolMismatch) => "protocol_mismatch",
            Some(LinkState::Unauthorized) => "unauthorized",
            Some(LinkState::Offline) | None => "offline",
        }),
        pending: status.pending,
        conflicts: status.conflicts,
        connected: host.as_ref().map_or(0, |host| host.connected),
        address: host
            .as_ref()
            .and_then(|host| Some(format!("{}:{}", local_ip()?, host.port))),
        security_code: fingerprint.as_deref().map(security_code),
    })
}

#[tauri::command]
pub async fn conflicts(state: State<'_, AppState>) -> Result<Vec<Conflict>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.conflicts(actor)?)
}

#[tauri::command]
pub async fn resolve_conflict(
    state: State<'_, AppState>,
    entry_id: EntryId,
    keep_mine: bool,
) -> Result<()> {
    {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        household.resolve_conflict(actor, entry_id, keep_mine)?;
    }
    state.net.poke();
    Ok(())
}

/// Executa o que só o Anfitrião pode fazer: aqui mesmo, se esta máquina é o
/// Anfitrião, ou nele, pela rede, se esta é uma cópia.
#[tauri::command]
pub async fn host_command(state: State<'_, AppState>, command: HostCommand) -> Result<Value> {
    let link = {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        if !household.is_replica() {
            return Ok(household.execute(actor, command)?);
        }
        state.net.link()
    };
    let link = link.ok_or(finance_core::Error::NeedsHost)?;
    Ok(link.command(command).await?)
}

// --------------------------------------------------------------- consultas

#[tauri::command]
pub async fn members(state: State<'_, AppState>) -> Result<Vec<Member>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.members(actor)?)
}

#[tauri::command]
pub async fn categories(state: State<'_, AppState>) -> Result<Vec<Category>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.categories(actor)?)
}

#[tauri::command]
pub async fn entries(state: State<'_, AppState>, filter: EntryFilter) -> Result<Vec<Entry>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.entries(actor, &filter)?)
}

#[tauri::command]
pub async fn dashboard(state: State<'_, AppState>, month: Month) -> Result<Dashboard> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.dashboard(actor, month)?)
}

#[tauri::command]
pub async fn recurrences(state: State<'_, AppState>) -> Result<Vec<Recurrence>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.recurrences(actor)?)
}

#[tauri::command]
pub async fn installment_plans(state: State<'_, AppState>) -> Result<Vec<InstallmentPlan>> {
    let desk = state.desk();
    let (household, actor) = desk.current()?;
    Ok(household.installment_plans(actor)?)
}

// ------------------------------------------------------------- lançamentos
//
// Funcionam também offline: numa cópia, a alteração fica gravada localmente e
// a ligação é avisada para enviá-la assim que o Anfitrião estiver ao alcance.

#[tauri::command]
pub async fn create_entry(state: State<'_, AppState>, input: EntryInput) -> Result<Entry> {
    let entry = {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        household.create_entry(actor, input)?
    };
    state.net.poke();
    Ok(entry)
}

#[tauri::command]
pub async fn update_entry(
    state: State<'_, AppState>,
    id: EntryId,
    input: EntryInput,
) -> Result<Entry> {
    let entry = {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        household.update_entry(actor, id, input)?
    };
    state.net.poke();
    Ok(entry)
}

#[tauri::command]
pub async fn settle_entry(
    state: State<'_, AppState>,
    id: EntryId,
    amount_cents: Option<i64>,
) -> Result<Entry> {
    let entry = {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        household.settle_entry(actor, id, amount_cents)?
    };
    state.net.poke();
    Ok(entry)
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, id: EntryId) -> Result<()> {
    {
        let desk = state.desk();
        let (household, actor) = desk.current()?;
        household.delete_entry(actor, id)?;
    }
    state.net.poke();
    Ok(())
}
