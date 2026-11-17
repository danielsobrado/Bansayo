//! Content index and validation problems (13 §11, 14 §5.1).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A path-qualified validation problem, e.g. `phases[2].enter.all[0].obs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ContentProblem {
    pub path: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    Shipped,
    User,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ContentNames {
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// One loaded definition, valid or not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ContentIndexEntry {
    /// `disciplines`, `techniques`, `metrics`, `scoring`, …
    pub kind: String,
    pub id: String,
    pub version: u32,
    pub status: String,
    pub names: ContentNames,
    pub valid: bool,
    pub problems: Vec<ContentProblem>,
    pub source: ContentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub discipline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub family: Option<String>,
    /// Repository-relative or user-directory-relative file. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ContentIndex {
    pub content_rev: u64,
    pub entries: Vec<ContentIndexEntry>,
}
