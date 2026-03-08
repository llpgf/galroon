//! Asset type — classifies files within a game folder.

use serde::{Deserialize, Serialize};

/// Type of asset file within a game folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    /// Main game archive or extracted game folder
    Game,
    /// Crack, patch, noDVD fix
    Crack,
    /// Original soundtrack, music files
    Ost,
    /// Voice drama / bonus voice data
    VoiceDrama,
    /// Save data
    Save,
    /// Walkthrough, guide, strategy
    Guide,
    /// Bonus materials: wallpapers, artbooks, tokuten, etc.
    Bonus,
    /// DLC / append disc / extra scenario
    Dlc,
    /// Update patch / hotfix
    Update,
    /// Unclassified — user needs to tag manually
    Unknown,
}

impl AssetType {
    /// Emoji icon for display.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Game => "🎮",
            Self::Crack => "🔓",
            Self::Ost => "🎵",
            Self::VoiceDrama => "🎙️",
            Self::Save => "💾",
            Self::Guide => "📖",
            Self::Bonus => "🎁",
            Self::Dlc => "📦",
            Self::Update => "🩹",
            Self::Unknown => "❓",
        }
    }
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Game => "Game",
            Self::Crack => "Crack",
            Self::Ost => "OST",
            Self::VoiceDrama => "Voice Drama",
            Self::Save => "Save",
            Self::Guide => "Guide",
            Self::Bonus => "Bonus",
            Self::Dlc => "DLC",
            Self::Update => "Update",
            Self::Unknown => "Unknown",
        };
        write!(f, "{}", label)
    }
}

/// A classified asset entry within a game folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Absolute path to the file or subfolder
    pub path: std::path::PathBuf,
    /// Filename (for display)
    pub filename: String,
    /// Detected asset type
    pub asset_type: AssetType,
    /// File size in bytes (0 for directories)
    pub size_bytes: u64,
    /// Is this a directory?
    pub is_dir: bool,
}
