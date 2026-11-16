//! Data contracts shared by the Kinexa engine and its clients.
//!
//! Every type that crosses a process boundary or is persisted lives here: identifiers, frames,
//! observations, landmarks, skeleton, pose, calibration, state, stream messages and HTTP bodies.
//! Types derive `serde` for JSON and `ts-rs` for the generated TypeScript in `contracts/ts/`.
//!
//! Conventions: right-handed, +Y up, +Z forward, metres, radians, quaternions `[x, y, z, w]`,
//! time in integer microseconds on the session clock.

#![forbid(unsafe_code)]

pub mod api;
pub mod calibration;
pub mod device;
pub mod frames;
pub mod ids;
pub mod landmarks;
pub mod observation;
pub mod pose;
pub mod quality;
pub mod session;
pub mod skeleton;
pub mod state;
pub mod time;
pub mod wire;

pub use api::*;
pub use calibration::*;
pub use device::*;
pub use frames::*;
pub use ids::*;
pub use landmarks::*;
pub use observation::*;
pub use pose::*;
pub use quality::*;
pub use session::*;
pub use skeleton::*;
pub use state::*;
pub use time::*;
pub use wire::*;
