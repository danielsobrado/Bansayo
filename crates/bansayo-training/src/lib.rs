//! Pure training library: no runtime, I/O or clock (ADR-018).
pub mod coaching;
pub mod content;
pub mod evidence;
pub mod exercise;
pub mod hash;
pub mod metrics;
pub mod observables;
pub mod progress;
pub mod recognize;
pub mod scoring;
pub mod segment;
pub mod session;
#[cfg(test)]
pub(crate) mod testkit;
