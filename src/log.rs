//! Lightweight, dependency-free observability: a leveled logger plus a search
//! telemetry struct.
//!
//! The log level is read once from the `RUSTCHESS_LOG` environment variable
//! (`off`, `error`, `warn`, `info`, `debug`, `trace`) and cached in an atomic so
//! logging has near-zero cost when disabled. Messages go to stderr so they
//! never corrupt UCI's stdout protocol.
//!
//! Use the [`log_error!`], [`log_warn!`], [`log_info!`], [`log_debug!`], and
//! [`log_trace!`] macros.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

/// Severity levels, ordered from most to least severe.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl Level {
    fn from_str(s: &str) -> Level {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "0" => Level::Off,
            "error" => Level::Error,
            "warn" | "warning" => Level::Warn,
            "info" => Level::Info,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Level::Off => "OFF",
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

/// 0 means "not yet initialized"; real levels are stored as `level as u8 + 1`.
static LEVEL: AtomicU8 = AtomicU8::new(0);

/// The active log level, initializing from the environment on first use.
pub fn level() -> Level {
    let raw = LEVEL.load(Ordering::Relaxed);
    if raw == 0 {
        let lvl = match std::env::var("RUSTCHESS_LOG") {
            Ok(v) => Level::from_str(&v),
            Err(_) => Level::Off,
        };
        LEVEL.store(lvl as u8 + 1, Ordering::Relaxed);
        return lvl;
    }
    match raw - 1 {
        1 => Level::Error,
        2 => Level::Warn,
        3 => Level::Info,
        4 => Level::Debug,
        5 => Level::Trace,
        _ => Level::Off,
    }
}

/// Override the log level programmatically (e.g. from a CLI flag or a test).
pub fn set_level(lvl: Level) {
    LEVEL.store(lvl as u8 + 1, Ordering::Relaxed);
}

/// `true` if a message at `lvl` would currently be emitted.
#[inline]
pub fn enabled(lvl: Level) -> bool {
    lvl <= level()
}

/// Emit a log line. Prefer the macros, which skip formatting when disabled.
pub fn log(lvl: Level, args: std::fmt::Arguments) {
    if enabled(lvl) {
        eprintln!("[{}] {}", lvl.label(), args);
    }
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Error, format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Warn, format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Info, format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Debug, format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Trace, format_args!($($arg)*)) };
}

/// Aggregated counters for one search, useful for benchmarking and tuning.
#[derive(Clone, Copy, Debug, Default)]
pub struct SearchTelemetry {
    pub nodes: u64,
    pub qnodes: u64,
    pub tt_hits: u64,
    pub tt_probes: u64,
    pub beta_cutoffs: u64,
    /// Of the beta cutoffs, how many happened on the first move searched,
    /// a measure of move-ordering quality (higher is better).
    pub first_move_cutoffs: u64,
    pub elapsed_ms: u128,
}

impl SearchTelemetry {
    /// Nodes searched per second.
    pub fn nps(&self) -> u64 {
        (self.nodes as u128 * 1000)
            .checked_div(self.elapsed_ms)
            .unwrap_or(0) as u64
    }

    /// Transposition-table hit rate in `0.0..=1.0`.
    pub fn tt_hit_rate(&self) -> f64 {
        if self.tt_probes == 0 {
            0.0
        } else {
            self.tt_hits as f64 / self.tt_probes as f64
        }
    }

    /// Fraction of cutoffs occurring on the first move (move-ordering quality).
    pub fn move_ordering_quality(&self) -> f64 {
        if self.beta_cutoffs == 0 {
            0.0
        } else {
            self.first_move_cutoffs as f64 / self.beta_cutoffs as f64
        }
    }

    /// A one-line human summary.
    pub fn summary(&self) -> String {
        format!(
            "nodes={} ({} q) nps={} tt_hit={:.1}% ordering={:.1}%",
            self.nodes,
            self.qnodes,
            self.nps(),
            self.tt_hit_rate() * 100.0,
            self.move_ordering_quality() * 100.0,
        )
    }
}

/// A scoped stopwatch that logs the elapsed time at `Debug` level when dropped.
pub struct Timer {
    label: &'static str,
    start: Instant,
}

impl Timer {
    pub fn new(label: &'static str) -> Self {
        Timer {
            label,
            start: Instant::now(),
        }
    }

    /// Elapsed milliseconds so far.
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        log(
            Level::Debug,
            format_args!("{} took {}ms", self.label, self.elapsed_ms()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ordering() {
        assert!(Level::Error < Level::Info);
        assert!(Level::Info < Level::Trace);
    }

    #[test]
    fn set_and_query_level() {
        set_level(Level::Debug);
        assert!(enabled(Level::Error));
        assert!(enabled(Level::Debug));
        assert!(!enabled(Level::Trace));
        set_level(Level::Off);
        assert!(!enabled(Level::Error));
    }

    #[test]
    fn telemetry_math() {
        let t = SearchTelemetry {
            nodes: 2000,
            tt_hits: 50,
            tt_probes: 100,
            beta_cutoffs: 10,
            first_move_cutoffs: 9,
            elapsed_ms: 1000,
            ..Default::default()
        };
        assert_eq!(t.nps(), 2000);
        assert!((t.tt_hit_rate() - 0.5).abs() < 1e-9);
        assert!((t.move_ordering_quality() - 0.9).abs() < 1e-9);
        assert!(t.summary().contains("nodes=2000"));
    }
}
