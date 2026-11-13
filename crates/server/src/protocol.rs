//! As mensagens trocadas entre uma cópia e o Anfitrião, em JSON.

use finance_core::{ChangeSet, EntryChange, HostCommand, HouseholdInfo, Member, PushOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientMessage {
    /// Devolvido na resposta, para casar uma com a outra.
    pub id: u64,
    pub request: Request,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Sempre a primeira mensagem: sem versões de protocolo iguais, nada mais é aceito.
    Hello {
        protocol: u32,
    },
    Join {
        code: String,
        username: String,
        password: String,
    },
    Login {
        username: String,
        password: String,
    },
    Resume {
        token: String,
    },
    Pull {
        since: i64,
    },
    Push {
        changes: Vec<EntryChange>,
    },
    Command {
        command: HostCommand,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Reply {
        id: u64,
        result: Result<Response, WireError>,
    },
    /// Algo mudou na residência; a cópia deve pedir as alterações.
    Changed { seq: i64 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Hello {
        protocol: u32,
        household: HouseholdInfo,
    },
    Session {
        token: String,
        member: Member,
    },
    Resumed {
        member: Member,
    },
    Changes {
        changes: ChangeSet,
    },
    Pushed {
        outcomes: Vec<PushOutcome>,
    },
    Done {
        value: Value,
    },
}

/// Um erro do Anfitrião, com o mesmo código e mensagem que teria localmente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireError {
    pub code: String,
    pub message: String,
}

pub const PROTOCOL_MISMATCH: &str = "protocol_mismatch";
pub const UNAUTHENTICATED: &str = "invalid_session";

impl From<finance_core::Error> for WireError {
    fn from(error: finance_core::Error) -> Self {
        WireError {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}
