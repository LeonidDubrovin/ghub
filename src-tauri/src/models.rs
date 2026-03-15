use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    #[serde(rename = "type")]
    pub space_type: String, // local, steam, itch, virtual
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSource {
    pub space_id: String,
    pub source_path: String,
    pub is_active: bool,
    pub scan_recursively: bool,
    pub last_scanned_at: Option<String>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub sort_title: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub cover_image: Option<String>,
    pub background_image: Option<String>,
    pub total_playtime_seconds: i64,
    pub last_played_at: Option<String>,
    pub times_launched: i32,
    pub is_favorite: bool,
    pub is_hidden: bool,
    pub completion_status: String, // not_played, playing, completed, abandoned, on_hold
    pub user_rating: Option<i32>,
    pub added_at: String,
    pub updated_at: String,
    pub external_link: Option<String>,
    // Optional fields for UI display (populated when joining with installs/spaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    pub id: String,
    pub game_id: String,
    pub space_id: String,
    pub install_path: String,
    pub executable_path: Option<String>,
    pub launch_arguments: Option<String>,
    pub working_directory: Option<String>,
    pub status: String, // installed, installing, broken
    pub version: Option<String>,
    pub install_size_bytes: Option<i64>,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedGame {
    pub path: String,
    pub title: String,
    pub executable: Option<String>,
    pub all_executables: Vec<String>,       // All found exe files
    pub size_bytes: u64,
    pub icon_path: Option<String>,          // Local icon if found
    pub cover_candidates: Vec<String>,       // Found image files that could be covers
    pub exe_metadata: Option<ExeMetadata>,   // Extracted from exe
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExeMetadata {
    pub product_name: Option<String>,
    pub company_name: Option<String>,
    pub file_description: Option<String>,
    pub file_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct CreateSpaceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub space_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub initial_sources: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    pub title: String,
    pub space_id: String,
    pub install_path: String,
    pub executable_path: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub cover_image: Option<String>,
    pub fetch_metadata: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGameLinkRequest {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateGameRequest {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub cover_image: Option<String>,
    pub is_favorite: Option<bool>,
    pub completion_status: Option<String>,
    pub user_rating: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadLink {
    pub id: String,
    pub url: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSearchResult {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub rating: Option<f32>,
    pub source: String,
    pub url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub genres: Option<Vec<String>>,
}