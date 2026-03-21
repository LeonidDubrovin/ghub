/// Shared title extraction logic used by both scanning implementations.
/// Provides multi-level fallback strategy for extracting accurate game titles.

use log::debug;
use lazy_static::lazy_static;
use regex_lite::Regex;
use serde_json;
use std::path::Path;

/// Local metadata structure
#[derive(Debug, Clone)]
pub struct LocalMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub version: Option<String>,
}

/// Read local metadata files (game.json, info.txt, README.md, etc.)
pub fn read_local_metadata(dir: &Path, metadata_files: &[String]) -> Option<LocalMetadata> {
    for filename in metadata_files {
        let file_path = dir.join(filename);
        if !file_path.exists() {
            continue;
        }

        debug!("[local_metadata] Found metadata file: {}", filename);

        // Try to read as JSON first
        if filename.ends_with(".json") {
            if let Some(metadata) = read_json_metadata(&file_path) {
                return Some(metadata);
            }
        }

        // Try to read as YAML
        if filename.ends_with(".yaml") || filename.ends_with(".yml") {
            if let Some(metadata) = read_yaml_metadata(&file_path) {
                return Some(metadata);
            }
        }

        // Try to read as TOML
        if filename.ends_with(".toml") {
            if let Some(metadata) = read_toml_metadata(&file_path) {
                return Some(metadata);
            }
        }

        // Try to read as XML
        if filename.ends_with(".xml") {
            if let Some(metadata) = read_xml_metadata(&file_path) {
                return Some(metadata);
            }
        }

        // Try to read as INI
        if filename.ends_with(".ini") {
            if let Some(metadata) = read_ini_metadata(&file_path) {
                return Some(metadata);
            }
        }

        // Try to read as text file
        if let Some(metadata) = read_text_metadata(&file_path) {
            return Some(metadata);
        }
    }

    None
}

/// Read JSON metadata file
fn read_json_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        let metadata = parse_key_value_file(|key| {
            json.get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

        if let Some(ref meta) = &metadata {
            debug!(
                "[local_metadata] Parsed JSON: name={:?}, desc={:?}",
                meta.name,
                meta.description.as_ref().map(|d| &d[..50.min(d.len())])
            );
        }
        return metadata;
    }

    None
}

/// Read YAML metadata file
fn read_yaml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let field_map = parse_key_value_pairs(&content, ':')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!(
            "[local_metadata] Parsed YAML: name={:?}, desc={:?}",
            meta.name,
            meta.description.as_ref().map(|d| &d[..50.min(d.len())])
        );
    }
    metadata
}

/// Read TOML metadata file
fn read_toml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let field_map = parse_key_value_pairs(&content, '=')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!(
            "[local_metadata] Parsed TOML: name={:?}, desc={:?}",
            meta.name,
            meta.description.as_ref().map(|d| &d[..50.min(d.len())])
        );
    }
    metadata
}

/// Read XML metadata file
fn read_xml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let mut field_map = std::collections::HashMap::new();

    // Simple XML parsing - look for <tag>value</tag> patterns
    let tags = [
        "name",
        "title",
        "game_name",
        "description",
        "desc",
        "about",
        "developer",
        "dev",
        "author",
        "publisher",
        "version",
        "ver",
        "release_date",
        "releasedate",
        "date",
    ];

    for tag in &tags {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);

        if let Some(start) = content.find(&open_tag) {
            if let Some(end) = content[start + open_tag.len()..].find(&close_tag) {
                let value = content[start + open_tag.len()..start + open_tag.len() + end].trim();
                if !value.is_empty() {
                    field_map.insert(tag.to_string(), value.to_string());
                }
            }
        }
    }

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!(
            "[local_metadata] Parsed XML: name={:?}, desc={:?}",
            meta.name,
            meta.description.as_ref().map(|d| &d[..50.min(d.len())])
        );
    }
    metadata
}

/// Read INI metadata file
fn read_ini_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let field_map = parse_key_value_pairs(&content, '=')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!(
            "[local_metadata] Parsed INI: name={:?}, desc={:?}",
            meta.name,
            meta.description.as_ref().map(|d| &d[..50.min(d.len())])
        );
    }
    metadata
}

/// Generic function to parse key-value pairs from text formats (YAML, TOML, INI)
fn parse_key_value_pairs(
    content: &str,
    separator: char,
) -> Option<std::collections::HashMap<String, String>> {
    let mut field_map = std::collections::HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(';')
            || line.starts_with('[')
        {
            continue;
        }

        if let Some(sep_pos) = line.find(separator) {
            let key = line[..sep_pos].trim().to_lowercase();
            let value = line[sep_pos + 1..]
                .trim()
                .trim_matches('"')
                .trim_matches('\'');

            if !value.is_empty() {
                field_map.insert(key, value.to_string());
            }
        }
    }

    Some(field_map)
}

/// Generic function to extract metadata fields from a key-value map
fn parse_key_value_file<F>(get_field: F) -> Option<LocalMetadata>
where
    F: Fn(&str) -> Option<String>,
{
    let name = get_field("name")
        .or_else(|| get_field("title"))
        .or_else(|| get_field("game_name"));

    let description = get_field("description")
        .or_else(|| get_field("desc"))
        .or_else(|| get_field("about"));

    let developer = get_field("developer")
        .or_else(|| get_field("dev"))
        .or_else(|| get_field("author"));

    let version = get_field("version").or_else(|| get_field("ver"));

    if name.is_some() || description.is_some() {
        Some(LocalMetadata {
            name,
            description,
            developer,
            version,
        })
    } else {
        None
    }
}

/// Read text metadata file (README, info.txt, etc.)
fn read_text_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let mut metadata = LocalMetadata {
        name: None,
        description: None,
        developer: None,
        version: None,
    };

    let lines: Vec<&str> = content.lines().collect();

    // Try to extract title from first line (often the game name)
    if let Some(first_line) = lines.first() {
        let trimmed = first_line.trim();
        // Remove markdown headers
        let title = trimmed.trim_start_matches('#').trim();
        if !title.is_empty() && title.len() < 100 {
            metadata.name = Some(title.to_string());
        }
    }

    // Try to find description in the content
    // Look for common patterns like "Description:", "About:", etc.
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();

        if lower.contains("description:") || lower.contains("about:") {
            // Take next non-empty line as description
            for j in (i + 1)..lines.len() {
                let desc_line = lines[j].trim();
                if !desc_line.is_empty() {
                    metadata.description = Some(desc_line.to_string());
                    break;
                }
            }
            break;
        }

        if lower.contains("developer:") || lower.contains("author:") || lower.contains("by:") {
            for j in (i + 1)..lines.len() {
                let dev_line = lines[j].trim();
                if !dev_line.is_empty() {
                    metadata.developer = Some(dev_line.to_string());
                    break;
                }
            }
        }

        if lower.contains("version:") {
            for j in (i + 1)..lines.len() {
                let ver_line = lines[j].trim();
                if !ver_line.is_empty() {
                    metadata.version = Some(ver_line.to_string());
                    break;
                }
            }
        }
    }

    // If no description found, use first few lines as description
    if metadata.description.is_none() && lines.len() > 1 {
        let mut desc_lines = Vec::new();
        for line in lines.iter().skip(1).take(5) {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                desc_lines.push(trimmed);
            }
        }
        if !desc_lines.is_empty() {
            metadata.description = Some(desc_lines.join(" "));
        }
    }

    if metadata.name.is_some() || metadata.description.is_some() {
        debug!(
            "[local_metadata] Parsed text: name={:?}, desc={:?}",
            metadata.name,
            metadata.description.as_ref().map(|d| &d[..50.min(d.len())])
        );
        return Some(metadata);
    }

    None
}

/// Clean a game title by removing common noise
pub fn clean_game_title(name: &str) -> String {
    // Remove common suffixes/prefixes
    let mut title = name.to_string();

    // Remove version numbers like v1.0, 1.0.0, V1.1_NEW, v012, etc.
    let re_version =
        regex_lite::Regex::new(r"[\s_]*(?:[vV]\d+(?:[\._]\d+)*|\d+(?:[\._]\d+)+).*$").ok();
    if let Some(re) = re_version {
        title = re.replace(&title, "").to_string();
    }

    // Remove platform tags
    for tag in &[
        "(Windows)",
        "(PC)",
        "(GOG)",
        "(Steam)",
        "[GOG]",
        "[Steam]",
        "(Mac)",
        "(Linux)",
        "_Windows",
        "_PC",
    ] {
        title = title.replace(tag, "");
    }

    // Remove common generic folder names that shouldn't be game titles
    let generic_names = [
        "Windows",
        "BootstrapPackagedGame",
        "Godot Engine",
        "Unity",
        "Unreal",
        "Game",
        "Build",
        "Release",
        "Bin",
        "Binary",
        "Executable",
        "App",
        "win64",
        "win32",
        "linux",
        "macos",
        "x64",
        "x86",
        "WindowsNoEditor",
        "Win64",
        "Win32",
        "Shipping",
        "Development",
        "Debug",
    ];

    let trimmed = title.trim();
    for generic in &generic_names {
        if trimmed.eq_ignore_ascii_case(generic) {
            return String::new(); // Return empty to signal we should use parent dir
        }
    }

    // Clean up trailing/leading underscores and dashes
    title = title
        .trim_matches(|c: char| c == '_' || c == '-' || c == ' ')
        .to_string();

    // Replace underscores with spaces for better readability
    title = title.replace('_', " ");

    // Remove multiple spaces
    let re_spaces = regex_lite::Regex::new(r"\s+").ok();
    if let Some(re) = re_spaces {
        title = re.replace_all(&title, " ").to_string();
    }

    title.trim().to_string()
}

/// Check if an exe product name is generic and shouldn't be used as game title
pub fn is_generic_exe_name(name: &str) -> bool {
    // Only filter out truly generic names that don't identify a specific game
    // Use word-boundary matching to avoid false positives (e.g., "Unity of Command" should not be filtered)

    let name_lower = name.to_lowercase();

    // Check exact matches first (fast path)
    let exact_generic = [
        "godot engine",
        "bootstrappackagedgame",
        "windows",
        "launcher",
        "setup",
        "installer",
        "updater",
        "crashreport",
        "crash handler",
        "unity player",
        "ue4 game",
        "game launcher",
        "application",
        "app",
        "windowsnoeditor",
        "win64",
        "win32",
        "shipping",
        "development",
        "debug",
        "release",
        "player",
        "runtime",
        "redistributable",
        "microsoft",
        "visual",
        "opengl",
        "vulkan",
        "xinput",
        "dinput",
        "physx",
        "nvidia",
        "amd",
        "intel",
        "steam",
        "epic",
        "gog",
        "origin",
        "ubisoft",
        "ea",
        "rockstar",
        "bethesda",
        "2k",
        "sega",
        "square enix",
        "capcom",
        "konami",
        "bandai namco",
        "activision",
        "blizzard",
        "microsoft studios",
        "xbox",
        "playstation",
        "nintendo",
    ];

    for generic in &exact_generic {
        if name_lower == *generic {
            return true;
        }
    }

    // Check regex patterns for word-boundary matching
    lazy_static! {
        static ref GENERIC_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"(?i)^(?:.*\s)?unity(?:\s|$)").unwrap(),
            Regex::new(r"(?i)^(?:.*\s)?unreal(?:\s|$)").unwrap(),
            Regex::new(r"(?i)^(?:.*\s)?ue[45](?:\s|$)").unwrap(),
            Regex::new(r"(?i)^(?:.*\s)?c\+\+(?:\s|$)").unwrap(),
            Regex::new(r"(?i)^(?:.*\s)?battle\.net(?:\s|$)").unwrap(),
        ];
    }

    for pattern in &*GENERIC_PATTERNS {
        if pattern.is_match(&name_lower) {
            return true;
        }
    }

    // Check if name is just version numbers (e.g., "1.0.0", "v2.0")
    if name
        .chars()
        .all(|c| c.is_numeric() || c == '.' || c == '_' || c == '-' || c == 'v' || c == 'V')
    {
        return true;
    }

    // Check if name contains only common non-game words
    let non_game_words = ["test", "demo", "sample", "example", "tutorial", "template"];
    for word in &non_game_words {
        if name_lower == *word {
            return true;
        }
    }

    false
}

/// Check if a game name is problematic (known to cause wrong metadata matches)
pub fn is_problematic_game_name(name: &str) -> bool {
    let problematic_names = [
        "ICARUS",
        "Life Makeover",
        "Microphage",
        "Godot Engine",
        "BootstrapPackagedGame",
        "WindowsNoEditor",
        "Win64",
        "Win32",
        "Shipping",
        "Development",
        "Debug",
        "Release",
    ];

    let name_lower = name.to_lowercase();
    for problematic in &problematic_names {
        if name_lower == problematic.to_lowercase() {
            return true;
        }
    }

    false
}

/// Helper: Check if title is non-empty after trimming
fn get_non_empty_title(title: String) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Helper: Search for a valid title in parent directories up to max_levels up
fn find_title_in_parents(path: &Path, max_levels: u32) -> Option<String> {
    let mut current = path;
    for _ in 0..max_levels {
        if let Some(parent) = current.parent() {
            if let Some(dir_name) = parent.file_name().and_then(|n| n.to_str()) {
                let cleaned = clean_game_title(dir_name);
                if let Some(title) = get_non_empty_title(cleaned) {
                    return Some(title);
                }
            }
            current = parent;
        } else {
            break;
        }
    }
    None
}

/// Helper: Extract a title from an executable filename
fn extract_title_from_executable(executable: &Option<String>) -> Option<String> {
    executable.as_ref().and_then(|exe| {
        let stem = Path::new(exe).file_stem()?.to_str()?;
        // Remove .exe extension if present and clean it
        let cleaned = clean_game_title(stem);
        get_non_empty_title(cleaned)
    })
}

/// Main title extraction with multi-level fallback strategy
pub fn extract_title_with_fallback(
    game_path: &Path,
    dir_name: &str,
    local_metadata: &Option<LocalMetadata>,
    exe_metadata: &Option<crate::models::ExeMetadata>,
    executable: &Option<String>,
) -> String {
    // Level 0: Try local metadata file (game.json, info.txt, etc.)
    if let Some(title) = try_extract_from_local_metadata(local_metadata) {
        debug!("[title] Using local metadata name: '{}'", title);
        return title;
    }

    // Level 1: Try cleaned directory name (PRIMARY - like Playnite uses shortcut/folder names)
    if let Some(title) = try_extract_from_dir_name(dir_name) {
        debug!("[title] Using cleaned dir name: '{}'", title);
        return title;
    }

    // Level 2: Try metadata product name (SECONDARY - only if not generic and exe is in reasonable location)
    if let Some(title) = try_extract_from_exe_metadata(game_path, exe_metadata) {
        debug!("[title] Using metadata product name: '{}'", title);
        return title;
    }

    // Level 3: Try parent directory (up to 3 levels up)
    if let Some(title) = find_title_in_parents(game_path, 3) {
        debug!("[title] Using parent dir: '{}'", title);
        return title;
    }

    // Level 4: Try to extract from best executable name
    if let Some(title) = extract_title_from_executable(executable) {
        debug!("[title] Using executable name: '{}'", title);
        return title;
    }

    // Level 5: Try metadata company name as last resort (only if not generic)
    if let Some(title) = try_extract_from_company_name(exe_metadata) {
        debug!("[title] Using company name: '{}'", title);
        return title;
    }

    // Final fallback: use original dir name or "Unknown Game"
    let fallback = if dir_name != "Unknown" {
        dir_name.to_string()
    } else {
        "Unknown Game".to_string()
    };
    debug!("[title] Using fallback: '{}'", fallback);
    fallback
}

/// Level 0: Extract title from local metadata file
fn try_extract_from_local_metadata(metadata: &Option<LocalMetadata>) -> Option<String> {
    metadata
        .as_ref()
        .and_then(|m| m.name.as_ref())
        .filter(|name| {
            !name.is_empty() && !is_generic_exe_name(name) && !is_problematic_game_name(name)
        })
        .cloned()
}

/// Level 1: Extract title from cleaned directory name
fn try_extract_from_dir_name(dir_name: &str) -> Option<String> {
    get_non_empty_title(clean_game_title(dir_name))
}

/// Level 2: Extract title from exe metadata product name (if not in deep subfolder)
fn try_extract_from_exe_metadata(
    game_path: &Path,
    exe_metadata: &Option<crate::models::ExeMetadata>,
) -> Option<String> {
    let path_str = game_path.to_string_lossy();
    let exe_in_deep_subfolder = path_str.contains("Engine\\Binaries")
        || path_str.contains("Engine/Binaries")
        || path_str.contains("Plugins")
        || path_str.contains("Binaries\\Win64")
        || path_str.contains("Binaries/Win64");

    exe_metadata
        .as_ref()
        .and_then(|m| m.product_name.clone())
        .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
        .filter(|_| !exe_in_deep_subfolder)
}

/// Level 5: Extract title from company name (as last resort)
fn try_extract_from_company_name(exe_metadata: &Option<crate::models::ExeMetadata>) -> Option<String> {
    exe_metadata
        .as_ref()
        .and_then(|m| m.company_name.clone())
        .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_game_title() {
        assert_eq!(clean_game_title("MyGame v1.0"), "MyGame");
        assert_eq!(clean_game_title("  Game  "), "Game");
        assert_eq!(clean_game_title("The Game (windows)"), "Game");
        assert_eq!(clean_game_title("Game - pc"), "Game");
        assert_eq!(clean_game_title("Game_v1.0"), "Game");
    }

    #[test]
    fn test_is_generic_exe_name() {
        assert!(is_generic_exe_name("launcher"));
        assert!(is_generic_exe_name("setup"));
        assert!(is_generic_exe_name("Unity Player"));
        assert!(is_generic_exe_name("UE4 Game"));
        assert!(!is_generic_exe_name("MyGame"));
    }

    #[test]
    fn test_is_problematic_game_name() {
        assert!(is_problematic_game_name("ICARUS"));
        assert!(is_problematic_game_name("Godot Engine"));
        assert!(!is_problematic_game_name("MyGame"));
    }

    #[test]
    fn test_extract_title_with_fallback() {
        let game_path = Path::new("/games/MyGame");
        let dir_name = "MyGame";
        let local_metadata = None;
        let exe_metadata = None;
        let executable = None;

        let title = extract_title_with_fallback(game_path, dir_name, &local_metadata, &exe_metadata, &executable);
        assert_eq!(title, "MyGame");
    }

    #[test]
    fn test_get_non_empty_title() {
        assert_eq!(get_non_empty_title("  ".to_string()), None);
        assert_eq!(get_non_empty_title("Hello".to_string()), Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_key_value_pairs() {
        let content = "name=My Game\ndescription=Test\n";
        let map = parse_key_value_pairs(content, '=').unwrap();
        assert_eq!(map.get("name"), Some(&"My Game".to_string()));
        assert_eq!(map.get("description"), Some(&"Test".to_string()));
    }
}
