use std::collections::HashMap;

use rusqlite::params;
use serde::Serialize;

use crate::entries::EntryKind;
use crate::household::uuid_col;
use crate::{CategoryId, Household, MemberId, Month, Result};

/// Quantos meses o fluxo mostra, terminando no mês consultado.
const FLOW_MONTHS: i32 = 6;

/// Os números de cada bloco do dashboard, já agregados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub month: Month,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub balance_cents: i64,
    /// Despesas do mês por Categoria, da maior para a menor.
    pub expenses_by_category: Vec<CategoryTotal>,
    /// Despesas do mês por Responsável, da maior para a menor.
    pub expenses_by_assignee: Vec<AssigneeTotal>,
    /// Um item por mês, em ordem cronológica, inclusive os meses sem lançamentos.
    pub flow: Vec<MonthFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category_id: CategoryId,
    pub name: String,
    pub total_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssigneeTotal {
    pub member_id: MemberId,
    pub name: String,
    pub total_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthFlow {
    pub month: Month,
    pub income_cents: i64,
    pub expense_cents: i64,
    /// Receitas menos despesas, somadas desde o primeiro mês do fluxo.
    pub accumulated_balance_cents: i64,
}

impl Household {
    pub fn dashboard(&self, actor: MemberId, month: Month) -> Result<Dashboard> {
        self.actor(actor)?;
        let start = month.first_day();
        let end = month.next().first_day();

        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, SUM(e.amount_cents) AS total
             FROM entry e JOIN category c ON c.id = e.category_id
             WHERE e.deleted_at IS NULL AND e.kind = 'expense' AND e.date >= ?1 AND e.date < ?2
             GROUP BY c.id ORDER BY total DESC, c.name",
        )?;
        let expenses_by_category = stmt
            .query_map(params![start, end], |row| {
                Ok(CategoryTotal {
                    category_id: uuid_col(row, 0)?,
                    name: row.get(1)?,
                    total_cents: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.username, SUM(e.amount_cents) AS total
             FROM entry e JOIN member m ON m.id = e.assignee_id
             WHERE e.deleted_at IS NULL AND e.kind = 'expense' AND e.date >= ?1 AND e.date < ?2
             GROUP BY m.id ORDER BY total DESC, m.username",
        )?;
        let expenses_by_assignee = stmt
            .query_map(params![start, end], |row| {
                Ok(AssigneeTotal {
                    member_id: uuid_col(row, 0)?,
                    name: row.get(1)?,
                    total_cents: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let flow = self.flow(month.plus(1 - FLOW_MONTHS), month)?;
        let current = flow.last().expect("o fluxo inclui o mês consultado");
        Ok(Dashboard {
            month,
            income_cents: current.income_cents,
            expense_cents: current.expense_cents,
            balance_cents: current.income_cents - current.expense_cents,
            expenses_by_category,
            expenses_by_assignee,
            flow,
        })
    }

    fn flow(&self, first: Month, last: Month) -> Result<Vec<MonthFlow>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(date, 1, 7), kind, SUM(amount_cents)
             FROM entry
             WHERE deleted_at IS NULL AND date >= ?1 AND date < ?2
             GROUP BY 1, 2",
        )?;
        let mut totals: HashMap<(String, EntryKind), i64> = HashMap::new();
        let rows = stmt.query_map(params![first.first_day(), last.next().first_day()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                EntryKind::from_column(row, 1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (month, kind, total) = row?;
            totals.insert((month, kind), total);
        }

        let mut flow = Vec::new();
        let mut accumulated = 0;
        let mut month = first;
        while month <= last {
            let total = |kind| totals.get(&(month.to_string(), kind)).copied().unwrap_or(0);
            let income_cents = total(EntryKind::Income);
            let expense_cents = total(EntryKind::Expense);
            accumulated += income_cents - expense_cents;
            flow.push(MonthFlow {
                month,
                income_cents,
                expense_cents,
                accumulated_balance_cents: accumulated,
            });
            month = month.next();
        }
        Ok(flow)
    }
}
