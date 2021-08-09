use crate::generation::GenId;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

pub struct BackupProgress {
    progress: ProgressBar,
}

impl BackupProgress {
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

    pub fn files_in_previous_generation(&self, count: u64) {
        self.progress.set_length(count);
    }

    pub fn found_problem(&self) {
        self.progress.inc(1);
    }

    pub fn found_problems(&self, n: u64) {
        self.progress.inc(n);
    }

    pub fn found_live_file(&self, filename: &Path) {
        self.progress.inc(1);
        if self.progress.length() < self.progress.position() {
            self.progress.set_length(self.progress.position());
        }
        self.progress.set_message(format!("{}", filename.display()));
    }

    pub fn finish(&self) {
        self.progress.set_length(self.progress.position());
        self.progress.finish_and_clear();
    }
}
