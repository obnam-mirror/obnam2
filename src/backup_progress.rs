//! Progress bars for Obnam.

use crate::generation::GenId;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

/// A progress bar abstraction specific to backups.
///
/// The progress bar is different for initial and incremental backups,
/// and for different phases of making a backup.
pub struct BackupProgress {
    progress: ProgressBar,
}

impl BackupProgress {
    /// Create a progress bar for an initial backup.
    pub fn initial() -> Self {
        let progress = if true {
            ProgressBar::new(0)
        } else {
            ProgressBar::hidden()
        };
        let parts = vec![
            "initial backup",
            "elapsed: {elapsed}",
            "files: {pos}",
            "current: {wide_msg}",
            "{spinner}",
        ];
        progress.set_style(ProgressStyle::default_bar().template(&parts.join("\n")));
        progress.enable_steady_tick(100);

        Self { progress }
    }

    /// Create a progress bar for an incremental backup.
    pub fn incremental() -> Self {
        let progress = if true {
            ProgressBar::new(0)
        } else {
            ProgressBar::hidden()
        };
        let parts = vec![
            "incremental backup",
            "{wide_bar}",
            "elapsed: {elapsed}",
            "files: {pos}/{len}",
            "current: {wide_msg}",
            "{spinner}",
        ];
        progress.set_style(ProgressStyle::default_bar().template(&parts.join("\n")));
        progress.enable_steady_tick(100);

        Self { progress }
    }

    /// Create a progress bar for uploading a new generation's metadata.
    pub fn upload_generation() -> Self {
        let progress = ProgressBar::new(0);
        let parts = vec![
            "uploading new generation metadata",
            "elapsed: {elapsed}",
            "{spinner}",
        ];
        progress.set_style(ProgressStyle::default_bar().template(&parts.join("\n")));
        progress.enable_steady_tick(100);

        Self { progress }
    }

    /// Create a progress bar for downloading an existing generation's
    /// metadata.
    pub fn download_generation(gen_id: &GenId) -> Self {
        let progress = ProgressBar::new(0);
        let parts = vec!["{msg}", "elapsed: {elapsed}", "{spinner}"];
        progress.set_style(ProgressStyle::default_bar().template(&parts.join("\n")));
        progress.enable_steady_tick(100);
        progress.set_message(format!(
            "downloading previous generation metadata: {}",
            gen_id
        ));

        Self { progress }
    }

    /// Set the number of files that were in the previous generation.
    ///
    /// The new generation usually has about the same number of files,
    /// so the progress bar can show progress for incremental backups
    /// without having to count all the files that actually exist first.
    pub fn files_in_previous_generation(&self, count: u64) {
        self.progress.set_length(count);
    }

    /// Update progress bar about number of problems found during a backup.
    pub fn found_problem(&self) {
        self.progress.inc(1);
    }

    /// Update progress bar about number of actual files found.
    pub fn found_live_file(&self, filename: &Path) {
        self.progress.inc(1);
        if self.progress.length() < self.progress.position() {
            self.progress.set_length(self.progress.position());
        }
        self.progress.set_message(format!("{}", filename.display()));
    }

    /// Tell progress bar it's finished.
    ///
    /// This will remove all traces of the progress bar from the
    /// screen.
    pub fn finish(&self) {
        self.progress.set_length(self.progress.position());
        self.progress.finish_and_clear();
    }
}
