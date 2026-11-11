//! "Manter conectado": quem entrou por último fica guardado no cofre de
//! credenciais do sistema operacional.
//!
//! Sem cofre disponível (Linux sem Secret Service, por exemplo) nada é
//! lembrado e o app simplesmente pede a senha na próxima abertura.

use finance_core::{HouseholdId, MemberId};
use serde::{Deserialize, Serialize};

const SERVICE: &str = "dev.vituvitamina.financemanager";
const ACCOUNT: &str = "session";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remembered {
    pub username: String,
    pub memberships: Vec<(HouseholdId, MemberId)>,
}

pub fn load() -> Option<Remembered> {
    let secret = keyring::Entry::new(SERVICE, ACCOUNT)
        .ok()?
        .get_password()
        .ok()?;
    serde_json::from_str(&secret).ok()
}

pub fn save(remembered: &Remembered) {
    let Ok(secret) = serde_json::to_string(remembered) else {
        return;
    };
    if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
        let _ = entry.set_password(&secret);
    }
}

pub fn clear() {
    if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
        let _ = entry.delete_credential();
    }
}
