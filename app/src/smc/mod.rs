pub mod platform;
pub mod service;

#[cfg(any(test, debug_assertions))]
pub mod fake;
