//! Database schema versions.

use serde::Serialize;

/// The type of schema version components.
pub type VersionComponent = u32;

/// Schema version of the database storing the generation.
///
/// An Obnam client can restore a generation using schema version
/// (x,y), if the client supports a schema version (x,z). If z < y,
/// the client knows it may not be able to the generation faithfully,
/// and should warn the user about this. If z >= y, the client knows
/// it can restore the generation faithfully. If the client does not
/// support any schema version x, it knows it can't restore the backup
/// at all.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SchemaVersion {
    /// Major version.
    pub major: VersionComponent,
    /// Minor version.
    pub minor: VersionComponent,
}

impl SchemaVersion {
    /// Create a new schema version object.
    pub fn new(major: VersionComponent, minor: VersionComponent) -> Self {
        Self { major, minor }
    }

    /// Return the major and minor version number component of a schema version.
    pub fn version(&self) -> (VersionComponent, VersionComponent) {
        (self.major, self.minor)
    }

    /// Is this schema version compatible with another schema version?
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl std::str::FromStr for SchemaVersion {
    type Err = SchemaVersionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(pos) = s.find('.') {
            let major = parse_int(&s[..pos])?;
            let minor = parse_int(&s[pos + 1..])?;
            Ok(SchemaVersion::new(major, minor))
        } else {
            Err(SchemaVersionError::Invalid(s.to_string()))
        }
    }
}

fn parse_int(s: &str) -> Result<VersionComponent, SchemaVersionError> {
    if let Ok(i) = s.parse() {
        Ok(i)
    } else {
        Err(SchemaVersionError::InvalidComponent(s.to_string()))
    }
}

/// Possible errors from parsing schema versions.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SchemaVersionError {
    /// Failed to parse a string as a schema version.
    #[error("Invalid schema version {0:?}")]
    Invalid(String),

    /// Failed to parse a string as a schema version component.
    #[error("Invalid schema version component {0:?}")]
    InvalidComponent(String),
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn from_string() {
        let v = SchemaVersion::from_str("1.2").unwrap();
        assert_eq!(v.version(), (1, 2));
    }

    #[test]
    fn from_string_fails_if_empty() {
        match SchemaVersion::from_str("") {
            Err(SchemaVersionError::Invalid(s)) => assert_eq!(s, ""),
            _ => unreachable!(),
        }
    }

    #[test]
    fn from_string_fails_if_empty_major() {
        match SchemaVersion::from_str(".2") {
            Err(SchemaVersionError::InvalidComponent(s)) => assert_eq!(s, ""),
            _ => unreachable!(),
        }
    }

    #[test]
    fn from_string_fails_if_empty_minor() {
        match SchemaVersion::from_str("1.") {
            Err(SchemaVersionError::InvalidComponent(s)) => assert_eq!(s, ""),
            _ => unreachable!(),
        }
    }

    #[test]
    fn from_string_fails_if_just_major() {
        match SchemaVersion::from_str("1") {
            Err(SchemaVersionError::Invalid(s)) => assert_eq!(s, "1"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn from_string_fails_if_nonnumeric_major() {
        match SchemaVersion::from_str("a.2") {
            Err(SchemaVersionError::InvalidComponent(s)) => assert_eq!(s, "a"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn from_string_fails_if_nonnumeric_minor() {
        match SchemaVersion::from_str("1.a") {
            Err(SchemaVersionError::InvalidComponent(s)) => assert_eq!(s, "a"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn compatible_with_self() {
        let v = SchemaVersion::new(1, 2);
        assert!(v.is_compatible_with(&v));
    }

    #[test]
    fn compatible_with_older_minor_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(1, 3);
        assert!(new.is_compatible_with(&old));
    }

    #[test]
    fn not_compatible_with_newer_minor_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(1, 3);
        assert!(!old.is_compatible_with(&new));
    }

    #[test]
    fn not_compatible_with_older_major_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(2, 0);
        assert!(!new.is_compatible_with(&old));
    }

    #[test]
    fn not_compatible_with_newer_major_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(2, 0);
        assert!(!old.is_compatible_with(&new));
    }
}
