use rusqlite::Connection;

use crate::{Error, Result};

/// Cada item leva o banco da versão do seu índice para a seguinte.
///
/// Todo registro sincronizável carrega `version` e `deleted_at` desde já,
/// para que a sincronização não exija migração estrutural (ADR 0001).
const MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE household (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        created_at  TEXT NOT NULL
    );

    CREATE TABLE member (
        id             TEXT PRIMARY KEY,
        user_id        TEXT NOT NULL UNIQUE,
        username       TEXT NOT NULL UNIQUE COLLATE NOCASE,
        password_hash  TEXT NOT NULL,
        role           TEXT NOT NULL CHECK (role IN ('admin', 'common')),
        active         INTEGER NOT NULL DEFAULT 1,
        version        INTEGER NOT NULL DEFAULT 1,
        updated_at     TEXT NOT NULL,
        deleted_at     TEXT
    );

    CREATE TABLE category (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        kind        TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
        archived    INTEGER NOT NULL DEFAULT 0,
        version     INTEGER NOT NULL DEFAULT 1,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT
    );

    CREATE TABLE entry (
        id            TEXT PRIMARY KEY,
        kind          TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
        amount_cents  INTEGER NOT NULL CHECK (amount_cents > 0),
        date          TEXT NOT NULL,
        description   TEXT NOT NULL,
        category_id   TEXT NOT NULL REFERENCES category (id),
        assignee_id   TEXT NOT NULL REFERENCES member (id),
        created_by    TEXT NOT NULL REFERENCES member (id),
        version       INTEGER NOT NULL DEFAULT 1,
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL,
        deleted_at    TEXT
    );
    CREATE INDEX entry_date ON entry (date);

    CREATE TABLE invite (
        id          TEXT PRIMARY KEY,
        code        TEXT NOT NULL UNIQUE,
        created_by  TEXT NOT NULL REFERENCES member (id),
        created_at  TEXT NOT NULL,
        expires_at  TEXT NOT NULL,
        used_at     TEXT,
        used_by     TEXT REFERENCES member (id)
    );
",
    // Previsibilidade: situação do lançamento, Recorrência e Parcelamento.
    "
    CREATE TABLE recurrence (
        id               TEXT PRIMARY KEY,
        kind             TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
        amount_cents     INTEGER NOT NULL CHECK (amount_cents > 0),
        description      TEXT NOT NULL,
        category_id      TEXT NOT NULL REFERENCES category (id),
        assignee_id      TEXT NOT NULL REFERENCES member (id),
        created_by       TEXT NOT NULL REFERENCES member (id),
        day_of_month     INTEGER NOT NULL,
        start_month      TEXT NOT NULL,
        end_month        TEXT,
        -- Último mês cujo lançamento já foi gerado; os anteriores não são gerados de novo.
        generated_until  TEXT,
        ended_at         TEXT,
        version          INTEGER NOT NULL DEFAULT 1,
        updated_at       TEXT NOT NULL,
        deleted_at       TEXT
    );

    CREATE TABLE installment_plan (
        id            TEXT PRIMARY KEY,
        description   TEXT NOT NULL,
        total_cents   INTEGER NOT NULL CHECK (total_cents > 0),
        count         INTEGER NOT NULL CHECK (count > 1),
        category_id   TEXT NOT NULL REFERENCES category (id),
        assignee_id   TEXT NOT NULL REFERENCES member (id),
        created_by    TEXT NOT NULL REFERENCES member (id),
        first_date    TEXT NOT NULL,
        version       INTEGER NOT NULL DEFAULT 1,
        updated_at    TEXT NOT NULL,
        deleted_at    TEXT
    );

    ALTER TABLE entry ADD COLUMN status TEXT NOT NULL DEFAULT 'settled'
        CHECK (status IN ('planned', 'settled'));
    ALTER TABLE entry ADD COLUMN recurrence_id TEXT REFERENCES recurrence (id);
    ALTER TABLE entry ADD COLUMN occurrence_month TEXT;
    -- Alterado individualmente: mudanças na Recorrência não o alcançam mais.
    ALTER TABLE entry ADD COLUMN detached INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE entry ADD COLUMN installment_plan_id TEXT REFERENCES installment_plan (id);
    ALTER TABLE entry ADD COLUMN installment_number INTEGER;
",
    // Sincronização (ADR 0001): relógio de alterações do Anfitrião, alterações
    // pendentes e conflitos da cópia, sessões e estado local desta máquina.
    "
    CREATE TABLE sync_clock (value INTEGER NOT NULL);
    INSERT INTO sync_clock VALUES (1);

    ALTER TABLE household ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE member ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE category ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE recurrence ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE installment_plan ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE entry ADD COLUMN seq INTEGER NOT NULL DEFAULT 1;

    -- Só na cópia: quantas alterações locais ainda não chegaram ao Anfitrião,
    -- e de qual versão dele elas partiram.
    ALTER TABLE entry ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE entry ADD COLUMN base_version INTEGER;

    CREATE TABLE conflict (
        entry_id      TEXT PRIMARY KEY,
        -- A linha do Anfitrião em JSON; nula quando ele não tem o lançamento.
        host_row      TEXT,
        host_version  INTEGER NOT NULL,
        -- Preenchido quando o Anfitrião recusou a alteração em vez de divergir dela.
        reason        TEXT
    );

    ALTER TABLE invite ADD COLUMN revoked_at TEXT;

    CREATE TABLE host_session (
        token_hash  TEXT PRIMARY KEY,
        member_id   TEXT NOT NULL REFERENCES member (id),
        created_at  TEXT NOT NULL
    );

    -- Nunca sincronizado: papel desta máquina, cursor, identidade TLS, credenciais.
    CREATE TABLE local_state (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );
",
];

/// As tabelas sincronizadas, em uma ordem que respeita as chaves estrangeiras.
pub(crate) const SYNCED_TABLES: &[&str] = &[
    "household",
    "member",
    "category",
    "recurrence",
    "installment_plan",
    "entry",
];

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = current as usize;
    if current > MIGRATIONS.len() {
        return Err(Error::NewerSchema);
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    create_seq_triggers(conn)?;
    Ok(())
}

/// Toda escrita em uma tabela sincronizada carimba a linha com o próximo valor
/// do relógio, e é por esse carimbo que as cópias pedem "o que mudou desde X".
/// Quem grava `seq` explicitamente, como a cópia ao aplicar o que recebeu,
/// não dispara o carimbo.
fn create_seq_triggers(conn: &Connection) -> Result<()> {
    for table in SYNCED_TABLES {
        let stamp = format!(
            "UPDATE sync_clock SET value = value + 1;
             UPDATE {table} SET seq = (SELECT value FROM sync_clock) WHERE id = NEW.id;"
        );
        conn.execute_batch(&format!(
            "CREATE TRIGGER IF NOT EXISTS {table}_seq_insert AFTER INSERT ON {table}
             BEGIN {stamp} END;
             CREATE TRIGGER IF NOT EXISTS {table}_seq_update AFTER UPDATE ON {table}
             WHEN NEW.seq IS OLD.seq
             BEGIN {stamp} END;"
        ))?;
    }
    Ok(())
}
