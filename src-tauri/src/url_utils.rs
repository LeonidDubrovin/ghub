use regex::Regex;

/// Extract the numeric Steam app ID from a Steam store/community URL.
pub fn extract_steam_app_id(url: &str) -> Option<String> {
    Regex::new(r"/app/(\d+)")
        .ok()
        .and_then(|re| re.captures(url))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn strip_query(url: &str) -> String {
    let mut s = url.trim().to_lowercase();
    if let Some(pos) = s.find('?') {
        s.truncate(pos);
    }
    s
}

fn normalize_url(url: &str) -> String {
    strip_query(url).trim_end_matches('/').to_string()
}

/// Produce a canonical URL for duplicate detection.
///
/// - Steam: `https://store.steampowered.com/app/{app_id}`
/// - itch.io: normalized URL (lowercase, no trailing slash, no query params)
/// - Other/unknown: same normalization as itch.io
pub fn canonical_url(url: &str, source_type: Option<&str>) -> String {
    match source_type {
        Some("steam") => {
            if let Some(app_id) = extract_steam_app_id(url) {
                return format!("https://store.steampowered.com/app/{}", app_id);
            }
        }
        Some("itch") => {
            return normalize_url(url);
        }
        _ => {}
    }
    normalize_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steam_canonical() {
        assert_eq!(
            canonical_url("https://store.steampowered.com/app/123456/GameName/", Some("steam")),
            "https://store.steampowered.com/app/123456"
        );
        assert_eq!(
            canonical_url("https://steamcommunity.com/app/123456", Some("steam")),
            "https://store.steampowered.com/app/123456"
        );
    }

    #[test]
    fn test_itch_canonical() {
        assert_eq!(
            canonical_url("https://Author.itch.io/Game/", Some("itch")),
            "https://author.itch.io/game"
        );
        assert_eq!(
            canonical_url("https://author.itch.io/game?key=value", Some("itch")),
            "https://author.itch.io/game"
        );
    }

    #[test]
    fn test_unknown_canonical() {
        assert_eq!(
            canonical_url("https://example.com/Game/?q=1", None),
            "https://example.com/game"
        );
    }
}
