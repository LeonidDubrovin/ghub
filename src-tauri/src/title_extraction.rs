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

    // Remove "The " prefix at the beginning (common article)
    if title.to_lowercase().starts_with("the ") {
        title = title[4..].trim().to_string();
    }

    // Remove version numbers like v1.0, 1.0.0, V1.1_NEW, v012, etc.
    // Only match if preceded by separator (space, underscore, dash) to avoid matching version-only names like "0.0.15c demo"
    let re_version =
        regex_lite::Regex::new(r"[ _-](?:[vV]\d+(?:[\._]\d+)*|\d+(?:[\._]\d+)+).*$").ok();
    if let Some(re) = re_version {
        title = re.replace(&title, "").to_string();
    }

    // Remove platform tags (case-insensitive)
    for tag in &[
        "(Windows)", "(PC)", "(GOG)", "(Steam)", "[GOG]", "[Steam]", "(Mac)", "(Linux)",
        "_Windows", "_PC", "_GOG", "_Steam", " - pc",
    ] {
        let lower_title = title.to_lowercase();
        let lower_tag = tag.to_lowercase();
        if lower_title.contains(&lower_tag) {
            // Find actual case in title and remove
            if let Some(pos) = lower_title.find(&lower_tag) {
                title = format!("{}{}", &title[..pos], &title[pos + tag.len()..]);
            }
        }
    }

    // Remove trailing incomplete parentheses/brackets (e.g., "Game (Demo", "Game (", "Game [")
    // These occur due to folder name truncation. Remove any trailing segment that starts
    // with '(' or '[' and lacks a closing ')' or ']'.
    let re_incomplete = regex_lite::Regex::new(r"\s*[\(\[][^\)\]\r\n]*$").ok();
    if let Some(re) = re_incomplete {
        title = re.replace(&title, "").to_string();
    }
    title = title.trim().to_string();

    // Remove common generic folder names that shouldn't be game titles
    let generic_names = [
        "Windows",
        "BootstrapPackagedGame",
        "Godot Engine",
        "Unity",
        "Unreal",
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
        // Additional generic names
        "PC",
        "Desktop",
        "Binaries",
        "Resources",
        "en-us",
        "en_us",
        "en-US",
        "MACOSX",
        "osx",
        "Steam",
        "GOG",
        "Epic",
        "Origin",
        "Uplay",
        "Battle.net",
        "Amazon",
        "Xbox",
        "PlayStation",
        "Nintendo",
        "Switch",
        "3DS",
        "Vita",
        "PSP",
        "PSX",
        "PS2",
        "PS3",
        "PS4",
        "PS5",
        "XB1",
        "XB360",
        "XboxOne",
        "XboxSeriesX",
        "XboxSeriesXS",
        "games",
        "list",
        "pack",
        "bundle",
        "indie",
        "freeware",
        "shareware",
        "trial",
        "beta",
        "alpha",
        "preview",
        "test",
        "sample",
        "example",
        "tutorial",
        "template",
        "drmfree",
        "launcher",
        "setup",
        "installer",
        "updater",
        "crashhandler",
        "crash_report",
        "unins000",
        "unins001",
        "unins002",
        "unins003",
        "unins004",
        "unins005",
        "unins006",
        "unins007",
        "unins008",
        "unins009",
        "unins010",
        // Additional generic folder names that should not be used as game titles
        "engine",
        "jre",
        "jdk",
        "runtime",
        "runtimes",
        "bin",
        "win",
        "windows",
        "x64",
        "x86",
        "gmlive",
        "__macosx",
        "d3d12",
        "new folder",
        "windowsclient",
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
        "microsoft corporation",
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
                // Use raw dir name (trim only, no underscore->space conversion) to preserve underscores
                let trimmed = dir_name.trim();
                // Check if not generic (case-insensitive)
                let is_generic = [
                    "windows", "bootstrap", "godot", "engine", "unity", "unreal", "build", "release",
                    "bin", "binary", "executable", "app", "win64", "win32", "linux", "macos", "x64", "x86",
                    "windowsnoeditor", "win", "shipping", "development", "debug", "pc", "desktop",
                    "binaries", "resources", "en-us", "en_us", "en-US", "macosx", "osx", "steam", "gog",
                    "epic", "origin", "uplay", "battle.net", "amazon", "xbox", "playstation", "nintendo",
                    "switch", "3ds", "vita", "psp", "psx", "ps2", "ps3", "ps4", "ps5", "xb1", "xb360",
                    "xboxone", "xboxseriesx", "xboxseriesxs", "games", "list", "pack",
                    "bundle", "indie", "freeware", "shareware", "trial", "beta", "alpha", "preview",
                    "test", "sample", "example", "tutorial", "template", "drmfree", "launcher", "setup",
                    "installer", "updater", "crashhandler", "crash_report", "unins000", "unins001",
                    "unins002", "unins003", "unins004", "unins005", "unins006", "unins007", "unins008",
                    "unins009", "unins010", "engine", "jre", "jdk", "runtime", "runtimes", "bin", "gmlive",
                    "__macosx", "d3d12", "new folder", "windowsclient",
                ].contains(&trimmed.to_lowercase().as_str());
                if !trimmed.is_empty() && !is_generic {
                    return Some(trimmed.to_string());
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
            !name.is_empty() 
                && !is_generic_exe_name(name) 
                && !is_problematic_game_name(name)
                && !is_likely_sentence(name)
                && is_valid_title_name(name)
        })
        .map(|name| name.trim().to_string())
}

/// Check if a title name is valid (contains at least one alphanumeric char, not just punctuation)
fn is_valid_title_name(name: &str) -> bool {
    if name.len() < 2 {
        return false;
    }
    // Must contain at least one alphanumeric character
    name.chars().any(|c| c.is_alphanumeric())
}

/// Check if a string looks like a descriptive sentence rather than a game title.
/// Sentences often contain verb phrases, colons, or attribution words.
fn is_likely_sentence(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Common verb indicators that suggest a sentence
    let sentence_indicators = [
        " is ", " are ", " was ", " were ", " has ", " have ", " do ", " does ", " did ",
        " will ", " would ", " could ", " should ", " may ", " might ", " must ", " can ", " cannot ",
        " for ", " by ", " with ", " and ", " or ", " but ", " if ", " then ", " else ",
    ];
    for indicator in &sentence_indicators {
        if lower.contains(indicator) {
            return true;
        }
    }
    // Ends with colon (common in descriptions like "garden is a collaboration between:")
    if name.trim_end().ends_with(':') {
        return true;
    }
    // Attribution words often found in descriptions
    if lower.contains(" by ") || lower.contains(" with ") {
        return true;
    }
    // Check for label-like patterns: "Controls:", "Instructions:", "System requirements:", etc.
    let label_prefixes = [
        "controls", "instructions", "description", "about", "how to play", "control",
        "movement", "system requirements", "requirements", "specs", "specifications",
    ];
    if let Some(colon_pos) = lower.find(':') {
        let before = lower[..colon_pos].trim();
        if label_prefixes.contains(&before) {
            return true;
        }
    }
    false
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
    use crate::models::ExeMetadata;

    #[test]
    fn test_clean_game_title() {
        assert_eq!(clean_game_title("MyGame v1.0"), "MyGame");
        assert_eq!(clean_game_title("  Game  "), "Game");
        assert_eq!(clean_game_title("The Game (windows)"), "Game");
        assert_eq!(clean_game_title("Game - pc"), "Game");
        assert_eq!(clean_game_title("Game_v1.0"), "Game");
        
        // Test version number removal
        assert_eq!(clean_game_title("Game v1.2.3"), "Game");
        assert_eq!(clean_game_title("Game V1.0.0"), "Game");
        assert_eq!(clean_game_title("Game_1.0.1"), "Game");
        assert_eq!(clean_game_title("Game-2.0.0-beta"), "Game");
        
        // Test platform tag removal (case-insensitive)
        assert_eq!(clean_game_title("Game (Windows)"), "Game");
        assert_eq!(clean_game_title("Game (windows)"), "Game");
        assert_eq!(clean_game_title("Game (PC)"), "Game");
        assert_eq!(clean_game_title("Game_GOG"), "Game");
        assert_eq!(clean_game_title("Game_Steam"), "Game");
        
        // Test trailing parenthesis stripping (truncation)
        assert_eq!(clean_game_title("Game ("), "Game");
        assert_eq!(clean_game_title("Game ["), "Game");
        assert_eq!(clean_game_title("Game (Demo"), "Game");
        
        // Test generic folder names that should return empty
        assert!(clean_game_title("Windows").is_empty());
        assert!(clean_game_title("win64").is_empty());
        assert!(clean_game_title("Binaries").is_empty());
        assert!(clean_game_title("Engine").is_empty());
        assert!(clean_game_title("jre").is_empty());
        assert!(clean_game_title("en-us").is_empty());
        assert!(clean_game_title("MACOSX").is_empty());
        assert!(clean_game_title("gmlive").is_empty());
        assert!(clean_game_title("Build").is_empty());
        assert!(clean_game_title("runtime").is_empty());
        
        // Test valid names
        assert_eq!(clean_game_title("My Awesome Game"), "My Awesome Game");
        assert_eq!(clean_game_title("Game_2022Update"), "Game 2022Update");
        assert_eq!(clean_game_title("(Win)Project Troll v2.2"), "(Win)Project Troll");
    }

    #[test]
    fn test_is_likely_sentence() {
        // Sentences with verb indicators
        assert!(is_likely_sentence("This is a game"));
        assert!(is_likely_sentence("garden is a collaboration between:"));
        assert!(is_likely_sentence("The game was made by John"));
        assert!(is_likely_sentence("Controls: WASD to move"));
        assert!(is_likely_sentence("To play the game you must extract the folder"));
        assert!(is_likely_sentence("made by Friedrich Hanisch"));
        assert!(is_likely_sentence("System requirements: 4GB RAM"));
        
        // Valid game titles (not sentences)
        assert!(!is_likely_sentence("MyGame"));
        assert!(!is_likely_sentence("The Legend of Zelda"));
        assert!(!is_likely_sentence("Super Mario Odyssey"));
        assert!(!is_likely_sentence("Game Name 2022"));
        assert!(!is_likely_sentence("Bikrash"));
        assert!(!is_likely_sentence("DANGEON!"));
    }

    #[test]
    fn test_is_generic_exe_name() {
        assert!(is_generic_exe_name("launcher"));
        assert!(is_generic_exe_name("setup"));
        assert!(is_generic_exe_name("Unity Player"));
        assert!(is_generic_exe_name("UE4 Game"));
        assert!(is_generic_exe_name("godot engine"));
        assert!(is_generic_exe_name("BootstrapPackagedGame"));
        assert!(is_generic_exe_name("crashreport"));
        assert!(is_generic_exe_name("WindowsNoEditor"));
        assert!(is_generic_exe_name("shipping"));
        assert!(is_generic_exe_name("debug"));
        assert!(is_generic_exe_name("runtime"));
        assert!(is_generic_exe_name("redistributable"));
        
        // Should not be generic
        assert!(!is_generic_exe_name("MyGame"));
        assert!(!is_generic_exe_name("Awesome Game"));
        assert!(!is_generic_exe_name("Roguelike"));
        assert!(!is_generic_exe_name("Project Troll"));
    }

    #[test]
    fn test_is_problematic_game_name() {
        assert!(is_problematic_game_name("ICARUS"));
        assert!(is_problematic_game_name("Godot Engine"));
        assert!(is_problematic_game_name("BootstrapPackagedGame"));
        assert!(is_problematic_game_name("WindowsNoEditor"));
        assert!(is_problematic_game_name("Win64"));
        assert!(is_problematic_game_name("Shipping"));
        
        assert!(!is_problematic_game_name("MyGame"));
        assert!(!is_problematic_game_name("Roguelike"));
    }

    #[test]
    fn test_get_non_empty_title() {
        assert_eq!(get_non_empty_title("  ".to_string()), None);
        assert_eq!(get_non_empty_title("Hello".to_string()), Some("Hello".to_string()));
        assert_eq!(get_non_empty_title("".to_string()), None);
        assert_eq!(get_non_empty_title("   Game   ".to_string()), Some("Game".to_string()));
    }

    #[test]
    fn test_parse_key_value_pairs() {
        let content = "name=My Game\ndescription=Test\n";
        let map = parse_key_value_pairs(content, '=').unwrap();
        assert_eq!(map.get("name"), Some(&"My Game".to_string()));
        assert_eq!(map.get("description"), Some(&"Test".to_string()));
        
        // Test with quotes
        let content = "name=\"My Game\"\ndesc='Test'\n";
        let map = parse_key_value_pairs(content, '=').unwrap();
        assert_eq!(map.get("name"), Some(&"My Game".to_string()));
        assert_eq!(map.get("desc"), Some(&"Test".to_string()));
        
        // Test with YAML-style colon separator
        let content = "name: My Game\ndescription: Test\n";
        let map = parse_key_value_pairs(content, ':').unwrap();
        assert_eq!(map.get("name"), Some(&"My Game".to_string()));
        assert_eq!(map.get("description"), Some(&"Test".to_string()));
    }

    #[test]
    fn test_try_extract_from_local_metadata() {
        // Valid metadata with good name
        let metadata = Some(LocalMetadata {
            name: Some("Bikrash".to_string()),
            description: Some("Pedaling ..... W Back ......... A Handling ..... A".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, Some("Bikrash".to_string()));
        
        // Metadata with sentence-like name should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("To play the game you must extract the folder".to_string()),
            description: None,
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // Metadata with generic name should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("Windows".to_string()),
            description: None,
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // Metadata with empty name
        let metadata = Some(LocalMetadata {
            name: Some("".to_string()),
            description: Some("A good game".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // No metadata
        let metadata: Option<LocalMetadata> = None;
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
    }

    #[test]
    fn test_extract_title_with_fallback_scenarios() {
        let game_path = Path::new("/games/MyGame");
        
        // Scenario 1: Local metadata with good name (Level 0)
        let local_metadata = Some(LocalMetadata {
            name: Some("Bikrash".to_string()),
            description: Some("Pedaling game".to_string()),
            developer: None,
            version: None,
        });
        let exe_metadata = None;
        let executable: Option<String> = None;
        let title = extract_title_with_fallback(
            game_path,
            "Bikrash_0.6",
            &local_metadata,
            &exe_metadata,
            &Some("Bikrash.exe".to_string()),
        );
        assert_eq!(title, "Bikrash");
        
        // Scenario 2: No metadata, good dir name (Level 1)
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let title = extract_title_with_fallback(
            game_path,
            "My Awesome Game_v1.0",
            &local_metadata,
            &exe_metadata,
            &None,
        );
        assert_eq!(title, "My Awesome Game");
        
        // Scenario 3: Dir name is generic, use parent (Level 3)
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/MyCollection/Windows");
        let title = extract_title_with_fallback(
            game_path,
            "Windows",
            &local_metadata,
            &exe_metadata,
            &None,
        );
        assert_eq!(title, "MyCollection");
        
        // Scenario 4: Dir name is "jre" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/Greedy Miners/Greedy Miners/jre/bin");
        let title = extract_title_with_fallback(
            game_path,
            "jre",
            &local_metadata,
            &exe_metadata,
            &Some("javaws.exe".to_string()),
        );
        assert_eq!(title, "Greedy Miners");
        
        // Scenario 5: Dir name is "en-us" (language folder), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/Game/Engine/Extras/Redist/en-us");
        let title = extract_title_with_fallback(
            game_path,
            "en-us",
            &local_metadata,
            &exe_metadata,
            &None,
        );
        assert_eq!(title, "Redist");
        
        // Scenario 6: Dir name is "Build" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/MyGame/Build");
        let title = extract_title_with_fallback(
            game_path,
            "Build",
            &local_metadata,
            &exe_metadata,
            &None,
        );
        assert_eq!(title, "MyGame");
        
        // Scenario 7: Dir name is "D3D12" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/Blattgold Download/build/D3D12");
        let title = extract_title_with_fallback(
            game_path,
            "D3D12",
            &local_metadata,
            &exe_metadata,
            &Some("BlattGold.exe".to_string()),
        );
        assert_eq!(title, "Blattgold Download");
        
        // Scenario 8: Dir name is "New folder" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/Animal Crushing/New folder");
        let title = extract_title_with_fallback(
            game_path,
            "New folder",
            &local_metadata,
            &exe_metadata,
            &Some("AssetJamSource.exe".to_string()),
        );
        assert_eq!(title, "Animal Crushing");
        
        // Scenario 9: Dir name is "WindowsClient" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/Balance'em/WindowsClient");
        let title = extract_title_with_fallback(
            game_path,
            "WindowsClient",
            &local_metadata,
            &exe_metadata,
            &Some("Balance'em.exe".to_string()),
        );
        assert_eq!(title, "Balance'em");
        
        // Scenario 10: Dir name is "Win" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games/GMTK2025/Win");
        let title = extract_title_with_fallback(
            game_path,
            "Win",
            &local_metadata,
            &exe_metadata,
            &Some("GMTK2025.exe".to_string()),
        );
        assert_eq!(title, "GMTK2025");
        
        // Scenario 11: Dir name is "games_tmp_for_dev" (generic), should use parent
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games_genre/games_tmp_for_dev/0.0.15c demo");
        let title = extract_title_with_fallback(
            game_path,
            "0.0.15c demo",
            &local_metadata,
            &exe_metadata,
            &Some("Glorysmith.exe".to_string()),
        );
        // Since "0.0.15c demo" is not in generic list, it should be used
        assert_eq!(title, "0.0.15c demo");
        
        // Scenario 12: Dir name is "games" (generic), should use parent "games_tmp_for_dev" (preserve underscores)
        let local_metadata = None;
        let exe_metadata = None;
        let executable: Option<String> = None;
        let game_path = Path::new("/games_genre/games_tmp_for_dev/games");
        let title = extract_title_with_fallback(
            game_path,
            "games",
            &local_metadata,
            &exe_metadata,
            &None,
        );
        assert_eq!(title, "games_tmp_for_dev");
    }

    #[test]
    fn test_extract_title_from_executable() {
        // Test with valid executable name
        let executable = Some("MyGame.exe".to_string());
        let title = extract_title_from_executable(&executable);
        assert_eq!(title, Some("MyGame".to_string()));
        
        // Test with version in exe name
        let executable = Some("MyGame_v1.0.exe".to_string());
        let title = extract_title_from_executable(&executable);
        assert_eq!(title, Some("MyGame".to_string()));
        
        // Test with generic exe name (should be filtered by clean_game_title)
        let executable = Some("launcher.exe".to_string());
        let title = extract_title_from_executable(&executable);
        assert_eq!(title, None);
        
        // Test with no executable
        let executable: Option<String> = None;
        let title = extract_title_from_executable(&executable);
        assert_eq!(title, None);
    }

    #[test]
    fn test_real_world_examples_from_logs() {
        // From log: (Win)Project Troll v2.2 -> (Win)Project Troll
        assert_eq!(clean_game_title("(Win)Project Troll v2.2"), "(Win)Project Troll");
        
        // From log: 0.0.15c demo -> should be valid (not generic)
        let title = clean_game_title("0.0.15c demo");
        assert_eq!(title, "0.0.15c demo");
        
        // From log: A Night Around The Fire_2022Update -> A Night Around The Fire 2022Update
        assert_eq!(clean_game_title("A Night Around The Fire_2022Update"), "A Night Around The Fire 2022Update");
        
        // From log: Abodtion -> Abodtion (should stay as is)
        assert_eq!(clean_game_title("Abodtion"), "Abodtion");
        
        // From log: ARLO4 with FPS Project.exe -> ARLO4 (dir name used because exe name doesn't match)
        // The dir name "ARLO4" is valid
        assert_eq!(clean_game_title("ARLO4"), "ARLO4");
        
        // From log: Bridgebourn Demo Win64 v0-6-29 -> Bridgebourn Demo Win64
        assert_eq!(clean_game_title("Bridgebourn Demo Win64 v0-6-29"), "Bridgebourn Demo Win64");
        
        // From log: COOKnRUN_1.1 -> COOKnRUN
        assert_eq!(clean_game_title("COOKnRUN_1.1"), "COOKnRUN");
        
        // From log: Dangeon_11b with readme name "DANGEON!" -> DANGEON!
        // This tests that metadata extraction works
        let metadata = Some(LocalMetadata {
            name: Some("DANGEON!".to_string()),
            description: Some("made by Friedrich Hanisch - www.ratking.de Music b".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, Some("DANGEON!".to_string()));
        
        // From log: Dash & Blast with readme "This demo is purely to showcase the core gameplay."
        // This is a sentence, should be rejected
        let metadata = Some(LocalMetadata {
            name: None,
            description: Some("This demo is purely to showcase the core gameplay.".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: Deadly Boat (new build) with readme "Control: WASD - Movement"
        // This is a sentence/controls description, should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("Control:".to_string()),
            description: Some("WASD - Movement Shift - Dash Space - power attack ".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: Elysis Demo 3b with readme "To play the game you must extract the folder"
        // This is a sentence, should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("To play the game you must extract the folder".to_string()),
            description: Some("Otherwise you may get maps that don't load You can".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: egg_windows_v11 with readme "----------------------------------------------------------------------------------"
        // This is not a valid name (all punctuation), should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("----------------------------------------------------------------------------------".to_string()),
            description: Some("EGG v1.1, 2nd November 2025".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: gauntlet-of-power-win with broken JSON name "{"
        // This is not a valid name (single punctuation), should be rejected
        let metadata = Some(LocalMetadata {
            name: Some("{".to_string()),
            description: Some("\"classPath\": [ \"heroesofloot3demo.dat\" ], \"mainCla".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: HeavyRecoil with about.txt "-------- Controls"
        let metadata = Some(LocalMetadata {
            name: None,
            description: Some("-------- Controls".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: Hive_Preserver_Jam_v10b with readme "It is a fast-paced first-person alien action game "
        // This is a sentence, should be rejected
        let metadata = Some(LocalMetadata {
            name: None,
            description: Some("It is a fast-paced first-person alien action game ".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        assert_eq!(title, None);
        
        // From log: Infineural with good readme name
        let metadata = Some(LocalMetadata {
            name: Some("Thank you so much for purchasing \"Infineural\".".to_string()),
            description: Some("If you have any issues regarding the game, please ".to_string()),
            developer: None,
            version: None,
        });
        let title = try_extract_from_local_metadata(&metadata);
        // This starts with "Thank you" which is a sentence indicator, should be rejected
        assert_eq!(title, None);
    }

    #[test]
    fn test_exe_metadata_extraction() {
        // Test with valid product name
        let exe_metadata = Some(ExeMetadata {
            product_name: Some("Roguelike".to_string()),
            company_name: Some("Some Company".to_string()),
            file_description: None,
            file_version: None,
        });
        let game_path = Path::new("/games/Roguelike");
        let title = try_extract_from_exe_metadata(game_path, &exe_metadata);
        assert_eq!(title, Some("Roguelike".to_string()));
        
        // Test with generic product name
        let exe_metadata = Some(ExeMetadata {
            product_name: Some("Unity Player".to_string()),
            company_name: Some("Unity Technologies".to_string()),
            file_description: None,
            file_version: None,
        });
        let game_path = Path::new("/games/MyGame");
        let title = try_extract_from_exe_metadata(&game_path, &exe_metadata);
        assert_eq!(title, None);
        
        // Test with exe in deep subfolder (Engine/Binaries) - should not use metadata
        let exe_metadata = Some(ExeMetadata {
            product_name: Some("MyGame".to_string()),
            company_name: Some("MyCompany".to_string()),
            file_description: None,
            file_version: None,
        });
        let game_path = Path::new("/games/MyGame/Engine/Binaries/Win64");
        let title = try_extract_from_exe_metadata(game_path, &exe_metadata);
        assert_eq!(title, None);
        
        // Test with exe in Plugins folder - should not use metadata
        let game_path = Path::new("/games/MyGame/Data/Plugins/x86_64");
        let title = try_extract_from_exe_metadata(game_path, &exe_metadata);
        assert_eq!(title, None);
    }

    #[test]
    fn test_company_name_extraction() {
        // Test with valid company name (as last resort)
        let exe_metadata = Some(ExeMetadata {
            product_name: Some("BootstrapPackagedGame".to_string()),
            company_name: Some("Valve".to_string()),
            file_description: None,
            file_version: None,
        });
        let title = try_extract_from_company_name(&exe_metadata);
        assert_eq!(title, Some("Valve".to_string()));
        
        // Test with generic company name
        let exe_metadata = Some(ExeMetadata {
            product_name: Some("MyGame".to_string()),
            company_name: Some("Microsoft Corporation".to_string()),
            file_description: None,
            file_version: None,
        });
        let title = try_extract_from_company_name(&exe_metadata);
        assert_eq!(title, None);
        
        // Test with None
        let exe_metadata: Option<ExeMetadata> = None;
        let title = try_extract_from_company_name(&exe_metadata);
        assert_eq!(title, None);
    }

    #[test]
    fn test_find_title_in_parents() {
        // Test finding title in immediate parent
        let path = Path::new("/games/MyGame/Data");
        let title = find_title_in_parents(path, 3);
        assert_eq!(title, Some("MyGame".to_string()));
        
        // Test with generic parent name, should skip to next
        let path = Path::new("/games/MyCollection/Windows/Data");
        let title = find_title_in_parents(path, 3);
        assert_eq!(title, Some("MyCollection".to_string()));
        
        // Test with multiple generic levels
        let path = Path::new("/games/Collection/Build/Windows/Data");
        let title = find_title_in_parents(path, 3);
        // Should find "Collection" (skipping "Windows" and "Build" if they're generic)
        // But "Collection" itself is NOT generic (we removed it from generic list)
        assert_eq!(title, Some("Collection".to_string()));
        
        // Test with all generic parents, should return None
        let path = Path::new("/games/Windows/Build/Release/Data");
        let title = find_title_in_parents(path, 3);
        assert_eq!(title, None);
    }
}
