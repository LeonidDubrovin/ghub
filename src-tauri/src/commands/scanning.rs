use crate::models::{ScannedGame, ExeMetadata};
use crate::AppState;
use tauri::State;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::fs;
use regex::Regex;
use lazy_static::lazy_static;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use log::{debug, info, warn, error};

/// Configuration for game scanning behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Maximum depth for recursive directory scanning
    pub max_scan_depth: usize,
    
    /// Maximum depth for searching executables within a game folder
    pub max_exe_search_depth: usize,
    
    /// Maximum number of cover candidates to return
    pub max_cover_candidates: usize,
    
    /// Whether to scan for local metadata files
    pub scan_local_metadata: bool,
    
    /// Whether to extract exe metadata (Windows only)
    pub extract_exe_metadata: bool,
    
    /// Additional exe exclusion patterns (regex)
    pub extra_exe_exclusions: Vec<String>,
    
    /// Additional folder exclusion patterns (regex)
    pub extra_folder_exclusions: Vec<String>,
    
    /// Additional image extensions to search for
    pub extra_image_extensions: Vec<String>,
    
    /// Additional metadata file names to search for
    pub extra_metadata_files: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_scan_depth: 5,
            max_exe_search_depth: 4,
            max_cover_candidates: 15,
            scan_local_metadata: true,
            extract_exe_metadata: true,
            extra_exe_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            extra_image_extensions: Vec::new(),
            extra_metadata_files: Vec::new(),
        }
    }
}

// Image extensions to look for covers/icons
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ico", "bmp", "webp", "gif"];

// Maximum depth for recursive scanning
const MAX_SCAN_DEPTH: usize = 5;

// Regex patterns for exe exclusion (more maintainable than string lists)
lazy_static! {
    static ref EXE_EXCLUSION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)unins\d*").unwrap(),
        Regex::new(r"(?i)^setup").unwrap(),
        Regex::new(r"(?i)^install").unwrap(),
        Regex::new(r"(?i)vc_redist\.(x64|x86)").unwrap(),
        Regex::new(r"(?i)dxsetup").unwrap(),
        Regex::new(r"(?i)directx").unwrap(),
        Regex::new(r"(?i)dotnet").unwrap(),
        Regex::new(r"(?i)crashreport").unwrap(),
        Regex::new(r"(?i)crash\s*handler").unwrap(),
        Regex::new(r"(?i)launcher$").unwrap(),
        Regex::new(r"(?i)updater$").unwrap(),
        Regex::new(r"(?i)ue4prereq").unwrap(),
        Regex::new(r"(?i)physx").unwrap(),
        Regex::new(r"(?i)steamcmd").unwrap(),
        Regex::new(r"(?i)easyanticheat").unwrap(),
        Regex::new(r"(?i)battleye").unwrap(),
        Regex::new(r"(?i)^notification_helper\.exe$").unwrap(),
        Regex::new(r"(?i)^unitycrashhandler(32|64)\.exe$").unwrap(),
        Regex::new(r"(?i)^python(w)?\.exe$").unwrap(),
        Regex::new(r"(?i)^zsync(make)?\.exe$").unwrap(),
    ];
    
    static ref FOLDER_EXCLUSION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)^(engine|redist|redistributables)$").unwrap(),
        Regex::new(r"(?i)^(directx|dotnet|vcredist|physx)$").unwrap(),
        Regex::new(r"(?i)^(prereqs?|prerequisites|support)$").unwrap(),
        Regex::new(r"(?i)^(commonredist|installer|install|setup)$").unwrap(),
        Regex::new(r"(?i)^(update|patch(es)?|backup)$").unwrap(),
        Regex::new(r"(?i)^(temp|tmp|cache|logs)$").unwrap(),
        Regex::new(r"(?i)^(saves?|screenshots?|mods?|plugins?)$").unwrap(),
        Regex::new(r"(?i)^binaries$").unwrap(),
        Regex::new(r"(?i)^__pycache__$").unwrap(),
        Regex::new(r"(?i)^\.git$").unwrap(),
    ];
}

// Local metadata files to look for
const METADATA_FILES: &[&str] = &[
    "game.json", "info.json", "metadata.json", "gameinfo.json",
    "game.yaml", "game.yml", "info.yaml", "info.yml", "metadata.yaml", "metadata.yml",
    "game.toml", "info.toml", "metadata.toml",
    "game.xml", "info.xml", "metadata.xml",
    "info.txt", "readme.txt", "README.md", "README.txt",
    "about.txt", "description.txt", "game_info.txt",
    "manifest.json", "package.json", "config.json",
    "UnityManifest.json", "ProjectSettings.asset",
    "DefaultGame.ini", "Game.ini", "config.ini"
];

#[tauri::command]
pub fn scan_space_sources(state: State<AppState>, space_id: String) -> Result<Vec<ScannedGame>, String> {
    debug!("scan_space_sources called with space_id: {}", space_id);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_active_sources_for_space(&space_id).map_err(|e| e.to_string())?;
    
    debug!("Found {} active source(s) for space {}", sources.len(), space_id);
    for (sp_id, source_path) in &sources {
        debug!("Source: {} -> {}", sp_id, source_path);
        let path = Path::new(source_path);
        debug!("Exists? {}", path.exists());
    }

    if sources.is_empty() {
        warn!("No active sources found - returning empty vector");
        return Ok(vec![]);
    }

    let mut all_games: Vec<ScannedGame> = Vec::new();

    for (_, source_path) in sources {
        debug!("Processing source: {}", source_path);
        let path = Path::new(&source_path);
        
        if !path.exists() {
            warn!("Source path does not exist: {}", source_path);
            continue;
        }
        
        debug!("Scanning directory: {}", source_path);
        match scan_directory_internal(path) {
            Ok(mut games) => {
                info!("Found {} games in {}", games.len(), source_path);
                for game in &games {
                    debug!("Game '{}' (path: {})", game.title, game.path);
                    if let Some(exe) = &game.executable {
                        debug!("Executable: {}", exe);
                    }
                }
                all_games.append(&mut games);
            }
            Err(e) => {
                error!("Scan error in {}: {}", source_path, e);
            }
        }
    }

    // Deduplicate by path
    all_games.sort_by(|a, b| a.path.cmp(&b.path));
    all_games.dedup_by(|a, b| a.path == b.path);

    info!("Total unique games found: {}", all_games.len());
    Ok(all_games)
}

#[tauri::command]
pub fn scan_directory(path: String) -> Result<Vec<ScannedGame>, String> {
    debug!("scan_directory called with path: {}", path);
    let base_path = Path::new(&path);

    if !base_path.exists() {
        return Err("Directory does not exist".to_string());
    }

    let result = scan_directory_internal(base_path).map_err(|e| e.to_string());
    debug!("scan_directory result: {:?}", result.as_ref().map(|games| games.len()));
    result
}

/// Extract game title from directory name with cleaning
fn extract_game_title(dir_name: &str) -> String {
    let mut title = dir_name.to_string();
    
    // Remove common prefixes/suffixes
    let prefixes = ["the ", "a ", "an "];
    let suffixes = [
        " (windows)", " (pc)", " (steam)", " (gog)", " (epic)",
        " - windows", " - pc", " - steam", " - gog", " - epic",
        " [windows]", " [pc]", " [steam]", " [gog]", " [epic]",
        " v1", " v2", " v3", " v4", " v5",
        " version 1", " version 2", " version 3",
    ];
    
    let lower_title = title.to_lowercase();
    
    // Remove prefixes
    for prefix in &prefixes {
        if lower_title.starts_with(prefix) {
            title = title[prefix.len()..].to_string();
            break;
        }
    }
    
    // Remove suffixes
    let lower_title = title.to_lowercase();
    for suffix in &suffixes {
        if lower_title.ends_with(suffix) {
            title = title[..title.len() - suffix.len()].to_string();
            break;
        }
    }
    
    // Clean up extra spaces and trim
    title = title.split_whitespace().collect::<Vec<&str>>().join(" ");
    title.trim().to_string()
}

/// Internal scan function that doesn't require a full path string
pub fn scan_directory_internal(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    scan_directory_internal_with_config(base_path, &ScanConfig::default())
}

/// Internal scan function with custom configuration
pub fn scan_directory_internal_with_config(base_path: &Path, config: &ScanConfig) -> Result<Vec<ScannedGame>, String> {
    debug!("[scan_directory_internal] base_path: {}", base_path.display());
    
    let mut games: Vec<ScannedGame> = Vec::new();
    // Use String paths for deduplication to avoid PathBuf normalization issues
    let mut scanned_paths = std::collections::HashSet::new();

    // Use recursive scanning with depth limit (similar to Playnite approach)
    for entry in WalkDir::new(base_path)
        .max_depth(config.max_scan_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        // Only process directories
        if !path.is_dir() {
            continue;
        }
        
        // Skip if already scanned (use string representation for consistent comparison)
        let path_str = path.to_string_lossy().to_string();
        if scanned_paths.contains(&path_str) {
            continue;
        }
        
        // Skip non-game folders using regex patterns
        let dir_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if is_folder_excluded(&dir_name) {
            debug!("[scan] Skipping excluded folder: {}", path.display());
            continue;
        }
        
        // Check if this directory contains executables
        if !has_executable_files(path) {
            continue;
        }
        
        debug!("[scan] Found game folder: {}", path.display());
        scanned_paths.insert(path_str);
        
        // Check if folder has only one subfolder and no exe - dive deeper
        let game_path = find_actual_game_folder(&path);
        debug!("[scan] Game folder resolved to: {}", game_path.display());

        let dir_name = game_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        // Extract and clean game title
        let title = extract_game_title(&dir_name);
        
        // Find ALL executables
        let all_executables = find_all_executables_with_config(&game_path, config);
        debug!("[scan] Found {} executables", all_executables.len());
        for exe in &all_executables {
            debug!("   - {}", exe);
        }

        // Pick the best executable
        let executable = pick_best_executable(&game_path, &all_executables);
        if let Some(exe) = &executable {
            debug!("[scan] Selected executable: {}", exe);
        } else {
            debug!("[scan] No suitable executable selected");
        }

        // Find cover/icon candidates
        let cover_candidates = find_cover_candidates_with_config(&game_path, config);
        if !cover_candidates.is_empty() {
            debug!("[scan] Found {} cover candidates", cover_candidates.len());
        }

        // Find icon
        let icon_path = find_icon(&game_path);
        if let Some(icon) = &icon_path {
            debug!("[scan] Found icon: {}", icon);
        }

        // Calculate folder size
        let size_bytes = calculate_dir_size(&game_path);
        debug!("[scan] Folder size: {} bytes", size_bytes);

        // Extract exe metadata (if we have an executable)
        let exe_metadata = executable.as_ref()
            .and_then(|exe| extract_exe_metadata(&game_path.join(exe)));
        if let Some(meta) = &exe_metadata {
            debug!("[scan] Exe metadata: product='{}', company='{}'",
                meta.product_name.as_deref().unwrap_or("n/a"),
                meta.company_name.as_deref().unwrap_or("n/a"));
        }

        // Try to read local metadata files
        let local_metadata = read_local_metadata(&game_path);
        
        // Enhanced title extraction with multi-level fallback
        // Priority: Local metadata > Folder name > EXE metadata (if not generic) > Parent folder > Executable name
        // This matches Playnite's approach: folder/shortcut name is primary, exe metadata is secondary
        let title = {
            // Level 0: Try local metadata file (game.json, info.txt, etc.)
            if let Some(ref meta) = local_metadata {
                if let Some(ref name) = meta.name {
                    if !name.is_empty() && !is_generic_exe_name(name) && !is_problematic_game_name(name) {
                        debug!("[title] Using local metadata name: '{}'", name);
                        name.clone()
                    } else {
                        // Continue to next level
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        };
        
        let title = if title.is_empty() {
            // Level 1: Try cleaned directory name (PRIMARY - like Playnite uses shortcut/folder names)
            if let Some(cleaned) = get_non_empty_title(clean_game_title(&dir_name)) {
                debug!("[title] Using cleaned dir name: '{}'", cleaned);
                cleaned
            } else {
                String::new()
            }
        } else {
            title
        };
        
        let title = if title.is_empty() {
            // Level 2: Try metadata product name (SECONDARY - only if not generic and exe is in reasonable location)
            let game_path_str = game_path.to_string_lossy();
            let exe_in_deep_subfolder = game_path_str.contains("Engine\\Binaries") ||
                                        game_path_str.contains("Engine/Binaries") ||
                                        game_path_str.contains("Plugins") ||
                                        game_path_str.contains("Binaries\\Win64") ||
                                        game_path_str.contains("Binaries/Win64");
            
            if let Some(meta_name) = exe_metadata.as_ref()
                .and_then(|m| m.product_name.clone())
                .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
            {
                if exe_in_deep_subfolder {
                    debug!("[title] Skipping metadata product name '{}' (exe in deep subfolder)", meta_name);
                    // Skip to next level
                    String::new()
                } else {
                    debug!("[title] Using metadata product name: '{}'", meta_name);
                    meta_name
                }
            } else {
                String::new()
            }
        } else {
            title
        };
        
        let title = if title.is_empty() {
            // Level 3: Try parent directory (up to 3 levels up)
            if let Some(parent_title) = find_title_in_parents(&game_path, 3) {
                debug!("[title] Using parent dir: '{}'", parent_title);
                parent_title
            }
            // Level 4: Try to extract from best executable name
            else if let Some(exe_name) = extract_title_from_executable(&executable) {
                debug!("[title] Using executable name: '{}'", exe_name);
                exe_name
            }
            // Level 5: Try metadata company name as last resort (only if not generic)
            else if let Some(company) = exe_metadata.as_ref()
                .and_then(|m| m.company_name.clone())
                .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
            {
                debug!("[title] Using company name: '{}'", company);
                company
            }
            // Final fallback: use original dir name or "Unknown Game"
            else {
                let fallback = if dir_name != "Unknown" { dir_name.clone() } else { "Unknown Game".to_string() };
                debug!("[title] Using fallback: '{}'", fallback);
                fallback
            }
        } else {
            title
        };
        debug!("[scan] Final title: '{}'", title);

        games.push(ScannedGame {
            path: game_path.to_string_lossy().to_string(),
            title,
            executable,
            all_executables,
            size_bytes,
            icon_path,
            cover_candidates,
            exe_metadata,
        });
    }

    Ok(games)
}

/// Find the actual game folder - if no exe in current folder, search subdirectories
fn find_actual_game_folder(dir: &Path) -> PathBuf {
    // First check if current directory has exe files
    if has_exe_files(dir) {
        debug!("[find_actual_game_folder] {} has exe directly, using it", dir.display());
        return dir.to_path_buf();
    }

    // No exe in current folder - search in subdirectories (up to 2 levels deep)
    // But avoid diving into Engine, Redist, and other non-game folders
    if let Some(found) = find_folder_with_exe(dir, 2) {
        debug!("[find_actual_game_folder] Found exe in subfolder: {}", found.display());
        return found;
    }

    debug!("[find_actual_game_folder] No exe found, returning original dir: {}", dir.display());
    dir.to_path_buf()
}

/// Check if folder name matches exclusion patterns
fn is_folder_excluded(dir_name: &str) -> bool {
    FOLDER_EXCLUSION_PATTERNS.iter().any(|pattern| pattern.is_match(dir_name))
}

/// Check if exe name matches exclusion patterns
fn is_exe_excluded(exe_name: &str) -> bool {
    EXE_EXCLUSION_PATTERNS.iter().any(|pattern| pattern.is_match(exe_name))
}

/// Check if directory contains any executable files (.exe or .lnk)
fn has_executable_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|entry| {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                        return ext_str == "exe" || ext_str == "lnk";
                    }
                }
                false
            })
        })
        .unwrap_or(false)
}

/// Check if directory contains any exe files (not in subdirs)
fn has_exe_files(dir: &Path) -> bool {
    let result = std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|entry| {
                let path = entry.path();
                path.is_file() && path.extension().map(|ext| ext.eq_ignore_ascii_case("exe")).unwrap_or(false)
            })
        })
        .unwrap_or(false);
    
    if result {
        debug!("[has_exe_files] {} contains exe files", dir.display());
    }
    result
}

/// Recursively find a subfolder that contains exe files
fn find_folder_with_exe(dir: &Path, max_depth: u32) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }

    let entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    // Check each subfolder
    for entry in &entries {
        let subdir = entry.path();
        
        // Skip non-game folders using regex patterns
        let dir_name = subdir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if is_folder_excluded(&dir_name) {
            debug!("[find_folder_with_exe] Skipping non-game folder: {}", subdir.display());
            continue;
        }
        
        if has_exe_files(&subdir) {
            return Some(subdir);
        }
    }

    // If no direct subfolder has exe, search deeper
    for entry in &entries {
        let subdir = entry.path();
        
        // Skip non-game folders using regex patterns
        let dir_name = subdir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if is_folder_excluded(&dir_name) {
            debug!("[find_folder_with_exe] Skipping non-game folder: {}", subdir.display());
            continue;
        }
        
        if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1) {
            return Some(found);
        }
    }

    None
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables(dir: &Path) -> Vec<String> {
    find_all_executables_with_config(dir, &ScanConfig::default())
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables_with_config(dir: &Path, config: &ScanConfig) -> Vec<String> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(dir).max_depth(config.max_exe_search_depth).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                
                // Support .exe files and .lnk files (Windows shortcuts)
                if ext_str == "exe" || ext_str == "lnk" {
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let name_lower = name.to_lowercase();

                    // Skip known non-game executables using regex patterns
                    let should_skip = is_exe_excluded(&name_lower);

                    if !should_skip && !name.is_empty() {
                        // Store relative path from game dir
                        let relative = path.strip_prefix(dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or(name);
                        executables.push(relative);
                    }
                }
            }
        }
    }

    executables.sort();
    executables.dedup();
    executables
}

/// Pick the best executable from the list
fn pick_best_executable(dir: &Path, executables: &[String]) -> Option<String> {
    if executables.is_empty() {
        return None;
    }

    let dir_name = dir.file_name()?.to_str()?.to_lowercase();

    // Priority 1: exe with same name as folder
    for exe in executables {
        let exe_stem = Path::new(exe)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if exe_stem == dir_name || dir_name.contains(&exe_stem) || exe_stem.contains(&dir_name) {
            debug!("[pick_best] Priority 1 match: '{}' matches folder '{}'", exe, dir_name);
            return Some(exe.clone());
        }
    }

    // Priority 2: exe in root folder (not subdir)
    for exe in executables {
        if !exe.contains('\\') && !exe.contains('/') {
            debug!("[pick_best] Priority 2 match: '{}' is in root", exe);
            return Some(exe.clone());
        }
    }

    // Priority 3: largest exe file (likely main game)
    let mut best: Option<(String, u64)> = None;
    for exe in executables {
        let full_path = dir.join(exe);
        if let Ok(meta) = std::fs::metadata(&full_path) {
            let size = meta.len();
            if best.is_none() || size > best.as_ref().unwrap().1 {
                best = Some((exe.clone(), size));
            }
        }
    }

    if let Some((exe, size)) = &best {
        debug!("[pick_best] Priority 3: selected largest '{}' ({} bytes)", exe, size);
    }
    best.map(|(exe, _)| exe)
}

/// Find potential cover/icon images
fn find_cover_candidates(dir: &Path) -> Vec<String> {
    find_cover_candidates_with_config(dir, &ScanConfig::default())
}

/// Find potential cover/icon images with custom configuration
fn find_cover_candidates_with_config(dir: &Path, config: &ScanConfig) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Search in root and common subdirs
    let search_paths = [
        dir.to_path_buf(),
        dir.join("images"),
        dir.join("image"),
        dir.join("img"),
        dir.join("art"),
        dir.join("assets"),
        dir.join("media"),
        dir.join("resources"),
        dir.join("gfx"),
        dir.join("graphics"),
        dir.join("covers"),
        dir.join("cover"),
        dir.join("box"),
        dir.join("boxart"),
        dir.join("screenshots"),
        dir.join("screenshot"),
        dir.join("promo"),
    ];

    for search_path in &search_paths {
        if !search_path.exists() {
            continue;
        }

        // Search recursively up to 3 levels deep for images
        for entry in WalkDir::new(search_path).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Prioritize files with cover-like names
            let cover_keywords = ["cover", "poster", "banner", "icon", "logo", "header", "art", "thumb", "image",
                "box", "front", "back", "screenshot", "promo", "keyart", "key_art", "key-art",
                "capsule", "library", "hero", "background", "bg", "wallpaper", "tile"];
            let is_cover_like = cover_keywords.iter().any(|kw| name.contains(kw));

            let relative = path.strip_prefix(dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());

            // Use HashSet for O(1) duplicate detection
            if seen.insert(relative.clone()) {
                if is_cover_like {
                    candidates.insert(0, relative);
                } else {
                    candidates.push(relative);
                }
            }
        }
    }

    // Limit to configured number of candidates
    candidates.truncate(config.max_cover_candidates);
    candidates
}

/// Find icon file
fn find_icon(dir: &Path) -> Option<String> {
    // Look for .ico files first
    for entry in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_str() == Some("ico") {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Look for icon.png
    let icon_png = dir.join("icon.png");
    if icon_png.exists() {
        return Some(icon_png.to_string_lossy().to_string());
    }

    None
}

/// Extract metadata from exe file (Windows only)
#[cfg(target_os = "windows")]
fn extract_exe_metadata(exe_path: &Path) -> Option<ExeMetadata> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW,
    };

    if !exe_path.exists() {
        return None;
    }

    // Convert path to wide string
    let path_wide: Vec<u16> = OsStr::new(exe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let size = GetFileVersionInfoSizeW(path_wide.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        if GetFileVersionInfoW(path_wide.as_ptr(), 0, size, buffer.as_mut_ptr() as *mut _) == 0 {
            return None;
        }

        // Query for StringFileInfo
        let mut metadata = ExeMetadata {
            product_name: None,
            company_name: None,
            file_description: None,
            file_version: None,
        };

        // Try to get all available version info fields
        metadata.product_name = query_version_string(&buffer, "ProductName");
        metadata.company_name = query_version_string(&buffer, "CompanyName");
        metadata.file_description = query_version_string(&buffer, "FileDescription");
        metadata.file_version = query_version_string(&buffer, "FileVersion");
        
        // Additional fields that might be useful - store them for future use
        let _product_version = query_version_string(&buffer, "ProductVersion");
        let _legal_copyright = query_version_string(&buffer, "LegalCopyright");
        let _original_filename = query_version_string(&buffer, "OriginalFilename");
        let _internal_name = query_version_string(&buffer, "InternalName");
        let _comments = query_version_string(&buffer, "Comments");

        // If we have a product name but it's generic, try to use internal name or original filename
        if metadata.product_name.is_some() {
            let product_name = metadata.product_name.as_ref().unwrap();
            if is_generic_exe_name(product_name) {
                // Try internal name first
                if let Some(internal_name) = _internal_name {
                    if !is_generic_exe_name(&internal_name) && !is_problematic_game_name(&internal_name) {
                        metadata.product_name = Some(internal_name);
                    }
                }
                // If still generic, try original filename
                if metadata.product_name.as_ref().map_or(true, |p| is_generic_exe_name(p)) {
                    if let Some(original_filename) = _original_filename {
                        let cleaned = clean_game_title(&original_filename.replace(".exe", ""));
                        if !cleaned.is_empty() && !is_generic_exe_name(&cleaned) {
                            metadata.product_name = Some(cleaned);
                        }
                    }
                }
            }
        }

        if metadata.product_name.is_some() || metadata.company_name.is_some() {
            Some(metadata)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_exe_metadata(_exe_path: &Path) -> Option<ExeMetadata> {
    None
}

fn query_version_string(buffer: &[u8], name: &str) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

    // Common language/codepage combinations
    let lang_codepages = [
        "040904B0", // US English, Unicode
        "040904E4", // US English, Multilingual
        "000004B0", // Neutral, Unicode
        "040904E4", // US English, Western European
    ];

    for lc in &lang_codepages {
        let query = format!("\\StringFileInfo\\{}\\{}", lc, name);
        let query_wide: Vec<u16> = OsStr::new(&query)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let mut ptr: *mut u16 = std::ptr::null_mut();
            let mut len: u32 = 0;

            if VerQueryValueW(
                buffer.as_ptr() as *const _,
                query_wide.as_ptr(),
                &mut ptr as *mut _ as *mut *mut _,
                &mut len,
            ) != 0 && len > 0 {
                let slice = std::slice::from_raw_parts(ptr, len as usize);
                // Find null terminator
                let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
                let result = String::from_utf16_lossy(&slice[..end]);
                if !result.is_empty() {
                    return Some(result);
                }
            }
        }
    }

    None
}

fn calculate_dir_size(dir: &Path) -> u64 {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Check if an exe product name is generic and shouldn't be used as game title
fn is_generic_exe_name(name: &str) -> bool {
    // Only filter out truly generic names that don't identify a specific game
    // Use exact matches to avoid false positives (e.g., "Unity of Command" should not be filtered)
    let generic_names = [
        "Godot Engine", "BootstrapPackagedGame", "Unity", "Unreal Engine",
        "UnrealEngine", "UE4", "UE5", "UE4Game",
        "Windows", "Launcher", "Setup", "Installer", "Updater",
        "CrashReport", "Crash Handler", "Unity Player", "UE4 Game",
        "Game Launcher", "Application", "App",
        "WindowsNoEditor", "Win64", "Win32", "Shipping",
        "Development", "Debug", "Release", "Player", "Runtime",
        "Redistributable", "Microsoft", "Visual", "C++", "DirectX",
        "OpenGL", "Vulkan", "XInput", "DInput", "PhysX", "NVIDIA",
        "AMD", "Intel", "Steam", "Epic", "GOG", "Origin", "Ubisoft",
        "EA", "Battle.net", "Rockstar", "Bethesda", "2K", "Sega",
        "Square Enix", "Capcom", "Konami", "Bandai Namco", "Activision",
        "Blizzard", "Microsoft Studios", "Xbox", "PlayStation", "Nintendo"
    ];
    
    let name_lower = name.to_lowercase();
    for generic in &generic_names {
        if name_lower == generic.to_lowercase() {
            return true;
        }
    }
    
    // Check if name is just version numbers (e.g., "1.0.0", "v2.0")
    if name.chars().all(|c| c.is_numeric() || c == '.' || c == '_' || c == '-' || c == 'v' || c == 'V') {
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
fn is_problematic_game_name(name: &str) -> bool {
    let problematic_names = [
        "ICARUS", "Life Makeover", "Microphage", "Godot Engine",
        "BootstrapPackagedGame", "WindowsNoEditor", "Win64", "Win32",
        "Shipping", "Development", "Debug", "Release"
    ];
    
    let name_lower = name.to_lowercase();
    for problematic in &problematic_names {
        if name_lower == problematic.to_lowercase() {
            return true;
        }
    }
    
    false
}

fn clean_game_title(name: &str) -> String {
    // Remove common suffixes/prefixes
    let mut title = name.to_string();

    // Remove version numbers like v1.0, 1.0.0, V1.1_NEW, v012, etc.
    let re_version = regex_lite::Regex::new(r"[\s_]*(?:[vV]\d+(?:[\._]\d+)*|\d+(?:[\._]\d+)+).*$").ok();
    if let Some(re) = re_version {
        title = re.replace(&title, "").to_string();
    }

    // Remove platform tags
    for tag in &["(Windows)", "(PC)", "(GOG)", "(Steam)", "[GOG]", "[Steam]", "(Mac)", "(Linux)", "_Windows", "_PC"] {
        title = title.replace(tag, "");
    }

    // Remove common generic folder names that shouldn't be game titles
    let generic_names = [
        "Windows", "BootstrapPackagedGame", "Godot Engine", "Unity", "Unreal",
        "Game", "Build", "Release", "Bin", "Binary", "Executable", "App",
        "win64", "win32", "linux", "macos", "x64", "x86", "WindowsNoEditor",
        "Win64", "Win32", "Shipping", "Development", "Debug"
    ];
    
    let trimmed = title.trim();
    for generic in &generic_names {
        if trimmed.eq_ignore_ascii_case(generic) {
            return String::new(); // Return empty to signal we should use parent dir
        }
    }

    // Clean up trailing/leading underscores and dashes
    title = title.trim_matches(|c: char| c == '_' || c == '-' || c == ' ').to_string();
    
    // Replace underscores with spaces for better readability
    title = title.replace('_', " ");
    
    // Remove multiple spaces
    let re_spaces = regex_lite::Regex::new(r"\s+").ok();
    if let Some(re) = re_spaces {
        title = re.replace_all(&title, " ").to_string();
    }

    title.trim().to_string()
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

/// Local metadata structure
#[derive(Debug, Clone)]
struct LocalMetadata {
    name: Option<String>,
    description: Option<String>,
    developer: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    release_date: Option<String>,
}

/// Helper function to extract metadata fields from a map-like structure
fn extract_metadata_fields<F>(get_field: F) -> Option<LocalMetadata>
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
    
    let publisher = get_field("publisher");
    let version = get_field("version").or_else(|| get_field("ver"));
    let release_date = get_field("release_date")
        .or_else(|| get_field("releaseDate"))
        .or_else(|| get_field("date"));

    if name.is_some() || description.is_some() {
        Some(LocalMetadata {
            name,
            description,
            developer,
            publisher,
            version,
            release_date,
        })
    } else {
        None
    }
}

/// Read local metadata files (game.json, info.txt, README.md, etc.)
fn read_local_metadata(dir: &Path) -> Option<LocalMetadata> {
    for filename in METADATA_FILES {
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
    let content = fs::read_to_string(file_path).ok()?;
    
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        let metadata = extract_metadata_fields(|key| {
            json.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
        });

        if let Some(ref meta) = metadata {
            debug!("[local_metadata] Parsed JSON: name={:?}, desc={:?}",
                meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
        }
        return metadata;
    }

    None
}

/// Read YAML metadata file
fn read_yaml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;
    
    let mut field_map = std::collections::HashMap::new();
    
    // Simple YAML parsing - look for key: value pairs
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_lowercase();
            let value = line[colon_pos + 1..].trim().trim_matches('"').trim_matches('\'');
            
            if !value.is_empty() {
                field_map.insert(key, value.to_string());
            }
        }
    }

    let metadata = extract_metadata_fields(|key| field_map.get(key).cloned());

    if let Some(ref meta) = metadata {
        debug!("[local_metadata] Parsed YAML: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read TOML metadata file
fn read_toml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;
    
    let mut field_map = std::collections::HashMap::new();
    
    // Simple TOML parsing - look for key = "value" pairs
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_lowercase();
            let value = line[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
            
            if !value.is_empty() {
                field_map.insert(key, value.to_string());
            }
        }
    }

    let metadata = extract_metadata_fields(|key| field_map.get(key).cloned());

    if let Some(ref meta) = metadata {
        debug!("[local_metadata] Parsed TOML: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read XML metadata file
fn read_xml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;
    
    let mut field_map = std::collections::HashMap::new();
    
    // Simple XML parsing - look for <tag>value</tag> patterns
    let tags = ["name", "title", "game_name", "description", "desc", "about",
                "developer", "dev", "author", "publisher", "version", "ver",
                "release_date", "releasedate", "date"];
    
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

    let metadata = extract_metadata_fields(|key| field_map.get(key).cloned());

    if let Some(ref meta) = metadata {
        debug!("[local_metadata] Parsed XML: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read INI metadata file
fn read_ini_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;
    
    let mut field_map = std::collections::HashMap::new();
    
    // Simple INI parsing - look for key=value pairs
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
            continue;
        }

        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_lowercase();
            let value = line[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
            
            if !value.is_empty() {
                field_map.insert(key, value.to_string());
            }
        }
    }

    let metadata = extract_metadata_fields(|key| field_map.get(key).cloned());

    if let Some(ref meta) = metadata {
        debug!("[local_metadata] Parsed INI: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read text metadata file (README, info.txt, etc.)
fn read_text_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;
    
    let mut metadata = LocalMetadata {
        name: None,
        description: None,
        developer: None,
        publisher: None,
        version: None,
        release_date: None,
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
        debug!("[local_metadata] Parsed text: name={:?}, desc={:?}",
            metadata.name, metadata.description.as_ref().map(|d| &d[..50.min(d.len())]));
        return Some(metadata);
    }

    None
}