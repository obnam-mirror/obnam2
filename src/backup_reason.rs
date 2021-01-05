use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use std::fmt;

#[derive(Debug, Copy, Clone)]
pub enum Reason {
    Skipped,
    IsNew,
    Changed,
    Unchanged,
    Error,
}

impl Reason {
    pub fn from_str(text: &str) -> Reason {
        match text {
            "skipped" => Reason::Skipped,
            "new" => Reason::IsNew,
            "changed" => Reason::Changed,
            "unchanged" => Reason::Unchanged,
            _ => Reason::Error,
        }
    }
}

impl ToSql for Reason {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Text(format!(
            "{}",
            self
        ))))
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let reason = match self {
            Reason::Skipped => "skipped",
            Reason::IsNew => "new",
            Reason::Changed => "changed",
            Reason::Unchanged => "unchanged",
            Reason::Error => "error",
        };
        write!(f, "{}", reason)
    }
}
