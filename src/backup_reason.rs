use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use std::fmt;

#[derive(Debug, Copy, Clone)]
pub enum Reason {
    Skipped,
    IsNew,
    Changed,
    Unchanged,
    GenerationLookupError,
    FileError,
    Unknown,
}

impl Reason {
    pub fn from(text: &str) -> Reason {
        match text {
            "skipped" => Reason::Skipped,
            "new" => Reason::IsNew,
            "changed" => Reason::Changed,
            "unchanged" => Reason::Unchanged,
            "genlookuperror" => Reason::GenerationLookupError,
            "fileerror" => Reason::FileError,
            _ => Reason::Unknown,
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
            Reason::GenerationLookupError => "genlookuperror",
            Reason::FileError => "fileerror",
            Reason::Unknown => "unknown",
        };
        write!(f, "{}", reason)
    }
}
