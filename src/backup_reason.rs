//! Why was a file backed up?

use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use std::fmt;

/// Represent the reason a file is in a backup.
#[derive(Debug, Copy, Clone)]
pub enum Reason {
    /// File was skipped due to policy, but carried over without
    /// changes.
    Skipped,
    /// File is new, compared to previous backup.
    IsNew,
    /// File has been changed, compared to previous backup,
    Changed,
    /// File has not been changed, compared to previous backup,
    Unchanged,
    /// There was an error looking up the file in the previous backup.
    ///
    /// File has been carried over without changes.
    GenerationLookupError,
    /// The was an error backing up the file.
    ///
    /// File has been carried over without changes.
    FileError,
    /// Reason is unknown.
    ///
    /// The previous backup had a reason that the current version of
    /// Obnam doesn't recognize. The file has been carried over
    /// without changes.
    Unknown,
}

impl Reason {
    /// Create a Reason from a string representation.
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
    /// Represent Reason as text for SQL.
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Text(format!(
            "{}",
            self
        ))))
    }
}

impl fmt::Display for Reason {
    /// Represent Reason for display.
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
