//! Data contracts shared by the Bansayo engine and its clients.
//!
//! Every type that crosses a process boundary or is persisted lives here: identifiers, frames,
//! observations, landmarks, skeleton, pose, calibration, state, stream messages and HTTP bodies.
//! Types derive `serde` for JSON and `ts-rs` for the generated TypeScript in `contracts/ts/`.
//!
//! Conventions: right-handed, +Y up, +Z forward, metres, radians, quaternions `[x, y, z, w]`,
//! time in integer microseconds on the session clock.

#![forbid(unsafe_code)]

pub mod api;
pub mod athlete;
pub mod calibration;
pub mod capability;
pub mod content;
pub mod device;
pub mod frames;
pub mod ids;
pub mod landmarks;
pub mod motion;
pub mod observation;
pub mod pose;
pub mod progress;
pub mod quality;
pub mod session;
pub mod skeleton;
pub mod state;
pub mod time;
pub mod training;
pub mod wire;
pub mod xr;

pub use api::*;
pub use athlete::*;
pub use calibration::*;
pub use capability::*;
pub use content::*;
pub use device::*;
pub use frames::*;
pub use ids::*;
pub use landmarks::*;
pub use motion::*;
pub use observation::*;
pub use pose::*;
pub use progress::*;
pub use quality::*;
pub use session::*;
pub use skeleton::*;
pub use state::*;
pub use time::*;
pub use training::*;
pub use wire::*;
pub use xr::*;
