//! Logging utilities for the terminal core
//!
//! Provides platform-aware logging that integrates with Android's Logcat.

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

/// Logger tag for Android Logcat
const LOG_TAG: &str = "TerminalCore";

/// Simple logger implementation for release builds
struct TerminalLogger {
    level: LevelFilter,
}

impl Log for TerminalLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // In release builds, this will be captured by Android Logcat
            // via the android_logger crate (added when we do full Android integration)
            let level_char = match record.level() {
                Level::Error => 'E',
                Level::Warn => 'W',
                Level::Info => 'I',
                Level::Debug => 'D',
                Level::Trace => 'T',
            };

            // For now, print to stderr which Android captures
            eprintln!(
                "[{}] {} {}: {}",
                level_char,
                LOG_TAG,
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: TerminalLogger = TerminalLogger {
    level: LevelFilter::Debug,
};

/// Initialize the logger
/// Call this once at library load time
pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)?;

    #[cfg(debug_assertions)]
    log::set_max_level(LevelFilter::Debug);

    #[cfg(not(debug_assertions))]
    log::set_max_level(LevelFilter::Info);

    Ok(())
}

/// Initialize logger, ignoring if already initialized
pub fn init_or_ignore() {
    let _ = init();
}
