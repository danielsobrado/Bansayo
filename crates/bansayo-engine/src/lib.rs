//! Local tracking engine. API consumers communicate with the pipeline actor through commands.
mod api;
mod app;
pub mod calibration;
pub mod clock;
pub mod config;
pub mod content_store;
pub mod math;
pub mod pipeline;
pub mod recording;
pub mod registry;
pub mod regress;
pub mod skeleton;
pub mod sources;
pub mod store;
mod ticker;
pub mod training;
pub use app::{EngineHandle, run, start};
