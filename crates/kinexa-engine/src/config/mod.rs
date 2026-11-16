//! Configuration: YAML schema, loading and validation with path-qualified errors (docs/04 §6).

mod load;
mod schema;
mod validate;

pub use load::{ConfigError, load, parse};
pub use schema::*;
pub use validate::{Problem, Validated};
