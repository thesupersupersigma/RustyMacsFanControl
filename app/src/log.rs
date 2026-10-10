//! Minimal leveled logger: writes to the log file when one is enabled and to stderr in
//! debug builds.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn set_file(file: File) {
    *FILE.lock().unwrap() = Some(file);
}

pub fn write(level: &str, message: std::fmt::Arguments) {
    let line = format!("[{level}] {message}");
    if cfg!(debug_assertions) {
        eprintln!("{line}");
    }
    if let Some(file) = FILE.lock().unwrap().as_mut() {
        let _ = writeln!(file, "{line}");
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log::write("info", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::log::write("warning", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log::write("error", format_args!($($arg)*)) };
}
