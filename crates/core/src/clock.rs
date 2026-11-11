use chrono::{DateTime, Utc};

/// Fonte da data e hora atuais, injetada para que os testes controlem o tempo.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
