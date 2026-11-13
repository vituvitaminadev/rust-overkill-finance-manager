//! Sincronização por versão de registro (ADR 0001).
//!
//! O Anfitrião carimba cada escrita com o seu relógio e serve "o que mudou
//! desde X". A cópia guarda as alterações locais em lançamentos, envia cada
//! uma com a versão de que partiu e registra um Conflito quando o Anfitrião
//! já está em outra versão. Este módulo não conhece rede: quem transporta as
//! mensagens fica em outra camada.

use chrono::NaiveDate;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::entries::{EntryStatus, NewEntryRow, insert_entry};
use crate::household::{Member, Role};
use crate::schema::SYNCED_TABLES;
use crate::{
    CategoryId, EntryId, EntryInput, EntryKind, Error, Household, InstallmentPlanId,
    InstallmentPlanInput, MemberId, NewCategory, RecurrenceId, RecurrenceInput, Result,
};

/// Muda sempre que o formato das mensagens ou das linhas trocadas muda.
/// Versões diferentes do app não sincronizam entre si.
pub const PROTOCOL_VERSION: u32 = 1;

const CURSOR_KEY: &str = "cursor";
/// Colunas que só fazem sentido na cópia e nunca trafegam.
const LOCAL_ONLY_COLUMNS: &[&str] = &["dirty", "base_version"];

type Row = serde_json::Map<String, Value>;

/// As linhas que mudaram no Anfitrião desde um ponto do relógio dele.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    /// O relógio do Anfitrião neste retrato: o próximo pedido parte daqui.
    pub seq: i64,
    pub tables: Vec<TableChanges>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableChanges {
    pub table: String,
    pub rows: Vec<Row>,
}

/// Uma alteração local em um lançamento, como a cópia a envia ao Anfitrião.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntryChange {
    Upsert {
        id: EntryId,
        /// A versão do Anfitrião de que a alteração partiu; zero para um lançamento novo.
        base_version: i64,
        input: EntryInput,
    },
    Delete {
        id: EntryId,
        base_version: i64,
    },
}

impl EntryChange {
    pub fn id(&self) -> EntryId {
        match self {
            EntryChange::Upsert { id, .. } | EntryChange::Delete { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingChange {
    pub change: EntryChange,
    /// Quantas alterações locais o lançamento tinha ao ser lido; se mudar até a
    /// resposta chegar, houve nova edição e ele continua pendente.
    pub generation: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushOutcome {
    Applied {
        version: i64,
    },
    Conflict {
        /// A linha atual do Anfitrião; ausente quando ele não tem o lançamento.
        host: Option<Row>,
        host_version: i64,
        /// Presente quando o Anfitrião recusou a alteração (permissão, validação).
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Lançamentos com alteração local ainda não enviada.
    pub pending: i64,
    pub conflicts: i64,
}

/// Um lançamento alterado dos dois lados, à espera de uma escolha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub entry_id: EntryId,
    /// A versão desta máquina; ausente se o lançamento foi excluído aqui.
    pub mine: Option<ConflictSide>,
    /// A versão do Anfitrião; ausente se ele não tem (ou excluiu) o lançamento.
    pub theirs: Option<ConflictSide>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSide {
    pub kind: EntryKind,
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub description: String,
    pub category_name: String,
    pub assignee_name: String,
    pub status: EntryStatus,
}

/// O que só o Anfitrião executa. A cópia envia o comando e depois recebe o
/// resultado como alterações comuns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "args", rename_all = "snake_case")]
pub enum HostCommand {
    CreateCategory(NewCategory),
    RenameCategory { id: CategoryId, name: String },
    SetCategoryArchived { id: CategoryId, archived: bool },
    CreateRecurrence(RecurrenceInput),
    EndRecurrence { id: RecurrenceId },
    UpdateEntryAndFollowing { id: EntryId, input: EntryInput },
    CreateInstallmentPlan(InstallmentPlanInput),
    DeleteInstallmentPlan { id: InstallmentPlanId },
    CreateInvite,
    Invites,
    RevokeInvite { id: Uuid },
    AddMember { name: String },
    SetMemberRole { id: MemberId, role: Role },
    RemoveMember { id: MemberId },
    ResetPassword { id: MemberId, password: String },
    ChangePassword { current: String, new: String },
}

impl Household {
    /// Executa um comando em nome de `actor` e devolve o resultado em JSON.
    pub fn execute(&self, actor: MemberId, command: HostCommand) -> Result<Value> {
        fn json<T: Serialize>(value: T) -> Result<Value> {
            serde_json::to_value(value).map_err(|e| Error::Invalid(e.to_string()))
        }
        match command {
            HostCommand::CreateCategory(input) => json(self.create_category(actor, input)?),
            HostCommand::RenameCategory { id, name } => {
                json(self.rename_category(actor, id, &name)?)
            }
            HostCommand::SetCategoryArchived { id, archived } => {
                json(self.set_category_archived(actor, id, archived)?)
            }
            HostCommand::CreateRecurrence(input) => json(self.create_recurrence(actor, input)?),
            HostCommand::EndRecurrence { id } => json(self.end_recurrence(actor, id)?),
            HostCommand::UpdateEntryAndFollowing { id, input } => {
                json(self.update_entry_and_following(actor, id, input)?)
            }
            HostCommand::CreateInstallmentPlan(input) => {
                json(self.create_installment_plan(actor, input)?)
            }
            HostCommand::DeleteInstallmentPlan { id } => {
                json(self.delete_installment_plan(actor, id)?)
            }
            HostCommand::CreateInvite => json(self.create_invite(actor)?),
            HostCommand::Invites => json(self.invites(actor)?),
            HostCommand::RevokeInvite { id } => json(self.revoke_invite(actor, id)?),
            HostCommand::AddMember { name } => json(self.add_member(actor, &name)?),
            HostCommand::SetMemberRole { id, role } => json(self.set_member_role(actor, id, role)?),
            HostCommand::RemoveMember { id } => json(self.remove_member(actor, id)?),
            HostCommand::ResetPassword { id, password } => {
                json(self.reset_password(actor, id, &password)?)
            }
            HostCommand::ChangePassword { current, new } => {
                json(self.change_password(actor, &current, &new)?)
            }
        }
    }

    // ---------------------------------------------------------------- Anfitrião

    /// O valor atual do relógio de alterações.
    pub fn clock_value(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT value FROM sync_clock", [], |row| row.get(0))?)
    }

    /// Tudo o que mudou depois de `since`; com zero, a residência inteira.
    pub fn changes_since(&self, since: i64) -> Result<ChangeSet> {
        self.host_only()?;
        self.top_up_recurrences()?;
        // Uma transação para que o retrato e o relógio sejam do mesmo instante.
        let tx = self.conn.unchecked_transaction()?;
        let seq: i64 = tx.query_row("SELECT value FROM sync_clock", [], |row| row.get(0))?;
        let mut tables = Vec::new();
        for table in SYNCED_TABLES {
            let rows = read_rows(
                &tx,
                &format!("SELECT * FROM {table} WHERE seq > ?1 AND seq <= ?2"),
                params![since, seq],
            )?;
            if !rows.is_empty() {
                tables.push(TableChanges {
                    table: (*table).to_owned(),
                    rows,
                });
            }
        }
        tx.commit()?;
        Ok(ChangeSet { seq, tables })
    }

    /// Aplica as alterações de uma cópia, feitas em nome de `actor`. Devolve um
    /// resultado por alteração, na mesma ordem.
    pub fn apply_push(&self, actor: MemberId, changes: &[EntryChange]) -> Result<Vec<PushOutcome>> {
        self.host_only()?;
        let actor = self.actor(actor)?;
        changes
            .iter()
            .map(|change| {
                let tx = self.conn.unchecked_transaction()?;
                let outcome = match self.apply_change(&actor, change) {
                    Ok(outcome) => outcome,
                    // Uma recusa não derruba o envio: vira um Conflito com o motivo.
                    Err(error @ (Error::Invalid(_) | Error::Forbidden | Error::NotFound)) => {
                        let host = self.raw_entry(change.id())?;
                        PushOutcome::Conflict {
                            host_version: host.as_ref().map_or(0, version_of),
                            host,
                            reason: Some(error.to_string()),
                        }
                    }
                    Err(other) => return Err(other),
                };
                tx.commit()?;
                Ok(outcome)
            })
            .collect()
    }

    fn apply_change(&self, actor: &Member, change: &EntryChange) -> Result<PushOutcome> {
        let host = self.raw_entry(change.id())?;
        let base_version = match change {
            EntryChange::Upsert { base_version, .. } | EntryChange::Delete { base_version, .. } => {
                *base_version
            }
        };
        let Some(host) = host else {
            return match change {
                // Excluir o que o Anfitrião nunca teve não muda nada.
                EntryChange::Delete { .. } => Ok(PushOutcome::Applied { version: 0 }),
                EntryChange::Upsert { .. } if base_version != 0 => Err(Error::NotFound),
                EntryChange::Upsert { id, input, .. } => {
                    self.insert_pushed(actor, *id, input)?;
                    Ok(PushOutcome::Applied { version: 1 })
                }
            };
        };

        let host_version = version_of(&host);
        if host_version != base_version {
            return Ok(PushOutcome::Conflict {
                host: Some(host),
                host_version,
                reason: None,
            });
        }
        let assignee = host
            .get("assignee_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok());
        if actor.role != Role::Admin && assignee != Some(actor.id) {
            return Err(Error::Forbidden);
        }
        let deleted = !host.get("deleted_at").is_none_or(Value::is_null);
        let id = change.id().to_string();
        let now = self.clock.now();

        match change {
            EntryChange::Delete { .. } => {
                if deleted {
                    return Ok(PushOutcome::Applied {
                        version: host_version,
                    });
                }
                self.conn.execute(
                    "UPDATE entry SET deleted_at = ?1, updated_at = ?1, version = version + 1
                     WHERE id = ?2",
                    params![now, id],
                )?;
            }
            EntryChange::Upsert { input, .. } => {
                // Um lançamento excluído no Anfitrião volta quando a cópia insiste na sua versão.
                let current = if deleted {
                    None
                } else {
                    Some(self.entry(actor, change.id())?)
                };
                let checked = self.check_entry(actor, input, current.as_ref())?;
                let status = input
                    .status
                    .or(current.map(|entry| entry.status))
                    .unwrap_or(EntryStatus::Settled);
                self.conn.execute(
                    "UPDATE entry
                     SET kind = ?1, amount_cents = ?2, date = ?3, description = ?4,
                         category_id = ?5, assignee_id = ?6, status = ?7, detached = 1,
                         deleted_at = NULL, version = version + 1, updated_at = ?8
                     WHERE id = ?9",
                    params![
                        input.kind.as_str(),
                        input.amount_cents,
                        input.date,
                        checked.description,
                        input.category_id.to_string(),
                        checked.assignee_id.to_string(),
                        status.as_str(),
                        now,
                        id
                    ],
                )?;
            }
        }
        Ok(PushOutcome::Applied {
            version: host_version + 1,
        })
    }

    fn insert_pushed(&self, actor: &Member, id: EntryId, input: &EntryInput) -> Result<()> {
        let checked = self.check_entry(actor, input, None)?;
        let status = input.status.unwrap_or(if input.date > self.clock.today() {
            EntryStatus::Planned
        } else {
            EntryStatus::Settled
        });
        insert_entry(
            &self.conn,
            self.clock.now(),
            &NewEntryRow {
                id,
                kind: input.kind,
                amount_cents: input.amount_cents,
                date: input.date,
                description: &checked.description,
                category_id: input.category_id,
                assignee_id: checked.assignee_id,
                created_by: actor.id,
                status,
                recurrence: None,
                installment: None,
            },
        )?;
        Ok(())
    }

    /// A linha do lançamento como está no banco, excluída ou não.
    fn raw_entry(&self, id: EntryId) -> Result<Option<Row>> {
        Ok(read_rows(
            &self.conn,
            "SELECT * FROM entry WHERE id = ?1",
            params![id.to_string()],
        )?
        .pop())
    }

    // -------------------------------------------------------------------- Cópia

    /// Até onde esta cópia já recebeu as alterações do Anfitrião.
    pub fn cursor(&self) -> Result<i64> {
        Ok(self
            .local(CURSOR_KEY)?
            .and_then(|value| value.parse().ok())
            .unwrap_or(0))
    }

    /// Grava nesta cópia o que veio do Anfitrião. Um lançamento com alteração
    /// local pendente não é sobrescrito: se o Anfitrião também o mudou, vira Conflito.
    pub fn apply_changes(&self, changes: &ChangeSet) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for table in &changes.tables {
            if !SYNCED_TABLES.contains(&table.table.as_str()) {
                return Err(Error::Invalid(format!(
                    "Tabela desconhecida na sincronização: {}",
                    table.table
                )));
            }
            for row in &table.rows {
                if table.table == "entry" && self.keeps_local_entry(&tx, row)? {
                    continue;
                }
                upsert_row(&tx, &table.table, row)?;
            }
        }
        tx.execute(
            "INSERT INTO local_state (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![CURSOR_KEY, changes.seq.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Verdadeiro se o lançamento tem alteração local e por isso a linha do
    /// Anfitrião não deve substituí-lo agora.
    fn keeps_local_entry(&self, conn: &Connection, host: &Row) -> Result<bool> {
        let Some(id) = host.get("id").and_then(Value::as_str) else {
            return Ok(false);
        };
        let local: Option<(i64, Option<i64>)> = conn
            .query_row(
                "SELECT dirty, base_version FROM entry WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((dirty, base_version)) = local.filter(|(dirty, _)| *dirty > 0) else {
            return Ok(false);
        };
        debug_assert!(dirty > 0);
        let host_version = version_of(host);
        if base_version != Some(host_version) {
            store_conflict(conn, id, Some(host), host_version, None)?;
        }
        Ok(true)
    }

    /// As alterações locais prontas para envio; as que estão em Conflito
    /// esperam a escolha de alguém.
    pub fn pending_changes(&self) -> Result<Vec<PendingChange>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, dirty, COALESCE(base_version, 0), deleted_at IS NOT NULL, kind,
                    amount_cents, date, description, category_id, assignee_id, status
             FROM entry
             WHERE dirty > 0 AND id NOT IN (SELECT entry_id FROM conflict)
             ORDER BY updated_at",
        )?;
        let pending = stmt
            .query_map([], |row| {
                let id = crate::household::uuid_col(row, 0)?;
                let base_version: i64 = row.get(2)?;
                let change = if row.get::<_, bool>(3)? {
                    EntryChange::Delete { id, base_version }
                } else {
                    EntryChange::Upsert {
                        id,
                        base_version,
                        input: EntryInput {
                            kind: EntryKind::from_column(row, 4)?,
                            amount_cents: row.get(5)?,
                            date: row.get(6)?,
                            description: row.get(7)?,
                            category_id: crate::household::uuid_col(row, 8)?,
                            assignee_id: Some(crate::household::uuid_col(row, 9)?),
                            status: Some(EntryStatus::from_column(row, 10)?),
                        },
                    }
                };
                Ok(PendingChange {
                    change,
                    generation: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(pending)
    }

    /// Registra a resposta do Anfitrião a um envio: o que foi aceito deixa de
    /// estar pendente, e o que divergiu vira Conflito.
    pub fn record_push(&self, sent: &[PendingChange], outcomes: &[PushOutcome]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (sent, outcome) in sent.iter().zip(outcomes) {
            let id = sent.change.id().to_string();
            match outcome {
                PushOutcome::Applied { version } => {
                    tx.execute(
                        "UPDATE entry
                         SET version = ?1,
                             base_version = CASE WHEN dirty = ?2 THEN NULL ELSE ?1 END,
                             dirty = CASE WHEN dirty = ?2 THEN 0 ELSE dirty END
                         WHERE id = ?3",
                        params![version, sent.generation, id],
                    )?;
                }
                PushOutcome::Conflict {
                    host,
                    host_version,
                    reason,
                } => store_conflict(&tx, &id, host.as_ref(), *host_version, reason.as_deref())?,
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatus> {
        Ok(self.conn.query_row(
            "SELECT
                 (SELECT COUNT(*) FROM entry
                  WHERE dirty > 0 AND id NOT IN (SELECT entry_id FROM conflict)),
                 (SELECT COUNT(*) FROM conflict)",
            [],
            |row| {
                Ok(SyncStatus {
                    pending: row.get(0)?,
                    conflicts: row.get(1)?,
                })
            },
        )?)
    }

    pub fn conflicts(&self, actor: MemberId) -> Result<Vec<Conflict>> {
        let actor = self.actor(actor)?;
        let stored: Vec<(EntryId, Option<String>, Option<String>)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT entry_id, host_row, reason FROM conflict ORDER BY entry_id")?;
            stmt.query_map([], |row| {
                Ok((
                    crate::household::uuid_col(row, 0)?,
                    row.get(1)?,
                    row.get(2)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?
        };

        let mut conflicts = Vec::new();
        for (entry_id, host_row, reason) in stored {
            let mine = match self.entry(&actor, entry_id) {
                Ok(entry) => Some(ConflictSide {
                    kind: entry.kind,
                    amount_cents: entry.amount_cents,
                    date: entry.date,
                    description: entry.description,
                    category_name: entry.category_name,
                    assignee_name: entry.assignee_name,
                    status: entry.status,
                }),
                Err(Error::NotFound) => None,
                Err(other) => return Err(other),
            };
            let theirs = match host_row {
                Some(json) => self.side_from_row(&json)?,
                None => None,
            };
            conflicts.push(Conflict {
                entry_id,
                mine,
                theirs,
                reason,
            });
        }
        Ok(conflicts)
    }

    /// Encerra um Conflito ficando com a versão desta máquina, que será
    /// reenviada, ou com a do Anfitrião, que substitui a local.
    pub fn resolve_conflict(
        &self,
        actor: MemberId,
        entry_id: EntryId,
        keep_mine: bool,
    ) -> Result<()> {
        self.actor(actor)?;
        let id = entry_id.to_string();
        let (host_row, host_version): (Option<String>, i64) = self
            .conn
            .query_row(
                "SELECT host_row, host_version FROM conflict WHERE entry_id = ?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(Error::NotFound)?;

        let tx = self.conn.unchecked_transaction()?;
        if keep_mine {
            tx.execute(
                "UPDATE entry SET base_version = ?1 WHERE id = ?2",
                params![host_version, id],
            )?;
        } else {
            match host_row {
                Some(json) => {
                    let row: Row =
                        serde_json::from_str(&json).map_err(|e| Error::Invalid(e.to_string()))?;
                    upsert_row(&tx, "entry", &row)?;
                    tx.execute(
                        "UPDATE entry SET dirty = 0, base_version = NULL WHERE id = ?1",
                        [&id],
                    )?;
                }
                // O Anfitrião nunca teve este lançamento: ele some daqui também.
                None => {
                    tx.execute("DELETE FROM entry WHERE id = ?1", [&id])?;
                }
            }
        }
        tx.execute("DELETE FROM conflict WHERE entry_id = ?1", [&id])?;
        tx.commit()?;
        Ok(())
    }

    fn side_from_row(&self, json: &str) -> Result<Option<ConflictSide>> {
        #[derive(Deserialize)]
        struct HostEntry {
            kind: EntryKind,
            amount_cents: i64,
            date: NaiveDate,
            description: String,
            category_id: String,
            assignee_id: String,
            status: EntryStatus,
            deleted_at: Option<String>,
        }
        let host: HostEntry =
            serde_json::from_str(json).map_err(|e| Error::Invalid(e.to_string()))?;
        if host.deleted_at.is_some() {
            return Ok(None);
        }
        let name = |sql: &str, id: &str| -> Result<String> {
            Ok(self
                .conn
                .query_row(sql, [id], |row| row.get(0))
                .optional()?
                .unwrap_or_else(|| "?".to_owned()))
        };
        Ok(Some(ConflictSide {
            kind: host.kind,
            amount_cents: host.amount_cents,
            date: host.date,
            description: host.description,
            category_name: name("SELECT name FROM category WHERE id = ?1", &host.category_id)?,
            assignee_name: name(
                "SELECT username FROM member WHERE id = ?1",
                &host.assignee_id,
            )?,
            status: host.status,
        }))
    }
}

fn version_of(row: &Row) -> i64 {
    row.get("version").and_then(Value::as_i64).unwrap_or(0)
}

fn store_conflict(
    conn: &Connection,
    entry_id: &str,
    host: Option<&Row>,
    host_version: i64,
    reason: Option<&str>,
) -> Result<()> {
    let host_row = host
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| Error::Invalid(e.to_string()))?;
    conn.execute(
        "INSERT INTO conflict (entry_id, host_row, host_version, reason)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (entry_id) DO UPDATE
         SET host_row = excluded.host_row, host_version = excluded.host_version,
             reason = excluded.reason",
        params![entry_id, host_row, host_version, reason],
    )?;
    Ok(())
}

/// Lê linhas inteiras como objetos JSON, sem as colunas que só existem na cópia.
fn read_rows(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<Row>> {
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().map(String::from).collect();
    let mut rows = stmt.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = Row::new();
        for (index, name) in names.iter().enumerate() {
            if LOCAL_ONLY_COLUMNS.contains(&name.as_str()) {
                continue;
            }
            let value = match row.get_ref(index)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(text) => Value::from(String::from_utf8_lossy(text).into_owned()),
            };
            object.insert(name.clone(), value);
        }
        out.push(object);
    }
    Ok(out)
}

/// Insere ou atualiza uma linha recebida, coluna a coluna. Só colunas que a
/// tabela realmente tem são aceitas, e as locais da cópia ficam intocadas.
fn upsert_row(conn: &Connection, table: &str, row: &Row) -> Result<()> {
    let known: Vec<String> = {
        let mut stmt = conn.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    let columns: Vec<&String> = row
        .keys()
        .filter(|name| known.contains(name) && !LOCAL_ONLY_COLUMNS.contains(&name.as_str()))
        .collect();
    if !columns.iter().any(|name| name.as_str() == "id") {
        return Err(Error::invalid("Linha sem identificador na sincronização."));
    }
    let names = columns
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = vec!["?"; columns.len()].join(", ");
    let updates = columns
        .iter()
        .filter(|name| name.as_str() != "id")
        .map(|name| format!("{name} = excluded.{name}"))
        .collect::<Vec<_>>()
        .join(", ");
    let values = columns.iter().map(|name| match &row[name.as_str()] {
        Value::Null => SqlValue::Null,
        Value::Bool(flag) => SqlValue::Integer(i64::from(*flag)),
        Value::Number(n) => match n.as_i64() {
            Some(integer) => SqlValue::Integer(integer),
            None => SqlValue::Real(n.as_f64().unwrap_or_default()),
        },
        Value::String(text) => SqlValue::Text(text.clone()),
        other => SqlValue::Text(other.to_string()),
    });
    conn.execute(
        &format!(
            "INSERT INTO {table} ({names}) VALUES ({placeholders})
             ON CONFLICT (id) DO UPDATE SET {updates}"
        ),
        rusqlite::params_from_iter(values),
    )?;
    Ok(())
}
