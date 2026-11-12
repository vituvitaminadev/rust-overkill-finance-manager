use std::sync::Arc;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration, Utc};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::categories::DEFAULT_CATEGORIES;
use crate::{Clock, Error, HouseholdId, MemberId, Result, schema};

const INVITE_VALIDITY_HOURS: i64 = 24;
// Sem I, O, 0 e 1, que se confundem ao ditar o código.
const INVITE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const MIN_PASSWORD_CHARS: usize = 8;

/// Uma Residência aberta: a interface de aplicação sobre o seu banco de dados.
pub struct Household {
    pub(crate) conn: Connection,
    pub(crate) clock: Arc<dyn Clock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdInfo {
    pub id: HouseholdId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Common,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Common => "common",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: MemberId,
    pub username: String,
    pub role: Role,
    pub active: bool,
}

/// As credenciais de um Usuário, para levá-lo a outra residência sem pedir a senha de novo.
#[derive(Debug, Clone)]
pub struct UserCredentials {
    user_id: Uuid,
    username: String,
    password_hash: String,
}

pub enum NewUser {
    New { username: String, password: String },
    Existing(UserCredentials),
}

pub struct NewHousehold {
    pub name: String,
    pub user: NewUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

impl Household {
    /// Cria a residência em um banco vazio; quem cria é o primeiro Administrador.
    pub fn create(
        conn: Connection,
        clock: Arc<dyn Clock>,
        new: NewHousehold,
    ) -> Result<(Household, Member)> {
        let name = new.name.trim();
        if name.is_empty() {
            return Err(Error::invalid("Informe o nome da residência."));
        }
        let credentials = match new.user {
            NewUser::New { username, password } => new_credentials(&username, &password)?,
            NewUser::Existing(credentials) => credentials,
        };

        schema::migrate(&conn)?;
        let household = Household { conn, clock };
        let now = household.clock.now();
        let tx = household.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO household (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![Uuid::new_v4().to_string(), name, now],
        )?;
        let member_id = insert_member(&tx, &credentials, Role::Admin, now)?;
        for (name, kind) in DEFAULT_CATEGORIES {
            tx.execute(
                "INSERT INTO category (id, name, kind, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![Uuid::new_v4().to_string(), name, kind.as_str(), now],
            )?;
        }
        tx.commit()?;

        let member = household.member(member_id)?;
        Ok((household, member))
    }

    pub fn open(conn: Connection, clock: Arc<dyn Clock>) -> Result<Household> {
        schema::migrate(&conn)?;
        Ok(Household { conn, clock })
    }

    pub fn info(&self) -> Result<HouseholdInfo> {
        Ok(self
            .conn
            .query_row("SELECT id, name FROM household", [], |row| {
                Ok(HouseholdInfo {
                    id: uuid_col(row, 0)?,
                    name: row.get(1)?,
                })
            })?)
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Result<Member> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT id, password_hash FROM member
                 WHERE username = ?1 AND active = 1 AND deleted_at IS NULL",
                [username.trim()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (id, hash) = found.ok_or(Error::InvalidCredentials)?;
        let hash = PasswordHash::new(&hash).map_err(|e| Error::PasswordHash(e.to_string()))?;
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .map_err(|_| Error::InvalidCredentials)?;
        self.member(Uuid::parse_str(&id).map_err(|_| Error::NotFound)?)
    }

    pub fn member(&self, id: MemberId) -> Result<Member> {
        self.conn
            .query_row(
                "SELECT id, username, role, active FROM member
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id.to_string()],
                member_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub fn members(&self, actor: MemberId) -> Result<Vec<Member>> {
        self.actor(actor)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, username, role, active FROM member
             WHERE deleted_at IS NULL ORDER BY username",
        )?;
        let members = stmt
            .query_map([], member_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(members)
    }

    pub fn user_credentials(&self, actor: MemberId) -> Result<UserCredentials> {
        self.actor(actor)?;
        Ok(self.conn.query_row(
            "SELECT user_id, username, password_hash FROM member WHERE id = ?1",
            [actor.to_string()],
            |row| {
                Ok(UserCredentials {
                    user_id: uuid_col(row, 0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                })
            },
        )?)
    }

    pub fn create_invite(&self, actor: MemberId) -> Result<Invite> {
        self.admin(actor)?;
        let now = self.clock.now();
        let invite = Invite {
            code: invite_code(),
            expires_at: now + Duration::hours(INVITE_VALIDITY_HOURS),
        };
        self.conn.execute(
            "INSERT INTO invite (id, code, created_by, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                invite.code,
                actor.to_string(),
                now,
                invite.expires_at
            ],
        )?;
        Ok(invite)
    }

    /// Consome o Convite e cria um membro Comum com as credenciais informadas.
    pub fn accept_invite(&self, code: &str, username: &str, password: &str) -> Result<Member> {
        let now = self.clock.now();
        let code = code.trim().to_uppercase();
        let found: Option<(String, DateTime<Utc>, Option<DateTime<Utc>>)> = self
            .conn
            .query_row(
                "SELECT id, expires_at, used_at FROM invite WHERE code = ?1",
                [&code],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (invite_id, expires_at, used_at) = found.ok_or(Error::InviteInvalid)?;
        if used_at.is_some() {
            return Err(Error::InviteUsed);
        }
        if expires_at <= now {
            return Err(Error::InviteExpired);
        }

        let credentials = new_credentials(username, password)?;
        let tx = self.conn.unchecked_transaction()?;
        let member_id = insert_member(&tx, &credentials, Role::Common, now)?;
        tx.execute(
            "UPDATE invite SET used_at = ?1, used_by = ?2 WHERE id = ?3",
            params![now, member_id.to_string(), invite_id],
        )?;
        tx.commit()?;
        self.member(member_id)
    }

    /// O membro em nome de quem a operação roda; quem não é membro ativo não faz nada.
    pub(crate) fn actor(&self, id: MemberId) -> Result<Member> {
        match self.member(id) {
            Ok(member) if member.active => Ok(member),
            Ok(_) | Err(Error::NotFound) => Err(Error::Forbidden),
            Err(other) => Err(other),
        }
    }

    pub(crate) fn admin(&self, id: MemberId) -> Result<Member> {
        let member = self.actor(id)?;
        if member.role != Role::Admin {
            return Err(Error::Forbidden);
        }
        Ok(member)
    }
}

fn new_credentials(username: &str, password: &str) -> Result<UserCredentials> {
    let username = username.trim();
    if username.is_empty() {
        return Err(Error::invalid("Informe um nome de usuário."));
    }
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(Error::Invalid(format!(
            "A senha precisa ter pelo menos {MIN_PASSWORD_CHARS} caracteres."
        )));
    }
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| Error::PasswordHash(e.to_string()))?
        .to_string();
    Ok(UserCredentials {
        user_id: Uuid::new_v4(),
        username: username.to_owned(),
        password_hash,
    })
}

fn insert_member(
    conn: &Connection,
    credentials: &UserCredentials,
    role: Role,
    now: DateTime<Utc>,
) -> Result<MemberId> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM member WHERE username = ?1)",
        [&credentials.username],
        |row| row.get(0),
    )?;
    if taken {
        return Err(Error::UsernameTaken);
    }
    let id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO member (id, user_id, username, password_hash, role, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.to_string(),
            credentials.user_id.to_string(),
            credentials.username,
            credentials.password_hash,
            role.as_str(),
            now
        ],
    )?;
    Ok(id)
}

fn invite_code() -> String {
    // 32 símbolos dividem 256 sem resto, então o módulo não enviesa a escolha.
    Uuid::new_v4().as_bytes()[..8]
        .iter()
        .map(|byte| INVITE_ALPHABET[usize::from(byte % 32)] as char)
        .collect()
}

fn member_from_row(row: &Row<'_>) -> rusqlite::Result<Member> {
    let role: String = row.get(2)?;
    Ok(Member {
        id: uuid_col(row, 0)?,
        username: row.get(1)?,
        role: if role == "admin" {
            Role::Admin
        } else {
            Role::Common
        },
        active: row.get(3)?,
    })
}

pub(crate) fn uuid_col(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let text: String = row.get(index)?;
    Uuid::parse_str(&text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
}

pub(crate) fn opt_uuid_col(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Uuid>> {
    let text: Option<String> = row.get(index)?;
    text.map(|text| {
        Uuid::parse_str(&text)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
    })
    .transpose()
}
