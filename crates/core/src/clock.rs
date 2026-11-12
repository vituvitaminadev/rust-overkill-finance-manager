use chrono::{DateTime, Local, NaiveDate, Utc};

/// Fonte da data e hora atuais, injetada para que os testes controlem o tempo.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;

    /// O dia de hoje no calendário de quem usa o app, que decide o que está atrasado.
    fn today(&self) -> NaiveDate;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn today(&self) -> NaiveDate {
        Local::now().date_naive()
    }
}
