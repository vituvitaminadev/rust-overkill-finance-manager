//! O que dá previsibilidade: Recorrências e Parcelamentos, que geram
//! lançamentos previstos.

use chrono::{Datelike, NaiveDate};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entries::{EntryStatus, NewEntryRow, insert_entry};
use crate::household::{Member, Role, uuid_col};
use crate::month::month_col;
use crate::{
    CategoryId, Entry, EntryId, EntryInput, EntryKind, Error, Household, InstallmentPlanId,
    MemberId, Month, RecurrenceId, Result,
};

/// Quantos meses à frente do atual uma Recorrência mantém lançamentos previstos.
const HORIZON_MONTHS: i32 = 12;
const MAX_INSTALLMENTS: u32 = 360;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceInput {
    pub kind: EntryKind,
    pub amount_cents: i64,
    pub description: String,
    pub category_id: CategoryId,
    pub assignee_id: Option<MemberId>,
    /// A data do primeiro lançamento; os seguintes caem no mesmo dia de cada mês.
    pub first_date: NaiveDate,
    /// O último mês com lançamento. Sem valor, a Recorrência não tem fim.
    #[serde(default)]
    pub end_month: Option<Month>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    pub id: RecurrenceId,
    pub kind: EntryKind,
    pub amount_cents: i64,
    pub description: String,
    pub category_id: CategoryId,
    pub category_name: String,
    pub assignee_id: MemberId,
    pub assignee_name: String,
    pub day_of_month: u32,
    pub start_month: Month,
    pub end_month: Option<Month>,
    /// Se o membro que consultou pode alterar e encerrar esta Recorrência.
    pub can_edit: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallmentPlanInput {
    pub description: String,
    /// O valor total da compra, dividido entre as parcelas.
    pub total_cents: i64,
    pub count: u32,
    /// A data da primeira parcela; as seguintes caem no mesmo dia dos meses seguintes.
    pub first_date: NaiveDate,
    pub category_id: CategoryId,
    pub assignee_id: Option<MemberId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallmentPlan {
    pub id: InstallmentPlanId,
    pub description: String,
    pub total_cents: i64,
    pub count: u32,
    /// Quantas parcelas ainda estão previstas, e quanto somam.
    pub planned_count: u32,
    pub planned_cents: i64,
    pub category_name: String,
    pub assignee_id: MemberId,
    pub assignee_name: String,
    pub first_date: NaiveDate,
    pub can_edit: bool,
}

const SELECT_RECURRENCE: &str = "
    SELECT r.id, r.kind, r.amount_cents, r.description, r.category_id, c.name,
           r.assignee_id, m.username, r.day_of_month, r.start_month, r.end_month
    FROM recurrence r
    JOIN category c ON c.id = r.category_id
    JOIN member m ON m.id = r.assignee_id
    WHERE r.deleted_at IS NULL AND r.ended_at IS NULL";

impl Household {
    pub fn create_recurrence(&self, actor: MemberId, input: RecurrenceInput) -> Result<Recurrence> {
        let actor = self.actor(actor)?;
        let start_month = Month::of(input.first_date);
        if input.end_month.is_some_and(|end| end < start_month) {
            return Err(Error::invalid(
                "O fim da recorrência não pode vir antes do começo.",
            ));
        }
        let checked = self.check_entry(&actor, &as_entry_input(&input), None)?;
        let id = Uuid::new_v4();
        self.conn.execute(
            "INSERT INTO recurrence (id, kind, amount_cents, description, category_id,
                                     assignee_id, created_by, day_of_month, start_month,
                                     end_month, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id.to_string(),
                input.kind.as_str(),
                input.amount_cents,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                actor.id.to_string(),
                input.first_date.day(),
                start_month.to_string(),
                input.end_month.map(|month| month.to_string()),
                self.clock.now()
            ],
        )?;
        self.top_up_recurrences()?;
        self.recurrence(&actor, id)
    }

    /// As Recorrências em vigor: nem encerradas, nem com o fim já ultrapassado.
    pub fn recurrences(&self, actor: MemberId) -> Result<Vec<Recurrence>> {
        let actor = self.actor(actor)?;
        let current = Month::of(self.clock.today()).to_string();
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_RECURRENCE} AND (r.end_month IS NULL OR r.end_month >= ?1)
             ORDER BY r.description COLLATE NOCASE"
        ))?;
        let recurrences = stmt
            .query_map([current], |row| recurrence_from_row(row, &actor))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(recurrences)
    }

    /// Encerra a Recorrência: os previstos futuros somem, e o que já aconteceu
    /// ou está atrasado permanece.
    pub fn end_recurrence(&self, actor: MemberId, id: RecurrenceId) -> Result<()> {
        let actor = self.actor(actor)?;
        self.editable_recurrence(&actor, id)?;
        let now = self.clock.now();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE entry SET deleted_at = ?1, updated_at = ?1, version = version + 1
             WHERE recurrence_id = ?2 AND status = 'planned' AND date > ?3
               AND deleted_at IS NULL",
            params![now, id.to_string(), self.clock.today()],
        )?;
        tx.execute(
            "UPDATE recurrence SET ended_at = ?1, updated_at = ?1, version = version + 1
             WHERE id = ?2",
            params![now, id.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Altera este lançamento e a Recorrência que o gerou, de modo que os
    /// previstos seguintes acompanhem. Efetivados e alterados individualmente
    /// ficam como estão.
    pub fn update_entry_and_following(
        &self,
        actor: MemberId,
        id: EntryId,
        input: EntryInput,
    ) -> Result<Entry> {
        let actor = self.actor(actor)?;
        let current = self.editable_entry(&actor, id)?;
        let recurrence_id = current
            .recurrence_id
            .ok_or_else(|| Error::invalid("Este lançamento não faz parte de uma recorrência."))?;
        self.editable_recurrence(&actor, recurrence_id)?;
        let checked = self.check_entry(&actor, &input, Some(&current))?;
        let occurrence_month: Option<String> = self.conn.query_row(
            "SELECT occurrence_month FROM entry WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )?;
        let now = self.clock.now();
        let day = input.date.day();

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE entry
             SET kind = ?1, amount_cents = ?2, date = ?3, description = ?4, category_id = ?5,
                 assignee_id = ?6, status = ?7, version = version + 1, updated_at = ?8
             WHERE id = ?9",
            params![
                input.kind.as_str(),
                input.amount_cents,
                input.date,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                input.status.unwrap_or(current.status).as_str(),
                now,
                id.to_string()
            ],
        )?;
        tx.execute(
            "UPDATE recurrence
             SET kind = ?1, amount_cents = ?2, description = ?3, category_id = ?4,
                 assignee_id = ?5, day_of_month = ?6, version = version + 1, updated_at = ?7
             WHERE id = ?8",
            params![
                input.kind.as_str(),
                input.amount_cents,
                checked.description,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                day,
                now,
                recurrence_id.to_string()
            ],
        )?;

        let following: Vec<(String, Option<Month>)> = {
            let mut stmt = tx.prepare(
                "SELECT id, occurrence_month FROM entry
                 WHERE recurrence_id = ?1 AND occurrence_month > ?2 AND status = 'planned'
                   AND detached = 0 AND deleted_at IS NULL",
            )?;
            stmt.query_map(
                params![recurrence_id.to_string(), occurrence_month],
                |row| Ok((row.get(0)?, month_col(row, 1)?)),
            )?
            .collect::<rusqlite::Result<_>>()?
        };
        for (entry_id, month) in following {
            let Some(month) = month else { continue };
            tx.execute(
                "UPDATE entry
                 SET kind = ?1, amount_cents = ?2, date = ?3, description = ?4,
                     category_id = ?5, assignee_id = ?6, version = version + 1, updated_at = ?7
                 WHERE id = ?8",
                params![
                    input.kind.as_str(),
                    input.amount_cents,
                    month.day(day),
                    checked.description,
                    input.category_id.to_string(),
                    checked.assignee_id.to_string(),
                    now,
                    entry_id
                ],
            )?;
        }
        tx.commit()?;
        self.entry(&actor, id)
    }

    /// Gera os lançamentos previstos que faltam para cada Recorrência cobrir
    /// os próximos doze meses.
    pub(crate) fn top_up_recurrences(&self) -> Result<()> {
        let horizon = Month::of(self.clock.today()).plus(HORIZON_MONTHS);
        let pending: Vec<PendingRecurrence> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, kind, amount_cents, description, category_id, assignee_id,
                        created_by, day_of_month, start_month, end_month, generated_until
                 FROM recurrence WHERE deleted_at IS NULL AND ended_at IS NULL",
            )?;
            stmt.query_map([], |row| {
                let start = month_col(row, 8)?.expect("coluna NOT NULL");
                let last = month_col(row, 9)?.map_or(horizon, |end| end.min(horizon));
                Ok(PendingRecurrence {
                    id: uuid_col(row, 0)?,
                    kind: EntryKind::from_column(row, 1)?,
                    amount_cents: row.get(2)?,
                    description: row.get(3)?,
                    category_id: uuid_col(row, 4)?,
                    assignee_id: uuid_col(row, 5)?,
                    created_by: uuid_col(row, 6)?,
                    day_of_month: row.get(7)?,
                    next: month_col(row, 10)?.map_or(start, Month::next),
                    last,
                })
            })?
            .collect::<rusqlite::Result<_>>()?
        };

        let now = self.clock.now();
        for recurrence in pending.iter().filter(|r| r.next <= r.last) {
            let tx = self.conn.unchecked_transaction()?;
            let mut month = recurrence.next;
            while month <= recurrence.last {
                insert_entry(
                    &tx,
                    now,
                    &NewEntryRow {
                        kind: recurrence.kind,
                        amount_cents: recurrence.amount_cents,
                        date: month.day(recurrence.day_of_month),
                        description: &recurrence.description,
                        category_id: recurrence.category_id,
                        assignee_id: recurrence.assignee_id,
                        created_by: recurrence.created_by,
                        status: EntryStatus::Planned,
                        recurrence: Some((recurrence.id, month)),
                        installment: None,
                    },
                )?;
                month = month.next();
            }
            tx.execute(
                "UPDATE recurrence SET generated_until = ?1 WHERE id = ?2",
                params![recurrence.last.to_string(), recurrence.id.to_string()],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Divide uma compra em parcelas mensais, todas Despesas previstas.
    pub fn create_installment_plan(
        &self,
        actor: MemberId,
        input: InstallmentPlanInput,
    ) -> Result<InstallmentPlan> {
        let actor = self.actor(actor)?;
        if !(2..=MAX_INSTALLMENTS).contains(&input.count) {
            return Err(Error::Invalid(format!(
                "O número de parcelas precisa ficar entre 2 e {MAX_INSTALLMENTS}."
            )));
        }
        let count = i64::from(input.count);
        if input.total_cents < count {
            return Err(Error::invalid(
                "O valor total é pequeno demais para esse número de parcelas.",
            ));
        }
        let checked = self.check_entry(
            &actor,
            &EntryInput {
                kind: EntryKind::Expense,
                amount_cents: input.total_cents,
                date: input.first_date,
                description: input.description.clone(),
                category_id: input.category_id,
                assignee_id: input.assignee_id,
                status: None,
            },
            None,
        )?;

        let id = Uuid::new_v4();
        let now = self.clock.now();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO installment_plan (id, description, total_cents, count, category_id,
                                           assignee_id, created_by, first_date, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                checked.description,
                input.total_cents,
                input.count,
                input.category_id.to_string(),
                checked.assignee_id.to_string(),
                actor.id.to_string(),
                input.first_date,
                now
            ],
        )?;
        let base = input.total_cents / count;
        let remainder = input.total_cents % count;
        let first_month = Month::of(input.first_date);
        for number in 1..=input.count {
            insert_entry(
                &tx,
                now,
                &NewEntryRow {
                    kind: EntryKind::Expense,
                    // Os centavos que sobram da divisão vão para a primeira parcela.
                    amount_cents: if number == 1 { base + remainder } else { base },
                    date: first_month
                        .plus(number as i32 - 1)
                        .day(input.first_date.day()),
                    description: &checked.description,
                    category_id: input.category_id,
                    assignee_id: checked.assignee_id,
                    created_by: actor.id,
                    status: EntryStatus::Planned,
                    recurrence: None,
                    installment: Some((id, number)),
                },
            )?;
        }
        tx.commit()?;
        self.installment_plan(&actor, id)
    }

    /// Os Parcelamentos que ainda têm parcelas previstas.
    pub fn installment_plans(&self, actor: MemberId) -> Result<Vec<InstallmentPlan>> {
        let actor = self.actor(actor)?;
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_PLAN} GROUP BY p.id HAVING planned_count > 0
             ORDER BY p.first_date DESC, p.description COLLATE NOCASE"
        ))?;
        let plans = stmt
            .query_map([], |row| plan_from_row(row, &actor))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(plans)
    }

    /// Desfaz o Parcelamento: as parcelas previstas somem e as efetivadas permanecem.
    pub fn delete_installment_plan(&self, actor: MemberId, id: InstallmentPlanId) -> Result<()> {
        let actor = self.actor(actor)?;
        let plan = self.installment_plan(&actor, id)?;
        if !plan.can_edit {
            return Err(Error::Forbidden);
        }
        let now = self.clock.now();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE entry SET deleted_at = ?1, updated_at = ?1, version = version + 1
             WHERE installment_plan_id = ?2 AND status = 'planned' AND deleted_at IS NULL",
            params![now, id.to_string()],
        )?;
        tx.execute(
            "UPDATE installment_plan
             SET deleted_at = ?1, updated_at = ?1, version = version + 1 WHERE id = ?2",
            params![now, id.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn recurrence(&self, actor: &Member, id: RecurrenceId) -> Result<Recurrence> {
        self.conn
            .query_row(
                &format!("{SELECT_RECURRENCE} AND r.id = ?1"),
                [id.to_string()],
                |row| recurrence_from_row(row, actor),
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    fn editable_recurrence(&self, actor: &Member, id: RecurrenceId) -> Result<Recurrence> {
        let recurrence = self.recurrence(actor, id)?;
        if !recurrence.can_edit {
            return Err(Error::Forbidden);
        }
        Ok(recurrence)
    }

    fn installment_plan(&self, actor: &Member, id: InstallmentPlanId) -> Result<InstallmentPlan> {
        self.conn
            .query_row(
                &format!("{SELECT_PLAN} AND p.id = ?1 GROUP BY p.id"),
                [id.to_string()],
                |row| plan_from_row(row, actor),
            )
            .optional()?
            .ok_or(Error::NotFound)
    }
}

const SELECT_PLAN: &str = "
    SELECT p.id, p.description, p.total_cents, p.count, c.name, p.assignee_id, m.username,
           p.first_date,
           COUNT(e.id) AS planned_count,
           COALESCE(SUM(e.amount_cents), 0) AS planned_cents
    FROM installment_plan p
    JOIN category c ON c.id = p.category_id
    JOIN member m ON m.id = p.assignee_id
    LEFT JOIN entry e ON e.installment_plan_id = p.id AND e.status = 'planned'
                     AND e.deleted_at IS NULL
    WHERE p.deleted_at IS NULL";

struct PendingRecurrence {
    id: RecurrenceId,
    kind: EntryKind,
    amount_cents: i64,
    description: String,
    category_id: CategoryId,
    assignee_id: MemberId,
    created_by: MemberId,
    day_of_month: u32,
    /// O primeiro mês ainda sem lançamento gerado.
    next: Month,
    /// O último mês a gerar agora: o fim da Recorrência ou o horizonte.
    last: Month,
}

fn as_entry_input(input: &RecurrenceInput) -> EntryInput {
    EntryInput {
        kind: input.kind,
        amount_cents: input.amount_cents,
        date: input.first_date,
        description: input.description.clone(),
        category_id: input.category_id,
        assignee_id: input.assignee_id,
        status: None,
    }
}

fn can_edit(actor: &Member, assignee_id: MemberId) -> bool {
    actor.role == Role::Admin || assignee_id == actor.id
}

fn recurrence_from_row(row: &Row<'_>, actor: &Member) -> rusqlite::Result<Recurrence> {
    let assignee_id = uuid_col(row, 6)?;
    Ok(Recurrence {
        id: uuid_col(row, 0)?,
        kind: EntryKind::from_column(row, 1)?,
        amount_cents: row.get(2)?,
        description: row.get(3)?,
        category_id: uuid_col(row, 4)?,
        category_name: row.get(5)?,
        assignee_id,
        assignee_name: row.get(7)?,
        day_of_month: row.get(8)?,
        start_month: month_col(row, 9)?.expect("coluna NOT NULL"),
        end_month: month_col(row, 10)?,
        can_edit: can_edit(actor, assignee_id),
    })
}

fn plan_from_row(row: &Row<'_>, actor: &Member) -> rusqlite::Result<InstallmentPlan> {
    let assignee_id = uuid_col(row, 5)?;
    Ok(InstallmentPlan {
        id: uuid_col(row, 0)?,
        description: row.get(1)?,
        total_cents: row.get(2)?,
        count: row.get(3)?,
        category_name: row.get(4)?,
        assignee_id,
        assignee_name: row.get(6)?,
        first_date: row.get(7)?,
        planned_count: row.get(8)?,
        planned_cents: row.get(9)?,
        can_edit: can_edit(actor, assignee_id),
    })
}
