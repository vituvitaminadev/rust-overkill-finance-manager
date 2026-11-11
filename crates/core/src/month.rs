use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

/// Um mês do calendário, escrito como `AAAA-MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Month {
    year: i32,
    month: u32,
}

impl Month {
    pub fn new(year: i32, month: u32) -> Option<Self> {
        (1..=12).contains(&month).then_some(Month { year, month })
    }

    pub fn of(date: NaiveDate) -> Self {
        Month {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn first_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("mês válido")
    }

    pub fn next(self) -> Self {
        self.plus(1)
    }

    /// O mês `months` à frente, ou atrás quando negativo.
    pub fn plus(self, months: i32) -> Self {
        let index = self.year * 12 + (self.month as i32 - 1) + months;
        Month {
            year: index.div_euclid(12),
            month: index.rem_euclid(12) as u32 + 1,
        }
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl FromStr for Month {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("mês inválido: {s}");
        let (year, month) = s.split_once('-').ok_or_else(invalid)?;
        let year = year.parse().map_err(|_| invalid())?;
        let month = month.parse().map_err(|_| invalid())?;
        Month::new(year, month).ok_or_else(invalid)
    }
}

impl TryFrom<String> for Month {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Month> for String {
    fn from(month: Month) -> String {
        month.to_string()
    }
}
