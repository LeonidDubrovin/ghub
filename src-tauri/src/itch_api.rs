use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub(crate) const ITCH_API_BASE: &str = "https://api.itch.io";

/// Save a raw byte payload to a debug file next to the executable for inspection.
fn save_debug_bytes(name: &str, bytes: &[u8]) -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let debug_dir = exe.parent().unwrap_or_else(|| Path::new(".")).join("logs");
    let _ = std::fs::create_dir_all(&debug_dir);
    let debug_path = debug_dir.join(name);
    if let Err(e) = std::fs::write(&debug_path, bytes) {
        error!("Failed to write debug bytes to {}: {}", debug_path.display(), e);
        return None;
    }
    Some(debug_path)
}

/// A single upload returned by the itch.io API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItchApiUpload {
    pub id: i64,
    pub filename: String,
    #[serde(rename = "display_name")]
    pub display_name: Option<String>,
    pub size: i64,
    #[serde(default)]
    pub created_at: Option<String>,
    pub platforms: Option<ItchUploadPlatforms>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ItchUploadPlatforms {
    #[serde(default)]
    pub windows: bool,
    #[serde(default)]
    pub linux: bool,
    #[serde(default)]
    pub osx: bool,
    #[serde(default)]
    pub android: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItchUploadsResponse {
    #[serde(default)]
    pub uploads: Option<Value>,
}

/// A single game returned by the authenticated itch.io search API.
#[derive(Debug, Clone, Deserialize)]
pub struct ItchApiGameResult {
    pub id: i64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub short_text: Option<String>,
    #[serde(default)]
    pub user: Option<Value>,
}

impl ItchApiGameResult {
    /// Extract the author name from the nested `user` object.
    pub fn author(&self) -> Option<String> {
        self.user.as_ref()
            .and_then(|u| u.get("display_name").or(u.get("username")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Small client for the internal itch.io API used by the official app.
pub struct ItchApiClient {
    api_key: String,
}

impl ItchApiClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// Fetch uploads for a game by its itch.io game ID.
    pub async fn get_game_uploads(
        &self,
        client: &Client,
        game_id: i64,
    ) -> Result<Vec<ItchApiUpload>, String> {
        let url = format!("{}/games/{}/uploads", ITCH_API_BASE, game_id);
        info!("Fetching itch uploads from {}", url);
        let resp = client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch uploads: {}", e))?;

        let status = resp.status();
        let body = resp.bytes().await.map_err(|e| format!("Failed to read uploads response: {}", e))?;
        debug!("Uploads response ({}), {} bytes", status, body.len());

        if !status.is_success() {
            let preview = String::from_utf8_lossy(&body);
            return Err(format!("Uploads endpoint returned status {}: {}", status, preview));
        }

        let parsed: ItchUploadsResponse = serde_json::from_slice(&body)
            .map_err(|e| format!("Failed to parse uploads response: {} (body: {})", e, String::from_utf8_lossy(&body)))?;

        let uploads = match parsed.uploads {
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|v| serde_json::from_value(v.clone()).map_err(|e| format!("Invalid upload item: {}", e)))
                .collect::<Result<Vec<_>, _>>()?,
            Some(Value::Object(map)) => map
                .values()
                .map(|v| serde_json::from_value(v.clone()).map_err(|e| format!("Invalid upload item: {}", e)))
                .collect::<Result<Vec<_>, _>>()?,
            Some(Value::Null) | None => Vec::new(),
            _ => return Err(format!("Unexpected uploads type in response: {}", String::from_utf8_lossy(&body))),
        };

        Ok(uploads)
    }

    /// Request a signed download URL for a specific upload.
    pub async fn get_download_url(
        &self,
        client: &Client,
        upload_id: i64,
    ) -> Result<String, String> {
        let url = format!("{}/uploads/{}/download", ITCH_API_BASE, upload_id);
        info!("Requesting itch download URL for upload {}", upload_id);
        let resp = client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch download URL: {}", e))?;

        let status = resp.status();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        let final_url = resp.url().to_string();
        info!(
            "Download URL response: status {}, content-type '{}' from {}",
            status, content_type, final_url
        );

        if !status.is_success() {
            let body = resp.bytes().await.map_err(|e| format!("Failed to read download URL response: {}", e))?;
            let debug_name = format!("itch_download_url_debug_{}.bin", upload_id);
            let path = save_debug_bytes(&debug_name, &body);
            let preview = String::from_utf8_lossy(&body);
            return Err(format!(
                "Download URL endpoint returned status {}: {}. Debug body saved to {}",
                status,
                preview,
                path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string())
            ));
        }

        // If the server returned the file directly (non-JSON content type), use the final URL as the direct download link.
        if !content_type.contains("application/json") {
            info!(
                "Download URL endpoint returned non-JSON content type '{}', using final URL as direct download",
                content_type
            );
            return Ok(final_url);
        }

        let body = resp.bytes().await.map_err(|e| format!("Failed to read download URL response: {}", e))?;
        debug!("Download URL response body ({}): {}", body.len(), String::from_utf8_lossy(&body));

        let value: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                // If the body is not valid JSON, fall back to the final URL. This happens when the endpoint
                // returns the file directly without a JSON wrapper.
                let debug_name = format!("itch_download_url_debug_{}.bin", upload_id);
                let path = save_debug_bytes(&debug_name, &body);
                info!(
                    "Download URL response was not valid JSON: {}. Falling back to final URL. Debug body saved to {}",
                    e,
                    path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string())
                );
                return Ok(final_url);
            }
        };

        if let Some(errors) = value.get("errors").and_then(|v| v.as_array()) {
            let msgs: Vec<String> = errors
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !msgs.is_empty() {
                return Err(msgs.join("; "));
            }
        }

        value
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                let preview = String::from_utf8_lossy(&body);
                format!("Download URL response did not contain a URL: {}", preview)
            })
    }

    /// Try to resolve a game ID by searching the documented itch.io API with the
    /// page URL (or slug/title) and matching the returned URL to the input URL.
    /// This avoids scraping the page with fragile regexes.
    pub async fn resolve_game_id_by_url(
        &self,
        client: &Client,
        url: &str,
        title: Option<&str>,
    ) -> Result<Option<i64>, String> {
        let mut queries = Vec::new();
        if let Some(t) = title {
            let t = t.trim();
            if !t.is_empty() {
                queries.push(t.to_string());
            }
        }
        if let Some(slug) = url.trim_end_matches('/').split('/').last() {
            let slug = slug.trim();
            if !slug.is_empty() {
                let spaced = slug.replace('-', " ").replace('_', " ");
                if spaced != slug {
                    queries.push(spaced);
                }
                queries.push(slug.to_string());
            }
        }
        queries.dedup();

        let target = normalize_itch_url(url);

        for query in &queries {
            let api_url = format!(
                "https://itch.io/api/1/{}/search/games?query={}",
                urlencoding::encode(&self.api_key),
                urlencoding::encode(query)
            );
            info!("Resolving itch game id via search API: query='{}'", query);
            let resp = client
                .get(&api_url)
                .header("User-Agent", crate::http_constants::USER_AGENT)
                .header("Accept-Language", crate::http_constants::ACCEPT_LANGUAGE)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| format!("Failed to call itch search API: {}", e))?;

            let status = resp.status();
            let body = resp
                .bytes()
                .await
                .map_err(|e| format!("Failed to read search response: {}", e))?;
            debug!("Search response ({}): {}", status, String::from_utf8_lossy(&body));

            if !status.is_success() {
                let preview = String::from_utf8_lossy(&body);
                return Err(format!("Itch search API returned status {}: {}", status, preview));
            }

            let parsed: Value = serde_json::from_slice(&body).map_err(|e| {
                format!(
                    "Failed to parse search response: {} (body: {})",
                    e,
                    String::from_utf8_lossy(&body)
                )
            })?;

            if let Some(games) = parsed.get("games").and_then(|v| v.as_array()) {
                for game in games {
                    if let (Some(id), Some(game_url)) = (
                        game.get("id").and_then(|v| v.as_i64()),
                        game.get("url").and_then(|v| v.as_str()),
                    ) {
                        if normalize_itch_url(game_url) == target {
                            info!("Resolved game id {} for {} via search API", id, url);
                            return Ok(Some(id));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Search for games by title using the authenticated itch.io search API.
    /// This is the same endpoint used by the official itch.io app and is more
    /// reliable than the anonymous `x/search/games` endpoint.
    pub async fn search_games(
        &self,
        client: &Client,
        query: &str,
    ) -> Result<Vec<ItchApiGameResult>, String> {
        let url = format!(
            "https://itch.io/api/1/{}/search/games?query={}",
            urlencoding::encode(&self.api_key),
            urlencoding::encode(query)
        );
        info!("Searching itch games via authenticated API: query='{}'", query);
        let resp = client
            .get(&url)
            .header("User-Agent", crate::http_constants::USER_AGENT)
            .header("Accept-Language", crate::http_constants::ACCEPT_LANGUAGE)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Failed to call itch search API: {}", e))?;

        let status = resp.status();
        let body = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read search response: {}", e))?;
        debug!(
            "Itch search response ({}): {}",
            status,
            String::from_utf8_lossy(&body)
        );

        if !status.is_success() {
            let preview = String::from_utf8_lossy(&body);
            return Err(format!("Itch search API returned status {}: {}", status, preview));
        }

        let parsed: Value = serde_json::from_slice(&body).map_err(|e| {
            format!(
                "Failed to parse itch search response: {} (body: {})",
                e,
                String::from_utf8_lossy(&body)
            )
        })?;

        let games = parsed
            .get("games")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut results = Vec::new();
        for game in games {
            let parsed_game: ItchApiGameResult = match serde_json::from_value(game.clone()) {
                Ok(g) => g,
                Err(e) => {
                    debug!("Skipping malformed itch search result: {}", e);
                    continue;
                }
            };
            if !parsed_game.title.is_empty() {
                results.push(parsed_game);
            }
            if results.len() >= 10 {
                break;
            }
        }

        Ok(results)
    }
}

fn normalize_itch_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_lowercase()
}

/// Extract the itch.io game ID from a game page's HTML.
/// The only fallback we keep is the reliable `itch:path` meta tag.
pub fn extract_game_id_from_page(html: &str) -> Result<i64, String> {
    // itch pages have a meta tag with the canonical path, e.g.
    // <meta name="itch:path" content="games/12345"/> or <meta content="games/12345" name="itch:path"/>.
    // The attributes can appear in any order, so we first find all candidate meta tags and then check
    // whether the tag also contains the itch:path name.
    let path_re = regex::Regex::new(r#"<meta[^>]*\bcontent=["']?games/(\d+)["']?[^>]*>"#)
        .map_err(|e| format!("Failed to compile itch:path content regex: {}", e))?;
    for caps in path_re.captures_iter(html) {
        let tag = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        if tag.contains("itch:path") {
            if let Some(id) = caps.get(1).and_then(|m| m.as_str().parse::<i64>().ok()) {
                return Ok(id);
            }
        }
    }

    Err("itch:path meta tag not found on page".to_string())
}

/// Fetch an itch.io game page and extract the game ID from the `itch:path` meta tag.
/// This is only used as a fallback when the API search does not resolve the URL.
pub async fn fetch_game_id_from_url(client: &Client, url: &str) -> Result<i64, String> {
    let resp = client
        .get(url)
        .header("User-Agent", crate::http_constants::USER_AGENT)
        .header("Accept-Language", crate::http_constants::ACCEPT_LANGUAGE)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch itch page: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("itch page returned status: {}", resp.status()));
    }

    let html = resp.text().await.map_err(|e| format!("Failed to read itch page: {}", e))?;
    match extract_game_id_from_page(&html) {
        Ok(id) => return Ok(id),
        Err(e) => {
            debug!(
                "Failed to extract game id from {} (html length {}). Error: {}",
                url,
                html.len(),
                e
            );
            // Save the HTML for debugging so the user can share it if needed.
            if let Ok(exe) = std::env::current_exe() {
                let debug_path = exe
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join("logs")
                    .join("itch_debug.html");
                let _ = std::fs::create_dir_all(
                    debug_path.parent().unwrap_or_else(|| std::path::Path::new(".")),
                );
                let _ = std::fs::write(&debug_path, &html);
                debug!("Saved failing itch HTML to {}", debug_path.display());
            }
        }
    }

    Err("No game ID found on page. Tried: itch search API and itch:path meta tag.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_game_id_from_itch_path_meta() {
        let html = r#"<meta name="itch:path" content="games/2646547"/>"#;
        assert_eq!(extract_game_id_from_page(html).unwrap(), 2646547);
    }

    #[test]
    fn test_extract_game_id_from_itch_path_meta_reversed() {
        let html = r#"<meta content="games/2162531" name="itch:path"/>"#;
        assert_eq!(extract_game_id_from_page(html).unwrap(), 2162531);
    }
}
