#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSearchResult;
    use crate::metadata::{SteamStrategy, ItchStrategy, MetadataAggregator, MetadataStrategy};
    use std::sync::Arc;
    
    #[test]
    fn test_steam_strategy_name() {
        let strategy = SteamStrategy::new();
        assert_eq!(strategy.name(), "steam");
    }
    
    #[test]
    fn test_steam_strategy_enabled() {
        let strategy = SteamStrategy::new();
        assert!(strategy.is_enabled());
        
        let strategy_disabled = SteamStrategy::with_enabled(false);
        assert!(!strategy_disabled.is_enabled());
    }
    
    #[test]
    fn test_itch_strategy_name() {
        let strategy = ItchStrategy::new();
        assert_eq!(strategy.name(), "itch");
    }
    
    #[test]
    fn test_itch_strategy_enabled() {
        let strategy = ItchStrategy::new();
        assert!(strategy.is_enabled());
        
        let strategy_disabled = ItchStrategy::with_enabled(false);
        assert!(!strategy_disabled.is_enabled());
    }
    
    #[test]
    fn test_aggregator_new() {
        let aggregator = MetadataAggregator::new();
        let sources = aggregator.available_sources();
        assert!(sources.contains(&"steam"));
        assert!(sources.contains(&"itch"));
    }
    
    #[test]
    fn test_aggregator_enabled_sources() {
        let aggregator = MetadataAggregator::new();
        let enabled = aggregator.enabled_sources();
        assert!(enabled.contains(&"steam"));
        assert!(enabled.contains(&"itch"));
    }
    
    #[test]
    fn test_aggregator_with_custom_strategies() {
        let strategies: Vec<Arc<dyn MetadataStrategy>> = vec![
            Arc::new(SteamStrategy::with_enabled(true)),
            Arc::new(ItchStrategy::with_enabled(false)),
        ];
        
        let aggregator = MetadataAggregator::with_strategies(strategies);
        let enabled = aggregator.enabled_sources();
        assert!(enabled.contains(&"steam"));
        assert!(!enabled.contains(&"itch"));
    }
    
    #[test]
    fn test_metadata_search_result_creation() {
        let result = MetadataSearchResult {
            id: "123".to_string(),
            name: "Test Game".to_string(),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            release_date: Some("2024-01-01".to_string()),
            developer: Some("Test Developer".to_string()),
            publisher: Some("Test Publisher".to_string()),
            description: Some("Test description".to_string()),
            rating: Some(4.5),
            source: "steam".to_string(),
            url: Some("https://store.steampowered.com/app/123".to_string()),
            tags: Some(vec!["Action".to_string(), "Adventure".to_string()]),
            genres: Some(vec!["RPG".to_string()]),
        };
        
        assert_eq!(result.id, "123");
        assert_eq!(result.name, "Test Game");
        assert_eq!(result.source, "steam");
        assert!(result.cover_url.is_some());
        assert!(result.tags.is_some());
    }
}