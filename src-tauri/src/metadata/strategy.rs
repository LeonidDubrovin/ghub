use crate::models::MetadataSearchResult;
use reqwest::Client;
use async_trait::async_trait;

/// Strategy trait for metadata sources
/// Implement this trait to add support for a new metadata source
#[async_trait]
pub trait MetadataStrategy: Send + Sync {
    /// Name of the metadata source (e.g., "steam", "itch", "igdb")
    fn name(&self) -> &str;
    
    /// Whether this strategy is enabled
    fn is_enabled(&self) -> bool;
    
    /// Search for games by query string
    async fn search(&self, client: &Client, query: &str) -> Result<Vec<MetadataSearchResult>, String>;
    
    /// Get detailed metadata for a specific game by ID
    async fn get_details(&self, client: &Client, id: &str) -> Result<Option<MetadataSearchResult>, String>;
}