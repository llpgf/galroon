//! Character entity.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::ids::CharacterId;

/// Gender of a character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
    Other,
    Unknown,
}

/// Character role in a work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterRole {
    Main,
    Primary,
    Side,
    Appears,
}

/// A trait attached to a character (e.g., "Tsundere", "Blue eyes").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterTrait {
    pub name: String,
    pub group: Option<String>,
    pub spoiler_level: u8,
}

/// A character from a visual novel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: CharacterId,
    pub vndb_id: Option<String>,
    pub name: String,
    pub name_original: Option<String>,
    pub gender: Gender,
    pub birthday: Option<NaiveDate>,
    pub bust: Option<String>,
    pub height: Option<u16>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub role: CharacterRole,
    pub voice_actor: Option<String>,
    pub traits: Vec<CharacterTrait>,
}
