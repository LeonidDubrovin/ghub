use crate::models::MetadataSearchResult;
use crate::metadata::strategy::MetadataStrategy;
use reqwest::Client;
use async_trait::async_trait;
use scraper::{Html, Selector};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";

pub struct SteamStrategy {
    enabled: bool,
}

impl SteamStrategy {
    pub fn new() -> Self {
        Self { enabled: true }
    }
    
    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl MetadataStrategy for SteamStrategy {
    fn name(&self) -> &str {
        "steam"
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    async fn search(&self, client: &Client, query: &str) -> Result<Vec<MetadataSearchResult>, String> {
        // Steam store search
        let url = format!("https://store.steampowered.com/search/results/?term={}&category1=998&l=english", urlencoding::encode(query));
        let resp = client.get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send().await.map_err(|e| e.to_string())?;
        
        if !resp.status().is_success() {
            return Err(format!("Steam returned status: {}", resp.status()));
        }
        
        let html = resp.text().await.map_err(|e| e.to_string())?;
        
        let mut results = Vec::new();
        
        {
            let document = Html::parse_document(&html);
            // Selector for Steam search results (rows)
            let row_selector = Selector::parse("a.search_result_row").map_err(|e| e.to_string())?;
            let title_selector = Selector::parse(".title").map_err(|e| e.to_string())?;
            let release_selector = Selector::parse(".search_released").map_err(|e| e.to_string())?;
            let img_selector = Selector::parse("img").map_err(|e| e.to_string())?;
            
            for row in document.select(&row_selector) {
                let href = row.value().attr("href").unwrap_or("");
                // Extract App ID from URL: .../app/12345/...
                let id = href.split("/app/").nth(1)
                    .and_then(|s| s.split('/').next())
                    .unwrap_or("")
                    .to_string();
                
                if id.is_empty() { continue; }
                
                let title = row.select(&title_selector).next().map(|el| el.text().collect::<String>()).unwrap_or_default();
                let release_date = row.select(&release_selector).next().map(|el| el.text().collect::<String>().trim().to_string()).unwrap_or_default();
                
                // Steam search images are small caps usually, but we can try to get bigger ones
                // src="..." inside img
                let cover_url = row.select(&img_selector).next()
                    .and_then(|el| el.value().attr("src"))
                    .map(|_s| {
                        // Convert header.jpg to header_292x136.jpg or library_600x900.jpg if possible
                        // Default search is small. Let's assume standard steam logic:
                        // https://cdn.akamai.steamstatic.com/steam/apps/{id}/capsule_sm_120.jpg
                        // Better cover: https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900.jpg
                        format!("https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.jpg", id)
                    });
                
                results.push(MetadataSearchResult {
                    id: id.clone(),
                    name: title,
                    cover_url,
                    release_date: if release_date.is_empty() { None } else { Some(release_date) },
                    developer: None, // Need details page
                    publisher: None,
                    description: None,
                    rating: None,
                    source: "steam".to_string(),
                    url: Some(href.to_string()),
                    tags: None,
                    genres: None,
                });
                
                if results.len() >= 5 { break; }
            }
        } // document is dropped here
        
        // Enrich Steam results with details (sequentially to avoid rate limits)
        let mut enriched_results = Vec::new();
        for mut res in results {
            if let Ok(details) = self.get_details(client, &res.id).await {
                if let Some(details) = details {
                    res.description = details.description.or(res.description);
                    res.developer = details.developer.or(res.developer);
                    res.publisher = details.publisher.or(res.publisher);
                    res.tags = details.tags.or(res.tags);
                    res.genres = details.genres.or(res.genres);
                }
            }
            enriched_results.push(res);
        }
        
        Ok(enriched_results)
    }
    
    async fn get_details(&self, client: &Client, app_id: &str) -> Result<Option<MetadataSearchResult>, String> {
        let url = format!("https://store.steampowered.com/app/{}/", app_id);
        let resp = client.get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            // Bypass age gate by setting cookies
            .header("Cookie", "wants_mature_content=1; birthtime=0; lastagecheckage=1-January-1980") 
            .send().await.map_err(|e| e.to_string())?;
            
        let html = resp.text().await.map_err(|e| e.to_string())?;
        let document = Html::parse_document(&html);
        
        let desc_selector = Selector::parse(".game_description_snippet").unwrap();
        let dev_selector = Selector::parse("#developers_list a").unwrap();
        let tag_selector = Selector::parse("a.app_tag").unwrap();
        let genre_selector = Selector::parse(".details_block a[href*='/genre/']").unwrap();
        
        let description = document.select(&desc_selector).next()
            .map(|el| el.text().collect::<String>().trim().to_string());
            
        let developer = document.select(&dev_selector).next()
            .map(|el| el.text().collect::<String>().trim().to_string());
            
        // Publisher logic is complex, often same as dev or in a specific row. Skipping for now or simple try.
        let publisher = None; 
        
        let tags: Vec<String> = document.select(&tag_selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty() && s != "+")
            .take(10)
            .collect();
            
        let genres: Vec<String> = document.select(&genre_selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect();

        Ok(Some(MetadataSearchResult {
            id: app_id.to_string(),
            name: String::new(), // Not needed for merge
            cover_url: None,
            release_date: None,
            developer,
            publisher,
            description,
            rating: None,
            source: "steam".to_string(),
            url: None,
            tags: if tags.is_empty() { None } else { Some(tags) },
            genres: if genres.is_empty() { None } else { Some(genres) },
        }))
    }
}