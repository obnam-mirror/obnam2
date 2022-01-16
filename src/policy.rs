//! Policy for what gets backed up.

use crate::backup_reason::Reason;
use crate::fsentry::FilesystemEntry;
use crate::generation::LocalGeneration;
use log::{debug, warn};

/// Policy for what gets backed up.
///
/// The policy allows two aspects to be controlled:
///
/// * should new files )(files that didn't exist in the previous
///   backup be included in the new backup?
/// * should files that haven't been changed since the previous backup
///   be included in the new backup?
///
/// If policy doesn't allow a file to be included, it's skipped.
pub struct BackupPolicy {
    new: bool,
    old_if_changed: bool,
}

impl BackupPolicy {
    /// Create a default policy.
    pub fn default() -> Self {
        Self {
            new: true,
            old_if_changed: true,
        }
    }

    /// Does a given file need to be backed up?
    pub fn needs_backup(&self, old: &LocalGeneration, new_entry: &FilesystemEntry) -> Reason {
        let new_name = new_entry.pathbuf();
        let reason = match old.get_file(&new_name) {
            Ok(None) => {
                if self.new {
                    Reason::IsNew
                } else {
                    Reason::Skipped
                }
            }
            Ok(Some(old_entry)) => {
                if self.old_if_changed {
                    if file_has_changed(&old_entry, new_entry) {
                        Reason::Changed
                    } else {
                        Reason::Unchanged
                    }
                } else {
                    Reason::Skipped
                }
            }
            Err(err) => {
                warn!(
                    "needs_backup: lookup in old generation returned error, ignored: {:?}: {}",
                    new_name, err
                );
                Reason::GenerationLookupError
            }
        };
        debug!(
            "needs_backup: file {:?}: policy decision: {}",
            new_name, reason
        );
        reason
    }
}

fn file_has_changed(old: &FilesystemEntry, new: &FilesystemEntry) -> bool {
    let unchanged = old.kind() == new.kind()
        && old.len() == new.len()
        && old.mode() == new.mode()
        && old.mtime() == new.mtime()
        && old.mtime_ns() == new.mtime_ns()
        && old.symlink_target() == new.symlink_target();
    !unchanged
}
