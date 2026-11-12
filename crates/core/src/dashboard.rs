use std::collections::HashMap;

use rusqlite::params;
use serde::Serialize;

use crate::entries::{EntryKind, EntryStatus};
use crate::household::uuid_col;
use crate::{CategoryId, Entry, Household, MemberId, Month, Result};

/// O fluxo mostra o mês consultado, os cinco anteriores e os seis seguintes.
const FLOW_MONTHS_BEFORE: i32 = 5;
const FLOW_MONTHS_AFTER: i32 = 6;
const UPCOMING_LIMIT: usize = 8;

/// Os números de cada bloco do dashboard, já agregados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub month: Month,
    pub income: Totals,
    pub expense: Totals,
    /// Receitas menos despesas do mês.
    pub balance: Totals,
    /// Despesas do mês por Categoria, da maior para a menor.
    pub expenses_by_category: Vec<CategoryTotal>,
    /// Despesas do mês por Responsável, da maior para a menor.
    pub expenses_by_assignee: Vec<AssigneeTotal>,
    /// Um item por mês, em ordem cronológica, inclusive os meses sem lançamentos.
    pub flow: Vec<MonthFlow>,
    /// Os próximos lançamentos previstos, a partir de hoje, do mais próximo ao mais distante.
    pub upcoming: Vec<Entry>,
    /// Todos os previstos cuja data já passou, do mais antigo ao mais recente.
    pub overdue: Vec<Entry>,
}

/// Um valor separado no que já aconteceu e no que ainda está previsto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub settled_cents: i64,
    pub planned_cents: i64,
}

impl Totals {
    pub fn total_cents(self) -> i64 {
        self.settled_cents + self.planned_cents
    }

    fn add(&mut self, status: EntryStatus, cents: i64) {
        match status {
            EntryStatus::Settled => self.settled_cents += cents,
            EntryStatus::Planned => self.planned_cents += cents,
        }
    }

    fn minus(self, other: Totals) -> Totals {
        Totals {
            settled_cents: self.settled_cents - other.settled_cents,
            planned_cents: self.planned_cents - other.planned_cents,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category_id: CategoryId,
    pub name: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssigneeTotal {
    pub member_id: MemberId,
    pub name: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthFlow {
    pub month: Month,
    pub income: Totals,
    pub expense: Totals,
    /// Receitas menos despesas, previstas inclusive, somadas desde o primeiro mês do fluxo.
    pub accumulated_balance_cents: i64,
}

impl Household {
    pub fn dashboard(&self, actor: MemberId, month: Month) -> Result<Dashboard> {
        let actor = self.actor(actor)?;
        self.top_up_recurrences()?;

        let flow = self.flow(
            month.plus(-FLOW_MONTHS_BEFORE),
            month.plus(FLOW_MONTHS_AFTER),
        )?;
        let current = flow
            .iter()
            .find(|item| item.month == month)
            .expect("o fluxo inclui o mês consultado");
        let (income, expense) = (current.income, current.expense);

        let expenses_by_category = self
            .expense_breakdown("category c ON c.id = e.category_id", "c.id, c.name", month)?
            .into_iter()
            .map(|(category_id, name, totals)| CategoryTotal {
                category_id,
                name,
                totals,
            })
            .collect();
        let expenses_by_assignee = self
            .expense_breakdown(
                "member c ON c.id = e.assignee_id",
                "c.id, c.username",
                month,
            )?
            .into_iter()
            .map(|(member_id, name, totals)| AssigneeTotal {
                member_id,
                name,
                totals,
            })
            .collect();

        let today = self.clock.today();
        let mut upcoming = self.select_entries(
            &actor,
            " AND e.status = 'planned' AND e.date >= ? ORDER BY e.date, e.created_at",
            vec![Box::new(today)],
        )?;
        upcoming.truncate(UPCOMING_LIMIT);
        let overdue = self.select_entries(
            &actor,
            " AND e.status = 'planned' AND e.date < ? ORDER BY e.date, e.created_at",
            vec![Box::new(today)],
        )?;

        Ok(Dashboard {
            month,
            income,
            expense,
            balance: income.minus(expense),
            expenses_by_category,
            expenses_by_assignee,
            flow,
            upcoming,
            overdue,
        })
    }

    /// As despesas do mês agrupadas pela tabela em `join`, da maior para a menor.
    fn expense_breakdown(
        &self,
        join: &str,
        columns: &str,
        month: Month,
    ) -> Result<Vec<(uuid::Uuid, String, Totals)>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {columns}, e.status, SUM(e.amount_cents)
             FROM entry e JOIN {join}
             WHERE e.deleted_at IS NULL AND e.kind = 'expense' AND e.date >= ?1 AND e.date < ?2
             GROUP BY 1, e.status"
        ))?;
        let rows = stmt.query_map(
            params![month.first_day(), month.next().first_day()],
            |row| {
                Ok((
                    uuid_col(row, 0)?,
                    row.get::<_, String>(1)?,
                    EntryStatus::from_column(row, 2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?;
        let mut groups: Vec<(uuid::Uuid, String, Totals)> = Vec::new();
        for row in rows {
            let (id, name, status, cents) = row?;
            match groups.iter_mut().find(|group| group.0 == id) {
                Some(group) => group.2.add(status, cents),
                None => {
                    let mut totals = Totals::default();
                    totals.add(status, cents);
                    groups.push((id, name, totals));
                }
            }
        }
        groups.sort_by(|a, b| {
            b.2.total_cents()
                .cmp(&a.2.total_cents())
                .then_with(|| a.1.cmp(&b.1))
        });
        Ok(groups)
    }

    fn flow(&self, first: Month, last: Month) -> Result<Vec<MonthFlow>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(date, 1, 7), kind, status, SUM(amount_cents)
             FROM entry
             WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2
             GROUP BY 1, 2, 3",
        )?;
        let mut totals: HashMap<(String, EntryKind), Totals> = HashMap::new();
        let rows = stmt.query_map(params![first.first_day(), last.next().first_day()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                EntryKind::from_column(row, 1)?,
                EntryStatus::from_column(row, 2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows {
            let (month, kind, status, cents) = row?;
            totals.entry((month, kind)).or_default().add(status, cents);
        }

        let mut flow = Vec::new();
        let mut accumulated = 0;
        let mut month = first;
        while month <= last {
            let total = |kind| {
                totals
                    .get(&(month.to_string(), kind))
                    .copied()
                    .unwrap_or_default()
            };
            let income = total(EntryKind::Income);
            let expense = total(EntryKind::Expense);
            accumulated += income.total_cents() - expense.total_cents();
            flow.push(MonthFlow {
                month,
                income,
                expense,
                accumulated_balance_cents: accumulated,
            });
            month = month.next();
        }
        Ok(flow)
    }
}
