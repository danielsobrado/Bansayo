//! Reading and parsing the configuration file.

use std::fmt;
use std::path::{Path, PathBuf};

use super::schema::Config;
use super::validate::{Problem, Validated, validate};

/// Why a configuration could not be used. `Display` lists every problem, one per line.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Parse(String),
    #[error("{}", Lines(.0))]
    Invalid(Vec<Problem>),
}

struct Lines<'a>(&'a [Problem]);

impl fmt::Display for Lines<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, p) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{p}")?;
        }
        Ok(())
    }
}

/// Reads, parses and validates the configuration at `path`.
pub fn load(path: impl AsRef<Path>) -> Result<Validated, ConfigError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })?;
    parse(&text).map_err(|e| match e {
        ConfigError::Parse(msg) => ConfigError::Parse(format!("{}: {msg}", path.display())),
        other => other,
    })
}

/// Parses and validates configuration text.
pub fn parse(text: &str) -> Result<Validated, ConfigError> {
    let config: Config =
        serde_saphyr::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))?;
    validate(config).map_err(ConfigError::Invalid)
}

/// Loads a bundled configuration into a host-selected data directory before filesystem validation.
/// Explicit user configuration should use `load` to retain its configured data path.
pub fn load_with_data_dir(
    path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
) -> Result<Validated, ConfigError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut config: Config = serde_saphyr::from_str(&text)
        .map_err(|e| ConfigError::Parse(format!("{}: {e}", path.display())))?;
    config.engine.data_dir = data_dir.as_ref().to_owned();
    validate(config).map_err(ConfigError::Invalid)
}
