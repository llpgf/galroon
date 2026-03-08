//! Person entity — voice actors, artists, writers, composers.

use serde::{Deserialize, Serialize};

use super::ids::PersonId;

/// The professional role a person plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonRole {
    VoiceActor,
    Artist,
    Writer,
    Composer,
    Director,
    Other(String),
}

/// A person who contributed to one or more works.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    pub name_original: Option<String>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub roles: Vec<PersonRole>,
    pub image_url: Option<String>,
    pub description: Option<String>,
}

/// Credit linking a Person to a Work with a specific role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkCredit {
    pub person_id: PersonId,
    pub role: PersonRole,
    pub character_name: Option<String>,
    pub notes: Option<String>,
}
