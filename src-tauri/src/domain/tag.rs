//! Tag system.

use serde::{Deserialize, Serialize};

use super::ids::TagId;

/// Tag category grouping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagCategory {
    Genre,
    Theme,
    Setting,
    Technical,
    ContentWarning,
    UserDefined,
}

/// A tag that can be applied to works.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    /// Normalized key for deduplication (lowercase, no spaces)
    pub key: String,
    /// Display label
    pub label: String,
    pub category: TagCategory,
    /// How many works have this tag
    pub usage_count: u32,
    /// VNDB tag ID if sourced from VNDB
    pub vndb_tag_id: Option<String>,
}

/// A tag applied to a specific work with an optional score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTag {
    pub tag_id: TagId,
    pub score: Option<f64>,
    pub spoiler_level: u8,
}
