//! Measure accumulated time for various operations.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::Instant;

/// Accumulated times for different clocks.
///
/// The caller defines a clock type, usually an enum.
/// `AccumulatedTime` accumulates time for each possible clock.
/// Conceptually, every type of clock exists. If a type of clock
/// doesn't ever get created, it measures at 0 accumulated time.
#[derive(Debug)]
pub struct AccumulatedTime<T> {
    accumulated: Mutex<HashMap<T, ClockTime>>,
}

#[derive(Debug, Default)]
struct ClockTime {
    nanos: u128,
    started: Option<Instant>,
}

impl<T: Eq + PartialEq + Hash + Copy> AccumulatedTime<T> {
    /// Create a new accumulated time collector.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            accumulated: Mutex::new(HashMap::new()),
        }
    }

    /// Start a new clock of a given type to measure a span of time.
    ///
    /// The clock's measured time is added to the accumulator when the
    /// clock is stopped.
    pub fn start(&mut self, clock: T) {
        let mut map = self.accumulated.lock().unwrap();
        let ct = map.entry(clock).or_insert_with(ClockTime::default);
        assert!(ct.started.is_none());
        ct.started = Some(Instant::now());
    }

    /// Stop a running clock.
    ///
    /// Its run time is added to the accumulated time for that kind of clock.
    pub fn stop(&mut self, clock: T) {
        let mut map = self.accumulated.lock().unwrap();
        if let Some(mut ct) = map.get_mut(&clock) {
            assert!(ct.started.is_some());
            if let Some(started) = ct.started.take() {
                ct.nanos += started.elapsed().as_nanos();
                ct.started = None;
            }
        }
    }

    /// Return the accumulated time for a type of clock, as whole seconds.
    pub fn secs(&self, clock: T) -> u128 {
        self.nanos(clock) / 1_000_000_000u128
    }

    /// Return the accumulated time for a type of clock, as nanoseconds.
    ///
    /// This includes the time spent in a currently running clock.
    pub fn nanos(&self, clock: T) -> u128 {
        let map = self.accumulated.lock().unwrap();
        if let Some(ct) = map.get(&clock) {
            if let Some(started) = ct.started {
                ct.nanos + started.elapsed().as_nanos()
            } else {
                ct.nanos
            }
        } else {
            0
        }
    }
}
