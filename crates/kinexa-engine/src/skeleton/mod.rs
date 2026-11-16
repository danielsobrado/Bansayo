//! The skeleton: built-in `kinexa.body.v1` definition, proportion scaling, forward kinematics.
//! Pure.

mod definition;
mod fk;
pub mod proportions;

pub use definition::*;
pub use fk::*;
