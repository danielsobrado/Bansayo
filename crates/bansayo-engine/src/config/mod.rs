//! Configuration: YAML schema, loading and validation with path-qualified errors (docs/04 §6).

mod load;
mod schema;
mod validate;

pub use load::{ConfigError, load, load_with_data_dir, parse};
pub use schema::*;
pub use validate::{
    CameraSpec, Problem, SimulatedSpec, TrackerSpec, Validated, triangulation_valid,
};
