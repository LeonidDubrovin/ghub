use crate::models::MetadataSearchResult;
use crate::metadata::strategy::MetadataStrategy;
use reqwest::Client;
use async_trait::async_trait;
use scraper::{Html, Selector};
use serde_json;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";

pub struct ItchStrategy {
    enabled: bool,
}

impl ItchStrategy {
    pub fn new() -> Self {
        Self { enabled: true }
    }
    
    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl MetadataStrategy for ItchStrategy {
    fn name(&self) -> &str {
        "itch"
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    async fn search(&self, client: &Client, query: &str) -> Result<Vec<MetadataSearchResult>, String> {
        // Try the undocumented API first as it provides better data and less blocking if it works
        let api_url = format!("https://itch.io/api/1/x/search/games?query={}", urlencoding::encode(query));
        
        let api_resp = client.get(&api_url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .send().await;
            
        if let Ok(resp) = api_resp {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(games) = json.get("games").and_then(|g| g.as_array()) {
                        let mut results = Vec::new();
                        for game in games {
                            let title = game.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
                            let cover_url = game.get("cover_url").and_then(|s| s.as_str()).map(|s| s.to_string());
                            let id = game.get("id").map(|i| i.to_string()).unwrap_or_default();
                            let url = game.get("url").and_then(|s| s.as_str()).map(|s| s.to_string());
                            let short_text = game.get("short_text").and_then(|s| s.as_str()).map(|s| s.to_string());
                            
                            let developer = game.get("user")
                                .and_then(|u| u.get("display_name").or(u.get("username")))
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string());
                                
                            if !title.is_empty() {
                                results.push(MetadataSearchResult {
                                    id: id.clone(),
                                    name: title,
                                    cover_url,
                                    release_date: None,
                                    developer,
                                    publisher: None,
                                    description: short_text,
                                    rating: None,
                                    source: "itch".to_string(),
                                    url,
                                    tags: None,
                                    genres: None,
                                });
                            }
                            if results.len() >= 5 { break; }
                        }
                        if !results.is_empty() {
                            return Ok(results);
                        }
                    }
                }
            }
        }

        // Fallback to DuckDuckGo scraping if API fails or returns empty
        let url = format!("https://duckduckgo.com/html/?q=site:itch.io+{}", urlencoding::encode(query));
        
        let resp = client.get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
            .header("Referer", "https://duckduckgo.com/")
            .send().await.map_err(|e| e.to_string())?;
        
        if !resp.status().is_success() {
            return Err(format!("DDG returned status: {}", resp.status()));
        }
        
        let html = resp.text().await.map_err(|e| e.to_string())?;
        
        let mut game_urls = Vec::new();
        
        {
            let document = Html::parse_document(&html);
            let result_selector = Selector::parse(".result").map_err(|e| e.to_string())?;
            let url_selector = Selector::parse(".result__url").map_err(|e| e.to_string())?;
            
            for result in document.select(&result_selector) {
                if game_urls.len() >= 5 { break; }
                
                let url_text = result.select(&url_selector).next()
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                    
                // Filter out non-game URLs
                if url_text.is_empty() 
                   || url_text.contains("itch.io/games") 
                   || url_text.contains("itch.io/c/") 
                   || url_text.contains("itch.io/t/")
                   || url_text.contains("itch.io/profile/")
                   || url_text.contains("itch.io/devlogs")
                   || !url_text.contains(".itch.io") {
                    continue;
                }
                
                // Ensure protocol
                let full_url = if url_text.starts_with("http") {
                    url_text
                } else {
                    format!("https://{}", url_text)
                };
                
                game_urls.push(full_url);
            }
        } // document dropped
        
        // Now fetch metadata for each game URL
        let mut results = Vec::new();
        
        // We do this sequentially to be kind to the server and avoid rate limits, 
        // but fast enough for 3-5 items.
        for game_url in game_urls {
            match self.get_details(client, &game_url).await {
                Ok(Some(meta)) => results.push(meta),
                _ => continue, // Skip failed
            }
        }
        
        Ok(results)
    }
    
    async fn get_details(&self, client: &Client, url: &str) -> Result<Option<MetadataSearchResult>, String> {
        let resp = client.get(url)
            .header("User-Agent", USER_AGENT)
            .send().await.map_err(|e| e.to_string())?;
            
        let html = resp.text().await.map_err(|e| e.to_string())?;
        let document = Html::parse_document(&html);
        
        let title_selector = Selector::parse("title").unwrap();
        let meta_selector = Selector::parse("meta").unwrap();
        
        let mut title = String::new();
        let mut cover_url = None;
        let mut developer = None;
        let mut description = None;
        
        // Fallback title
        if let Some(el) = document.select(&title_selector).next() {
            title = el.text().collect::<String>().trim().replace(" by itch.io", "").replace(" - itch.io", "");
            // Often "Game Name by Developer"
            if let Some(idx) = title.find(" by ") {
                developer = Some(title[idx+4..].trim().to_string());
                title = title[..idx].trim().to_string();
            }
        }
        
        for meta in document.select(&meta_selector) {
            let property = meta.value().attr("property").unwrap_or("");
            let name = meta.value().attr("name").unwrap_or("");
            let content = meta.value().attr("content").unwrap_or("");
            
            if property == "og:title" && !content.is_empty() {
                title = content.to_string();
            } else if property == "og:image" {
                cover_url = Some(content.to_string());
            } else if property == "og:description" || name == "description" {
                description = Some(content.to_string());
            }
        }
        
        Ok(Some(MetadataSearchResult {
            id: url.to_string(),
            name: title,
            cover_url,
            release_date: None,
            developer,
            publisher: None,
            description,
            rating: None,
            source: "itch".to_string(),
            url: Some(url.to_string()),
            tags: None,
            genres: None,
        }))
    }
}