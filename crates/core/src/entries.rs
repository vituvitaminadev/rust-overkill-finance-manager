use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::{ToSql, Type};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::household::{Member, Role, opt_uuid_col, uuid_col};
use crate::{
    CategoryId, EntryId, Error, Household, InstallmentPlanId, MemberId, Month, RecurrenceId, Result,
};

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
            other => Err(unknown_value(index, "tipo de lançamento", other)),
        }
    }
}

/// A situação de um lançamento: Previsto ou Efetivado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Planned,
    Settled,
}

impl EntryStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EntryStatus::Planned => "planned",
            EntryStatus::Settled => "settled",
        }
    }

    pub(crate) fn from_column(row: &Row<'_>, index: usize) -> rusqlite::Result<Self> {
        let text: String = row.get(index)?;
        match text.as_str() {
            "planned" => Ok(EntryStatus::Planned),
            "settled" => Ok(EntryStatus::Settled),
            other => Err(unknown_value(index, "situação", other)),
        }
    }
}

fn unknown_value(index: usize, what: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        format!("{what} desconhecido: {value}").into(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Sem valor, um lançamento novo com data futura nasce Previsto e os demais,
    /// Efetivados; numa edição, a situação atual é mantida.
    #[serde(default)]
    pub status: Option<EntryStatus>,
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
    pub status: EntryStatus,
    /// Previsto cuja data já passou: nada se efetiva sozinho.
    pub overdue: bool,
    /// A Recorrência que gerou este lançamento, se houver.
    pub recurrence_id: Option<RecurrenceId>,
    /// A posição deste lançamento no seu Parcelamento, se houver.
    pub installment: Option<Installment>,
    pub version: i64,
    /// Se o membro que consultou pode editar e excluir este lançamento.
    pub can_edit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installment {
    pub plan_id: InstallmentPlanId,
    pub number: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EntryFilter {
    pub month: Option<Month>,
    pub kind: Option<EntryKind>,
    pub status: Option<EntryStatus>,
    pub category_id: Option<CategoryId>,
    pub assignee_id: Option<MemberId>,
    /// Trecho da descrição, sem diferenciar maiúsculas de minúsculas.
    pub search: Option<String>,
}

pub(crate) const SELECT_ENTRY: &str = "
    SELECT e.id, e.kind, e.amount_cents, e.date, e.description,
           e.category_id, c.name, e.assignee_id, m.username, e.version,
           e.status, e.recurrence_id, e.installment_plan_id, e.installment_number, p.count
    FROM entry e
    JOIN category c ON c.id = e.category_id
    JOIN member m ON m.id = e.assignee_id
    LEFT JOIN installment_plan p ON p.id = e.installment_plan_id
    WHERE e.deleted_at IS NULL";

/// Um lançamento já validado, pronto para ser gravado.
pub(crate) struct NewEntryRow<'a> {
    pub id: EntryId,
    pub kind: EntryKind,
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub description: &'a str,
    pub category_id: CategoryId,
    pub assignee_id: MemberId,
    pub created_by: MemberId,
    pub status: EntryStatus,
    /// A Recorrência de origem e o mês que este lançamento ocupa nela.
    pub recurrence: Option<(RecurrenceId, Month)>,
    /// O Parcelamento de origem e o número da parcela.
    pub installment: Option<(InstallmentPlanId, u32)>,
}

pub(crate) fn insert_entry(
    conn: &Connection,
    now: DateTime<Utc>,
    row: &NewEntryRow<'_>,
) -> rusqlite::Result<EntryId> {
    let id = row.id;
    conn.execute(
        "INSERT INTO entry (id, kind, amount_cents, date, description, category_id, assignee_id,
                            created_by, status, recurrence_id, occurrence_month,
                            installment_plan_id, installment_number, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
        params![
            id.to_string(),
            row.kind.as_str(),
            row.amount_cents,
            row.date,
            row.description,
            row.category_id.to_string(),
            row.assignee_id.to_string(),
            row.created_by.to_string(),
            row.status.as_str(),
            row.recurrence.map(|(id, _)| id.to_string()),
            row.recurrence.map(|(_, month)| month.to_string()),
            row.installment.map(|(id, _)| id.to_string()),
            row.installment.map(|(_, number)| number),
            now
        ],
    )?;
    Ok(id)
}

impl Household {
    pub fn create_entry(&self, actor: MemberId, input: EntryInput) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let checked = self.check_entry(&actor, &input, None)?;
        let status = input.status.unwrap_or(if input.date > self.clock.today() {
            EntryStatus::Planned
        } else {
            EntryStatus::Settled
        });
        let id = insert_entry(
            &self.conn,
            self.clock.now(),
            &NewEntryRow {
                id: Uuid::new_v4(),
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
        self.mark_dirty(id, 0)?;
        self.entry(&actor, id)
    }

    /// Altera só este lançamento. Se ele veio de uma Recorrência, mudanças
    /// posteriores nela deixam de alcançá-lo.
    pub fn update_entry(&self, actor: MemberId, id: EntryId, input: EntryInput) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let current = self.editable_entry(&actor, id)?;
        let checked = self.check_entry(&actor, &input, Some(&current))?;
        self.conn.execute(
            "UPDATE entry
             SET kind = ?1, amount_cents = ?2, date = ?3, description = ?4, category_id = ?5,
                 assignee_id = ?6, status = ?7, detached = 1, version = version + 1,
                 updated_at = ?8
             WHERE id = ?9",
            params![
                input.kind.as_str(),
                input.amount_cents,
                input.date,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                input.status.unwrap_or(current.status).as_str(),
                self.clock.now(),
                id.to_string()
            ],
        )?;
        self.mark_dirty(id, current.version)?;
        self.entry(&actor, id)
    }

    /// Marca um lançamento Previsto como Efetivado, opcionalmente com o valor real.
    pub fn settle_entry(
        &self,
        actor: MemberId,
        id: EntryId,
        amount_cents: Option<i64>,
    ) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let current = self.editable_entry(&actor, id)?;
        let amount_cents = amount_cents.unwrap_or(current.amount_cents);
        if amount_cents <= 0 {
            return Err(Error::invalid("O valor precisa ser maior que zero."));
        }
        self.conn.execute(
            "UPDATE entry
             SET status = 'settled', amount_cents = ?1, detached = 1, version = version + 1,
                 updated_at = ?2
             WHERE id = ?3",
            params![amount_cents, self.clock.now(), id.to_string()],
        )?;
        self.mark_dirty(id, current.version)?;
        self.entry(&actor, id)
    }

    pub fn delete_entry(&self, actor: MemberId, id: EntryId) -> Result<()> {
        let actor = self.actor(actor)?;
        let current = self.editable_entry(&actor, id)?;
        // A linha fica, marcada: a sincronização precisa saber que houve uma exclusão.
        self.conn.execute(
            "UPDATE entry SET deleted_at = ?1, updated_at = ?1, version = version + 1
             WHERE id = ?2",
            params![self.clock.now(), id.to_string()],
        )?;
        self.mark_dirty(id, current.version)?;
        Ok(())
    }

    /// Na cópia, anota que o lançamento tem uma alteração local a enviar ao
    /// Anfitrião e de qual versão dele ela partiu. No Anfitrião, não faz nada.
    fn mark_dirty(&self, id: EntryId, previous_version: i64) -> Result<()> {
        if self.replica {
            self.conn.execute(
                "UPDATE entry
                 SET dirty = dirty + 1, base_version = COALESCE(base_version, ?1)
                 WHERE id = ?2",
                params![previous_version, id.to_string()],
            )?;
        }
        Ok(())
    }

    /// Os lançamentos da residência, de qualquer Responsável, do mais recente ao mais antigo.
    pub fn entries(&self, actor: MemberId, filter: &EntryFilter) -> Result<Vec<Entry>> {
        let actor = self.actor(actor)?;
        self.top_up_recurrences()?;

        let mut conditions = String::new();
        let mut values: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(month) = filter.month {
            conditions.push_str(" AND e.date >= ? AND e.date < ?");
            values.push(Box::new(month.first_day()));
            values.push(Box::new(month.next().first_day()));
        }
        if let Some(kind) = filter.kind {
            conditions.push_str(" AND e.kind = ?");
            values.push(Box::new(kind.as_str()));
        }
        if let Some(status) = filter.status {
            conditions.push_str(" AND e.status = ?");
            values.push(Box::new(status.as_str()));
        }
        if let Some(category_id) = filter.category_id {
            conditions.push_str(" AND e.category_id = ?");
            values.push(Box::new(category_id.to_string()));
        }
        if let Some(assignee_id) = filter.assignee_id {
            conditions.push_str(" AND e.assignee_id = ?");
            values.push(Box::new(assignee_id.to_string()));
        }
        let mut entries = self.select_entries(
            &actor,
            &format!("{conditions} ORDER BY e.date DESC, e.created_at DESC"),
            values,
        )?;

        // O `LIKE` do SQLite só ignora maiúsculas em ASCII, e "Água" precisa achar "água".
        let search = filter.search.as_deref().map(str::trim).unwrap_or_default();
        if !search.is_empty() {
            let needle = search.to_lowercase();
            entries.retain(|entry| entry.description.to_lowercase().contains(&needle));
        }
        Ok(entries)
    }

    /// Os lançamentos que satisfazem `tail`, um trecho de SQL que continua o `WHERE`.
    pub(crate) fn select_entries(
        &self,
        actor: &Member,
        tail: &str,
        values: Vec<Box<dyn ToSql>>,
    ) -> Result<Vec<Entry>> {
        let today = self.clock.today();
        let mut stmt = self.conn.prepare(&format!("{SELECT_ENTRY}{tail}"))?;
        let entries = stmt
            .query_map(rusqlite::params_from_iter(values), |row| {
                entry_from_row(row, actor, today)
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(entries)
    }

    pub(crate) fn entry(&self, actor: &Member, id: EntryId) -> Result<Entry> {
        let today = self.clock.today();
        self.conn
            .query_row(
                &format!("{SELECT_ENTRY} AND e.id = ?1"),
                [id.to_string()],
                |row| entry_from_row(row, actor, today),
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub(crate) fn editable_entry(&self, actor: &Member, id: EntryId) -> Result<Entry> {
        let entry = self.entry(actor, id)?;
        if !entry.can_edit {
            return Err(Error::Forbidden);
        }
        Ok(entry)
    }

    pub(crate) fn check_entry(
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

pub(crate) struct CheckedEntry {
    pub description: String,
    pub assignee_id: MemberId,
}

fn entry_from_row(row: &Row<'_>, actor: &Member, today: NaiveDate) -> rusqlite::Result<Entry> {
    let assignee_id = uuid_col(row, 7)?;
    let date: NaiveDate = row.get(3)?;
    let status = EntryStatus::from_column(row, 10)?;
    let installment = match opt_uuid_col(row, 12)? {
        Some(plan_id) => Some(Installment {
            plan_id,
            number: row.get(13)?,
            count: row.get(14)?,
        }),
        None => None,
    };
    Ok(Entry {
        id: uuid_col(row, 0)?,
        kind: EntryKind::from_column(row, 1)?,
        amount_cents: row.get(2)?,
        date,
        description: row.get(4)?,
        category_id: uuid_col(row, 5)?,
        category_name: row.get(6)?,
        assignee_id,
        assignee_name: row.get(8)?,
        status,
        overdue: status == EntryStatus::Planned && date < today,
        recurrence_id: opt_uuid_col(row, 11)?,
        installment,
        version: row.get(9)?,
        can_edit: actor.role == Role::Admin || assignee_id == actor.id,
    })
}
