//! Gestão de membros, convites pendentes, senhas e sessões das cópias.

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::household::{NO_PASSWORD, Role, hash_password, password_matches, uuid_col};
use crate::{Error, Household, Member, MemberId, Result};

/// Um Convite ainda utilizável: nem usado, nem revogado, nem expirado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInvite {
    pub id: Uuid,
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

impl Household {
    pub fn invites(&self, actor: MemberId) -> Result<Vec<PendingInvite>> {
        self.host_only()?;
        self.admin(actor)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, code, expires_at FROM invite
             WHERE used_at IS NULL AND revoked_at IS NULL AND expires_at > ?1
             ORDER BY expires_at",
        )?;
        let invites = stmt
            .query_map([self.clock.now()], |row| {
                Ok(PendingInvite {
                    id: uuid_col(row, 0)?,
                    code: row.get(1)?,
                    expires_at: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(invites)
    }

    pub fn revoke_invite(&self, actor: MemberId, id: Uuid) -> Result<()> {
        self.host_only()?;
        self.admin(actor)?;
        let revoked = self.conn.execute(
            "UPDATE invite SET revoked_at = ?1
             WHERE id = ?2 AND used_at IS NULL AND revoked_at IS NULL",
            params![self.clock.now(), id.to_string()],
        )?;
        if revoked == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Acrescenta à residência alguém que não entra no app: um participante a
    /// quem os lançamentos podem ser atribuídos. Sem senha, ninguém entra em
    /// nome dele até que um Administrador defina uma.
    pub fn add_member(&self, actor: MemberId, name: &str) -> Result<Member> {
        self.host_only()?;
        self.admin(actor)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::invalid("Informe o nome do participante."));
        }
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM member WHERE username = ?1)",
            [name],
            |row| row.get(0),
        )?;
        if taken {
            return Err(Error::UsernameTaken);
        }
        let id = Uuid::new_v4();
        self.conn.execute(
            "INSERT INTO member (id, user_id, username, password_hash, role, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'common', ?5)",
            params![
                id.to_string(),
                Uuid::new_v4().to_string(),
                name,
                NO_PASSWORD,
                self.clock.now()
            ],
        )?;
        self.member(id)
    }

    pub fn set_member_role(&self, actor: MemberId, id: MemberId, role: Role) -> Result<Member> {
        self.host_only()?;
        self.admin(actor)?;
        let member = self.active_member(id)?;
        if member.role == Role::Admin && role != Role::Admin {
            self.keep_an_admin(id)?;
        }
        self.conn.execute(
            "UPDATE member SET role = ?1, version = version + 1, updated_at = ?2 WHERE id = ?3",
            params![role.as_str(), self.clock.now(), id.to_string()],
        )?;
        self.member(id)
    }

    /// Tira o acesso do membro. Os lançamentos dele continuam existindo e
    /// atribuídos a ele.
    pub fn remove_member(&self, actor: MemberId, id: MemberId) -> Result<()> {
        self.host_only()?;
        self.admin(actor)?;
        let member = self.active_member(id)?;
        if member.role == Role::Admin {
            self.keep_an_admin(id)?;
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE member SET active = 0, version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![self.clock.now(), id.to_string()],
        )?;
        tx.execute(
            "DELETE FROM host_session WHERE member_id = ?1",
            [id.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Um Administrador define uma senha nova para outro membro que esqueceu a sua.
    pub fn reset_password(&self, actor: MemberId, id: MemberId, password: &str) -> Result<()> {
        self.host_only()?;
        self.admin(actor)?;
        self.active_member(id)?;
        self.store_password(id, password)
    }

    pub fn change_password(&self, actor: MemberId, current: &str, new: &str) -> Result<()> {
        self.host_only()?;
        self.actor(actor)?;
        let hash: String = self.conn.query_row(
            "SELECT password_hash FROM member WHERE id = ?1",
            [actor.to_string()],
            |row| row.get(0),
        )?;
        if !password_matches(&hash, current)? {
            return Err(Error::InvalidCredentials);
        }
        self.store_password(actor, new)
    }

    /// Emite a credencial com que uma cópia fala em nome do membro.
    pub fn issue_token(&self, member: MemberId) -> Result<String> {
        self.host_only()?;
        self.actor(member)?;
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        self.conn.execute(
            "INSERT INTO host_session (token_hash, member_id, created_at) VALUES (?1, ?2, ?3)",
            params![token_hash(&token), member.to_string(), self.clock.now()],
        )?;
        Ok(token)
    }

    /// O membro dono da credencial, se ela existe e ele continua ativo.
    pub fn authenticate_token(&self, token: &str) -> Result<Member> {
        let member_id = self
            .conn
            .query_row(
                "SELECT member_id FROM host_session WHERE token_hash = ?1",
                [token_hash(token)],
                |row| uuid_col(row, 0),
            )
            .optional()?
            .ok_or(Error::InvalidSession)?;
        self.actor(member_id).map_err(|_| Error::InvalidSession)
    }

    fn active_member(&self, id: MemberId) -> Result<Member> {
        match self.member(id)? {
            member if member.active => Ok(member),
            _ => Err(Error::NotFound),
        }
    }

    /// Falha se `leaving` for o único Administrador ativo.
    fn keep_an_admin(&self, leaving: MemberId) -> Result<()> {
        let others: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM member
             WHERE role = 'admin' AND active = 1 AND deleted_at IS NULL AND id != ?1",
            [leaving.to_string()],
            |row| row.get(0),
        )?;
        if others == 0 {
            return Err(Error::LastAdmin);
        }
        Ok(())
    }

    fn store_password(&self, id: MemberId, password: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE member SET password_hash = ?1, version = version + 1, updated_at = ?2
             WHERE id = ?3",
            params![hash_password(password)?, self.clock.now(), id.to_string()],
        )?;
        Ok(())
    }
}

fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
