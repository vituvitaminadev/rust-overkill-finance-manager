use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entries::EntryKind;
use crate::household::uuid_col;
use crate::{CategoryId, Error, Household, MemberId, Result};

pub(crate) const DEFAULT_CATEGORIES: &[(&str, EntryKind)] = &[
    ("Moradia", EntryKind::Expense),
    ("Mercado", EntryKind::Expense),
    ("Transporte", EntryKind::Expense),
    ("Saúde", EntryKind::Expense),
    ("Educação", EntryKind::Expense),
    ("Lazer", EntryKind::Expense),
    ("Contas e serviços", EntryKind::Expense),
    ("Outras despesas", EntryKind::Expense),
    ("Salário", EntryKind::Income),
    ("Outras receitas", EntryKind::Income),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub kind: EntryKind,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
    pub name: String,
    pub kind: EntryKind,
}

impl Household {
    /// Todas as categorias, inclusive as arquivadas.
    pub fn categories(&self, actor: MemberId) -> Result<Vec<Category>> {
        self.actor(actor)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, archived FROM category
             WHERE deleted_at IS NULL ORDER BY kind, name COLLATE NOCASE",
        )?;
        let categories = stmt
            .query_map([], category_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(categories)
    }

    pub fn create_category(&self, actor: MemberId, new: NewCategory) -> Result<Category> {
        self.host_only()?;
        self.admin(actor)?;
        let name = self.available_category_name(&new.name, new.kind, None)?;
        let id = Uuid::new_v4();
        self.conn.execute(
            "INSERT INTO category (id, name, kind, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id.to_string(), name, new.kind.as_str(), self.clock.now()],
        )?;
        self.category(id)
    }

    pub fn rename_category(&self, actor: MemberId, id: CategoryId, name: &str) -> Result<Category> {
        self.host_only()?;
        self.admin(actor)?;
        let category = self.category(id)?;
        let name = self.available_category_name(name, category.kind, Some(id))?;
        self.conn.execute(
            "UPDATE category SET name = ?1, version = version + 1, updated_at = ?2 WHERE id = ?3",
            params![name, self.clock.now(), id.to_string()],
        )?;
        self.category(id)
    }

    /// Categorias nunca são excluídas: arquivar preserva a classificação dos lançamentos antigos.
    pub fn set_category_archived(
        &self,
        actor: MemberId,
        id: CategoryId,
        archived: bool,
    ) -> Result<Category> {
        self.host_only()?;
        self.admin(actor)?;
        self.category(id)?;
        self.conn.execute(
            "UPDATE category SET archived = ?1, version = version + 1, updated_at = ?2
             WHERE id = ?3",
            params![archived, self.clock.now(), id.to_string()],
        )?;
        self.category(id)
    }

    pub(crate) fn category(&self, id: CategoryId) -> Result<Category> {
        self.conn
            .query_row(
                "SELECT id, name, kind, archived FROM category
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id.to_string()],
                category_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    fn available_category_name(
        &self,
        name: &str,
        kind: EntryKind,
        except: Option<CategoryId>,
    ) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::invalid("Informe o nome da categoria."));
        }
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM category
                 WHERE name = ?1 COLLATE NOCASE AND kind = ?2 AND id IS NOT ?3
                   AND deleted_at IS NULL
             )",
            params![name, kind.as_str(), except.map(|id| id.to_string())],
            |row| row.get(0),
        )?;
        if taken {
            return Err(Error::invalid("Já existe uma categoria com esse nome."));
        }
        Ok(name.to_owned())
    }
}

fn category_from_row(row: &Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: uuid_col(row, 0)?,
        name: row.get(1)?,
        kind: EntryKind::from_column(row, 2)?,
        archived: row.get(3)?,
    })
}
