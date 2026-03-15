use crate::models::MetadataSearchResult;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";

pub async fn search_steam(client: &Client, query: &str) -> Result<Vec<MetadataSearchResult>, String> {
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
        if let Ok(details) = fetch_steam_details(client, &res.id).await {
            res.description = details.description.or(res.description);
            res.developer = details.developer.or(res.developer);
            res.publisher = details.publisher.or(res.publisher);
            res.tags = details.tags.or(res.tags);
            res.genres = details.genres.or(res.genres);
            // If main cover is small, details might have better one? 
            // Actually the one we construct from ID is usually good.
        }
        enriched_results.push(res);
    }
    
    Ok(enriched_results)
}

async fn fetch_steam_details(client: &Client, app_id: &str) -> Result<MetadataSearchResult, String> {
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
    // let pub_selector = Selector::parse(".dev_row > .summary.column a").unwrap(); // This selector is tricky on Steam
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

    Ok(MetadataSearchResult {
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
    })
}

pub async fn search_itch(client: &Client, query: &str) -> Result<Vec<MetadataSearchResult>, String> {
    // Try the undocumented API first as it provides better data and less blocking if it works
    let api_url = format!("https://itch.io/api/1/x/search/games?query={}", urlencoding::encode(query));
    // println!("Trying Itch API: {}", api_url);
    
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
                            
                        // API usually doesn't give tags directly in search list, might need deep fetch
                        
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
        match fetch_itch_game_page(client, &game_url).await {
            Ok(meta) => results.push(meta),
            Err(_) => continue, // Skip failed
        }
    }
    
    Ok(results)
}

async fn fetch_itch_game_page(client: &Client, url: &str) -> Result<MetadataSearchResult, String> {
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
        } else if property == "og:site_name" {
            // Sometimes site name is "itch.io", sometimes developer
            if content != "itch.io" {
                // developer = Some(content.to_string()); // Not reliable
            }
        }
    }
    
    Ok(MetadataSearchResult {
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
    })
}
