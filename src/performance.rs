//! Performance measurements from an Obnam run.

use crate::accumulated_time::AccumulatedTime;
use log::info;

/// The kinds of clocks we have.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Clock {
    /// The complete runtime of the program.
    RunTime,

    /// Time spent downloading previous backup generations.
    GenerationDownload,

    /// Time spent uploading backup generations.
    GenerationUpload,

    /// Time spent checking if a chunk exists already on server.
    HasChunk,

    /// Time spent computing splitting files into chunks.
    Chunking,

    /// Time spent scanning live data.
    Scanning,
}

/// Collected measurements from this Obnam run.
#[derive(Debug)]
pub struct Performance {
    args: Vec<String>,
    time: AccumulatedTime<Clock>,
    live_files: u64,
    files_backed_up: u64,
    chunks_uploaded: u64,
    chunks_reused: u64,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            args: std::env::args().collect(),
            time: AccumulatedTime::<Clock>::new(),
            live_files: 0,
            files_backed_up: 0,
            chunks_reused: 0,
            chunks_uploaded: 0,
        }
    }
}

impl Performance {
    /// Log all performance measurements to the log file.
    pub fn log(&self) {
        info!("Performance measurements for this Obnam run");
        for (i, arg) in self.args.iter().enumerate() {
            info!("argv[{}]={:?}", i, arg);
        }
        info!("Live files found: {}", self.live_files);
        info!("Files backed up: {}", self.files_backed_up);
        info!("Chunks uploaded: {}", self.chunks_uploaded);
        info!("Chunks reused: {}", self.chunks_reused);
        info!(
            "Scanning live data (seconds): {}",
            self.time.secs(Clock::Scanning)
        );
        info!(
            "Chunking live data (seconds): {}",
            self.time.secs(Clock::Chunking)
        );
        info!(
            "Checking for duplicate chunks (seconds): {}",
            self.time.secs(Clock::HasChunk)
        );
        info!(
            "Downloading previous generation (seconds): {}",
            self.time.secs(Clock::GenerationDownload)
        );
        info!(
            "Uploading new generation (seconds): {}",
            self.time.secs(Clock::GenerationUpload)
        );
        info!(
            "Complete run time (seconds): {}",
            self.time.secs(Clock::RunTime)
        );
    }

    /// Start a specific clock.
    pub fn start(&mut self, clock: Clock) {
        self.time.start(clock)
    }

    /// Stop a specific clock.
    pub fn stop(&mut self, clock: Clock) {
        self.time.stop(clock)
    }

    /// Increment number of live files.
    pub fn found_live_files(&mut self, n: u64) {
        self.live_files += n;
    }

    /// Increment number of files backed up this run.
    pub fn back_up_file(&mut self) {
        self.files_backed_up += 1;
    }

    /// Increment number of reused chunks.
    pub fn reuse_chunk(&mut self) {
        self.chunks_reused += 1;
    }

    /// Increment number of uploaded chunks.
    pub fn upload_chunk(&mut self) {
        self.chunks_uploaded += 1;
    }
}
