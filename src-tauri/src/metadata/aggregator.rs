use crate::models::MetadataSearchResult;
use crate::metadata::strategy::MetadataStrategy;
use crate::metadata::{SteamStrategy, ItchStrategy};
use reqwest::Client;
use std::sync::Arc;

/// Aggregator for metadata strategies
/// Manages multiple metadata sources and provides unified search interface
pub struct MetadataAggregator {
    strategies: Vec<Arc<dyn MetadataStrategy>>,
}

impl MetadataAggregator {
    /// Create a new aggregator with default strategies (Steam and Itch)
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Arc::new(SteamStrategy::new()),
                Arc::new(ItchStrategy::new()),
            ],
        }
    }
    
    /// Create aggregator with custom strategies
    pub fn with_strategies(strategies: Vec<Arc<dyn MetadataStrategy>>) -> Self {
        Self { strategies }
    }
    
    /// Add a strategy to the aggregator
    pub fn add_strategy(&mut self, strategy: Arc<dyn MetadataStrategy>) {
        self.strategies.push(strategy);
    }
    
    /// Get list of available strategy names
    pub fn available_sources(&self) -> Vec<&str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }
    
    /// Get list of enabled strategy names
    pub fn enabled_sources(&self) -> Vec<&str> {
        self.strategies
            .iter()
            .filter(|s| s.is_enabled())
            .map(|s| s.name())
            .collect()
    }
    
    /// Search across all enabled strategies
    /// Returns results from all sources combined
    pub async fn search_all(&self, client: &Client, query: &str) -> Vec<MetadataSearchResult> {
        let mut all_results = Vec::new();
        
        for strategy in &self.strategies {
            if !strategy.is_enabled() {
                continue;
            }
            
            match strategy.search(client, query).await {
                Ok(results) => {
                    println!("   [{}] Found {} results", strategy.name(), results.len());
                    all_results.extend(results);
                }
                Err(e) => {
                    println!("   [{}] Search error: {}", strategy.name(), e);
                }
            }
        }
        
        all_results
    }
    
    /// Search across specific sources only
    pub async fn search_sources(
        &self,
        client: &Client,
        query: &str,
        sources: &[&str],
    ) -> Vec<MetadataSearchResult> {
        let mut all_results = Vec::new();
        
        for strategy in &self.strategies {
            if !strategy.is_enabled() || !sources.contains(&strategy.name()) {
                continue;
            }
            
            match strategy.search(client, query).await {
                Ok(results) => {
                    println!("   [{}] Found {} results", strategy.name(), results.len());
                    all_results.extend(results);
                }
                Err(e) => {
                    println!("   [{}] Search error: {}", strategy.name(), e);
                }
            }
        }
        
        all_results
    }
    
    /// Search for the best match across all enabled strategies
    /// Returns the first result found (prioritizing by strategy order)
    pub async fn search_best(&self, client: &Client, query: &str) -> Option<MetadataSearchResult> {
        for strategy in &self.strategies {
            if !strategy.is_enabled() {
                continue;
            }
            
            match strategy.search(client, query).await {
                Ok(results) => {
                    if let Some(first) = results.into_iter().next() {
                        println!("   [{}] Found best match: {}", strategy.name(), first.name);
                        return Some(first);
                    }
                }
                Err(e) => {
                    println!("   [{}] Search error: {}", strategy.name(), e);
                }
            }
        }
        
        None
    }
    
    /// Get details for a specific game from a specific source
    pub async fn get_details(
        &self,
        client: &Client,
        source: &str,
        id: &str,
    ) -> Result<Option<MetadataSearchResult>, String> {
        for strategy in &self.strategies {
            if strategy.name() == source && strategy.is_enabled() {
                return strategy.get_details(client, id).await;
            }
        }
        
        Err(format!("Source '{}' not found or disabled", source))
    }
}

impl Default for MetadataAggregator {
    fn default() -> Self {
        Self::new()
    }
}