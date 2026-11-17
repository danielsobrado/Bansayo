//! Pure training library: no runtime, I/O or clock (ADR-018).
pub mod content;
pub mod evidence;
pub mod hash;
pub mod observables;
pub mod recognize;
pub mod segment;
#[cfg(test)]
pub(crate) mod testkit;
