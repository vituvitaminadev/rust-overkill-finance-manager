use chrono::NaiveDate;
use rusqlite::types::{ToSql, Type};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::household::{Member, Role, uuid_col};
use crate::{CategoryId, EntryId, Error, Household, MemberId, Month, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Income,
    Expense,
}

impl EntryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EntryKind::Income => "income",
            EntryKind::Expense => "expense",
        }
    }

    pub(crate) fn from_column(row: &Row<'_>, index: usize) -> rusqlite::Result<Self> {
        let text: String = row.get(index)?;
        match text.as_str() {
            "income" => Ok(EntryKind::Income),
            "expense" => Ok(EntryKind::Expense),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                index,
                Type::Text,
                format!("tipo de lançamento desconhecido: {other}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub kind: EntryKind,
    /// Sempre positivo; o tipo do lançamento dá o sinal.
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub description: String,
    pub category_id: CategoryId,
    /// Sem valor, o Responsável é quem lança (ou continua sendo o atual, numa edição).
    pub assignee_id: Option<MemberId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: EntryId,
    pub kind: EntryKind,
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub description: String,
    pub category_id: CategoryId,
    pub category_name: String,
    pub assignee_id: MemberId,
    pub assignee_name: String,
    pub version: i64,
    /// Se o membro que consultou pode editar e excluir este lançamento.
    pub can_edit: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EntryFilter {
    pub month: Option<Month>,
    pub kind: Option<EntryKind>,
    pub category_id: Option<CategoryId>,
    pub assignee_id: Option<MemberId>,
    /// Trecho da descrição, sem diferenciar maiúsculas de minúsculas.
    pub search: Option<String>,
}

const SELECT_ENTRY: &str = "
    SELECT e.id, e.kind, e.amount_cents, e.date, e.description,
           e.category_id, c.name, e.assignee_id, m.username, e.version
    FROM entry e
    JOIN category c ON c.id = e.category_id
    JOIN member m ON m.id = e.assignee_id
    WHERE e.deleted_at IS NULL";

impl Household {
    pub fn create_entry(&self, actor: MemberId, input: EntryInput) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let checked = self.check_entry(&actor, &input, None)?;
        let id = Uuid::new_v4();
        let now = self.clock.now();
        self.conn.execute(
            "INSERT INTO entry (id, kind, amount_cents, date, description, category_id,
                                assignee_id, created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id.to_string(),
                input.kind.as_str(),
                input.amount_cents,
                input.date,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                actor.id.to_string(),
                now
            ],
        )?;
        self.entry(&actor, id)
    }

    pub fn update_entry(&self, actor: MemberId, id: EntryId, input: EntryInput) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let current = self.editable_entry(&actor, id)?;
        let checked = self.check_entry(&actor, &input, Some(&current))?;
        self.conn.execute(
            "UPDATE entry
             SET kind = ?1, amount_cents = ?2, date = ?3, description = ?4, category_id = ?5,
                 assignee_id = ?6, version = version + 1, updated_at = ?7
             WHERE id = ?8",
            params![
                input.kind.as_str(),
                input.amount_cents,
                input.date,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                self.clock.now(),
                id.to_string()
            ],
        )?;
        self.entry(&actor, id)
    }

    pub fn delete_entry(&self, actor: MemberId, id: EntryId) -> Result<()> {
        let actor = self.actor(actor)?;
        self.editable_entry(&actor, id)?;
        // A linha fica, marcada: a sincronização precisa saber que houve uma exclusão.
        self.conn.execute(
            "UPDATE entry SET deleted_at = ?1, updated_at = ?1, version = version + 1
             WHERE id = ?2",
            params![self.clock.now(), id.to_string()],
        )?;
        Ok(())
    }

    /// Os lançamentos da residência, de qualquer Responsável, do mais recente ao mais antigo.
    pub fn entries(&self, actor: MemberId, filter: &EntryFilter) -> Result<Vec<Entry>> {
        let actor = self.actor(actor)?;
        let mut sql = SELECT_ENTRY.to_owned();
        let mut values: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(month) = filter.month {
            sql.push_str(" AND e.date >= ? AND e.date < ?");
            values.push(Box::new(month.first_day()));
            values.push(Box::new(month.next().first_day()));
        }
        if let Some(kind) = filter.kind {
            sql.push_str(" AND e.kind = ?");
            values.push(Box::new(kind.as_str()));
        }
        if let Some(category_id) = filter.category_id {
            sql.push_str(" AND e.category_id = ?");
            values.push(Box::new(category_id.to_string()));
        }
        if let Some(assignee_id) = filter.assignee_id {
            sql.push_str(" AND e.assignee_id = ?");
            values.push(Box::new(assignee_id.to_string()));
        }
        sql.push_str(" ORDER BY e.date DESC, e.created_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let mut entries: Vec<Entry> = stmt
            .query_map(rusqlite::params_from_iter(values), |row| {
                entry_from_row(row, &actor)
            })?
            .collect::<rusqlite::Result<_>>()?;

        // O `LIKE` do SQLite só ignora maiúsculas em ASCII, e "Água" precisa achar "água".
        let search = filter.search.as_deref().map(str::trim).unwrap_or_default();
        if !search.is_empty() {
            let needle = search.to_lowercase();
            entries.retain(|entry| entry.description.to_lowercase().contains(&needle));
        }
        Ok(entries)
    }

    fn entry(&self, actor: &Member, id: EntryId) -> Result<Entry> {
        self.conn
            .query_row(
                &format!("{SELECT_ENTRY} AND e.id = ?1"),
                [id.to_string()],
                |row| entry_from_row(row, actor),
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    fn editable_entry(&self, actor: &Member, id: EntryId) -> Result<Entry> {
        let entry = self.entry(actor, id)?;
        if !entry.can_edit {
            return Err(Error::Forbidden);
        }
        Ok(entry)
    }

    fn check_entry(
        &self,
        actor: &Member,
        input: &EntryInput,
        current: Option<&Entry>,
    ) -> Result<CheckedEntry> {
        if input.amount_cents <= 0 {
            return Err(Error::invalid("O valor precisa ser maior que zero."));
        }
        let description = input.description.trim();
        if description.is_empty() {
            return Err(Error::invalid("Informe uma descrição."));
        }

        let category = match self.category(input.category_id) {
            Err(Error::NotFound) => return Err(Error::invalid("Escolha uma categoria.")),
            other => other?,
        };
        if category.kind != input.kind {
            return Err(Error::invalid(
                "A categoria não corresponde ao tipo do lançamento.",
            ));
        }
        let keeps_category = current.is_some_and(|entry| entry.category_id == category.id);
        if category.archived && !keeps_category {
            return Err(Error::invalid("Esta categoria está arquivada."));
        }

        let assignee_id = input
            .assignee_id
            .or(current.map(|entry| entry.assignee_id))
            .unwrap_or(actor.id);
        if assignee_id != actor.id {
            if actor.role != Role::Admin {
                return Err(Error::Forbidden);
            }
            let keeps_assignee = current.is_some_and(|entry| entry.assignee_id == assignee_id);
            let assignable = matches!(self.member(assignee_id), Ok(member) if member.active);
            if !assignable && !keeps_assignee {
                return Err(Error::invalid(
                    "O responsável precisa ser um membro da residência.",
                ));
            }
        }

        Ok(CheckedEntry {
            description: description.to_owned(),
            assignee_id,
        })
    }
}

struct CheckedEntry {
    description: String,
    assignee_id: MemberId,
}

fn entry_from_row(row: &Row<'_>, actor: &Member) -> rusqlite::Result<Entry> {
    let assignee_id = uuid_col(row, 7)?;
    Ok(Entry {
        id: uuid_col(row, 0)?,
        kind: EntryKind::from_column(row, 1)?,
        amount_cents: row.get(2)?,
        date: row.get(3)?,
        description: row.get(4)?,
        category_id: uuid_col(row, 5)?,
        category_name: row.get(6)?,
        assignee_id,
        assignee_name: row.get(8)?,
        version: row.get(9)?,
        can_edit: actor.role == Role::Admin || assignee_id == actor.id,
    })
}
