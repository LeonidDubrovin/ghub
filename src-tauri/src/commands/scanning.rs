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
    
    /// Maximum depth for searching cover images
    pub max_cover_search_depth: usize,
    
    /// Whether to scan for local metadata files
    pub scan_local_metadata: bool,
    
    /// Whether to extract exe metadata (Windows only)
    pub extract_exe_metadata: bool,
    
    /// Base exe exclusion patterns (regex strings)
    pub base_exe_exclusions: Vec<String>,
    
    /// Additional exe exclusion patterns (regex)
    pub extra_exe_exclusions: Vec<String>,
    
    /// Base folder exclusion patterns (regex strings)
    pub base_folder_exclusions: Vec<String>,
    
    /// Additional folder exclusion patterns (regex)
    pub extra_folder_exclusions: Vec<String>,
    
    /// Base image extensions to search for
    pub base_image_extensions: Vec<String>,
    
    /// Additional image extensions to search for
    pub extra_image_extensions: Vec<String>,
    
    /// Base metadata file names to search for
    pub base_metadata_files: Vec<String>,
    
    /// Additional metadata file names to search for
    pub extra_metadata_files: Vec<String>,
    
    /// Cover search paths (subdirectories to search for covers)
    pub cover_search_paths: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_scan_depth: 5,
            max_exe_search_depth: 4,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            scan_local_metadata: true,
            extract_exe_metadata: true,
            base_exe_exclusions: vec![
                r"(?i)unins\d*".to_string(),
                r"(?i)^setup".to_string(),
                r"(?i)^install".to_string(),
                r"(?i)vc_redist\.(x64|x86)".to_string(),
                r"(?i)dxsetup".to_string(),
                r"(?i)directx".to_string(),
                r"(?i)dotnet".to_string(),
                r"(?i)crashreport".to_string(),
                r"(?i)crash\s*handler".to_string(),
                r"(?i)launcher$".to_string(),
                r"(?i)updater$".to_string(),
                r"(?i)ue4prereq".to_string(),
                r"(?i)physx".to_string(),
                r"(?i)steamcmd".to_string(),
                r"(?i)easyanticheat".to_string(),
                r"(?i)battleye".to_string(),
                r"(?i)^notification_helper\.exe$".to_string(),
                r"(?i)^unitycrashhandler(32|64)\.exe$".to_string(),
                r"(?i)^python(w)?\.exe$".to_string(),
                r"(?i)^zsync(make)?\.exe$".to_string(),
            ],
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: vec![
                r"(?i)^(engine|redist|redistributables)$".to_string(),
                r"(?i)^(directx|dotnet|vcredist|physx)$".to_string(),
                r"(?i)^(prereqs?|prerequisites|support)$".to_string(),
                r"(?i)^(commonredist|installer|install|setup)$".to_string(),
                r"(?i)^(update|patch(es)?|backup)$".to_string(),
                r"(?i)^(temp|tmp|cache|logs)$".to_string(),
                r"(?i)^(saves?|screenshots?|mods?|plugins?)$".to_string(),
                r"(?i)^binaries$".to_string(),
                r"(?i)^__pycache__$".to_string(),
                r"(?i)^\.git$".to_string(),
            ],
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "ico".to_string(),
                "bmp".to_string(),
                "webp".to_string(),
                "gif".to_string(),
            ],
            extra_image_extensions: Vec::new(),
            base_metadata_files: vec![
                "game.json".to_string(),
                "info.json".to_string(),
                "metadata.json".to_string(),
                "gameinfo.json".to_string(),
                "game.yaml".to_string(),
                "game.yml".to_string(),
                "info.yaml".to_string(),
                "info.yml".to_string(),
                "metadata.yaml".to_string(),
                "metadata.yml".to_string(),
                "game.toml".to_string(),
                "info.toml".to_string(),
                "metadata.toml".to_string(),
                "game.xml".to_string(),
                "info.xml".to_string(),
                "metadata.xml".to_string(),
                "info.txt".to_string(),
                "readme.txt".to_string(),
                "README.md".to_string(),
                "README.txt".to_string(),
                "about.txt".to_string(),
                "description.txt".to_string(),
                "game_info.txt".to_string(),
                "manifest.json".to_string(),
                "package.json".to_string(),
                "config.json".to_string(),
                "UnityManifest.json".to_string(),
                "ProjectSettings.asset".to_string(),
                "DefaultGame.ini".to_string(),
                "Game.ini".to_string(),
                "config.ini".to_string(),
            ],
            extra_metadata_files: Vec::new(),
            cover_search_paths: vec![
                "images".to_string(),
                "image".to_string(),
                "img".to_string(),
                "art".to_string(),
                "assets".to_string(),
                "media".to_string(),
                "resources".to_string(),
                "gfx".to_string(),
                "graphics".to_string(),
                "covers".to_string(),
                "cover".to_string(),
                "box".to_string(),
                "boxart".to_string(),
                "screenshots".to_string(),
                "screenshot".to_string(),
                "promo".to_string(),
            ],
        }
    }
}

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

    // Deduplicate by normalized path
    all_games.sort_by(|a, b| {
        let norm_a = normalize_path(Path::new(&a.path));
        let norm_b = normalize_path(Path::new(&b.path));
        norm_a.cmp(&norm_b)
    });
    all_games.dedup_by(|a, b| {
        let norm_a = normalize_path(Path::new(&a.path));
        let norm_b = normalize_path(Path::new(&b.path));
        norm_a == norm_b
    });

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

/// Normalize a path string for consistent comparison
fn normalize_path(path: &Path) -> String {
    // Convert to canonical path if possible, otherwise use string representation
    path.canonicalize()
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
        .trim_end_matches(['\\', '/'])
        .to_string()
}

/// Compile regex patterns from string slices
fn compile_patterns<'a>(base: &[String], extra: &[String]) -> Vec<Regex> {
    let mut patterns = Vec::new();
    
    // Add base patterns
    for pattern_str in base {
        if let Ok(re) = Regex::new(pattern_str) {
            patterns.push(re);
        } else {
            warn!("Invalid regex pattern: {}", pattern_str);
        }
    }
    
    // Add extra patterns
    for pattern_str in extra {
        if let Ok(re) = Regex::new(pattern_str) {
            patterns.push(re);
        } else {
            warn!("Invalid regex pattern: {}", pattern_str);
        }
    }
    
    patterns
}

/// Internal scan function with custom configuration
pub fn scan_directory_internal_with_config(base_path: &Path, config: &ScanConfig) -> Result<Vec<ScannedGame>, String> {
    debug!("[scan_directory_internal] base_path: {}", base_path.display());

    // Compile regex patterns from config (base + extra)
    let exe_patterns = compile_patterns(&config.base_exe_exclusions, &config.extra_exe_exclusions);
    let folder_patterns = compile_patterns(&config.base_folder_exclusions, &config.extra_folder_exclusions);
    
    // Combine metadata files and image extensions
    let all_metadata_files: Vec<String> = config.base_metadata_files.iter()
        .chain(&config.extra_metadata_files)
        .cloned()
        .collect();
    let all_image_extensions: Vec<String> = config.base_image_extensions.iter()
        .chain(&config.extra_image_extensions)
        .cloned()
        .collect();

    let mut games: Vec<ScannedGame> = Vec::new();
    // Use normalized paths for deduplication
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

        // Normalize path for consistent comparison
        let normalized_path = normalize_path(path);
        if scanned_paths.contains(&normalized_path) {
            continue;
        }
        
        // Skip non-game folders using regex patterns
        let dir_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if is_folder_excluded(&dir_name, &folder_patterns) {
            debug!("[scan] Skipping excluded folder: {}", path.display());
            continue;
        }
        
        // Check if this directory contains executables
        if !has_executable_files(path) {
            continue;
        }
        
        debug!("[scan] Found game folder: {}", path.display());
        scanned_paths.insert(normalized_path);
        
        // Check if folder has only one subfolder and no exe - dive deeper
        let game_path = find_actual_game_folder(&path, &folder_patterns);
        debug!("[scan] Game folder resolved to: {}", game_path.display());

        let dir_name = game_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        // Extract and clean game title
        let title = extract_game_title(&dir_name);
        
        // Find ALL executables
        let all_executables = find_all_executables_with_config(&game_path, config, &exe_patterns);
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
        let cover_candidates = find_cover_candidates_with_config(&game_path, config, &all_image_extensions);
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
        let local_metadata = read_local_metadata(&game_path, &all_metadata_files);

        // Enhanced title extraction with multi-level fallback
        // Priority: Local metadata > Folder name > EXE metadata (if not generic) > Parent folder > Executable name
        // This matches Playnite's approach: folder/shortcut name is primary, exe metadata is secondary
        let title = extract_title_with_fallback(
            &game_path,
            &dir_name,
            &local_metadata,
            &exe_metadata,
            &executable,
        );
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
fn find_actual_game_folder(dir: &Path, folder_patterns: &[Regex]) -> PathBuf {
    // First check if current directory has exe files
    if has_exe_files(dir) {
        debug!("[find_actual_game_folder] {} has exe directly, using it", dir.display());
        return dir.to_path_buf();
    }

    // No exe in current folder - search in subdirectories (up to 2 levels deep)
    // But avoid diving into Engine, Redist, and other non-game folders
    if let Some(found) = find_folder_with_exe(dir, 2, folder_patterns) {
        debug!("[find_actual_game_folder] Found exe in subfolder: {}", found.display());
        return found;
    }

    debug!("[find_actual_game_folder] No exe found, returning original dir: {}", dir.display());
    dir.to_path_buf()
}

/// Check if folder name matches exclusion patterns
fn is_folder_excluded(dir_name: &str, patterns: &[Regex]) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(dir_name))
}

/// Check if exe name matches exclusion patterns
fn is_exe_excluded(exe_name: &str, patterns: &[Regex]) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(exe_name))
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
fn find_folder_with_exe(dir: &Path, max_depth: u32, folder_patterns: &[Regex]) -> Option<PathBuf> {
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
        
        if is_folder_excluded(&dir_name, folder_patterns) {
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
        
        if is_folder_excluded(&dir_name, folder_patterns) {
            debug!("[find_folder_with_exe] Skipping non-game folder: {}", subdir.display());
            continue;
        }
        
        if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1, folder_patterns) {
            return Some(found);
        }
    }

    None
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables(dir: &Path) -> Vec<String> {
    let config = ScanConfig::default();
    let exe_patterns = compile_patterns(&config.base_exe_exclusions, &config.extra_exe_exclusions);
    find_all_executables_with_config(dir, &config, &exe_patterns)
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables_with_config(dir: &Path, config: &ScanConfig, exe_patterns: &[Regex]) -> Vec<String> {
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
                    let should_skip = is_exe_excluded(&name_lower, exe_patterns);

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
    let config = ScanConfig::default();
    let image_extensions: Vec<String> = config.base_image_extensions.iter()
        .chain(&config.extra_image_extensions)
        .cloned()
        .collect();
    find_cover_candidates_with_config(dir, &config, &image_extensions)
}

/// Find potential cover/icon images with custom configuration
fn find_cover_candidates_with_config(dir: &Path, config: &ScanConfig, image_extensions: &[String]) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Build search paths from config (always include root dir)
    let mut search_paths = vec![dir.to_path_buf()];
    for subdir in &config.cover_search_paths {
        search_paths.push(dir.join(subdir));
    }

    for search_path in &search_paths {
        if !search_path.exists() {
            continue;
        }

        // Search recursively up to configured depth for images
        for entry in WalkDir::new(search_path).max_depth(config.max_cover_search_depth).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            // Check if extension is in the configured list (case-insensitive)
            if !image_extensions.iter().any(|ext_ok| ext_ok.eq_ignore_ascii_case(&ext)) {
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
    // Use word-boundary matching to avoid false positives (e.g., "Unity of Command" should not be filtered)
    let generic_names = [
        "godot engine", "bootstrappackagedgame", "^(?:.*\\s)?unity(?:\\s|$)", "unreal engine",
        "unrealengine", "ue4", "ue5", "ue4game",
        "windows", "launcher", "setup", "installer", "updater",
        "crashreport", "crash handler", "unity player", "ue4 game",
        "game launcher", "application", "app",
        "windowsnoeditor", "win64", "win32", "shipping",
        "development", "debug", "release", "player", "runtime",
        "redistributable", "microsoft", "visual", "c\\+\\+", "directx",
        "opengl", "vulkan", "xinput", "dinput", "physx", "nvidia",
        "amd", "intel", "steam", "epic", "gog", "origin", "ubisoft",
        "ea", "battle\\.net", "rockstar", "bethesda", "2k", "sega",
        "square enix", "capcom", "konami", "bandai namco", "activision",
        "blizzard", "microsoft studios", "xbox", "playstation", "nintendo"
    ];

    let name_lower = name.to_lowercase();

    // Check exact matches first (fast path)
    let exact_generic = [
        "godot engine", "bootstrappackagedgame", "windows", "launcher", "setup",
        "installer", "updater", "crashreport", "crash handler", "unity player",
        "ue4 game", "game launcher", "application", "app", "windowsnoeditor",
        "win64", "win32", "shipping", "development", "debug", "release",
        "player", "runtime", "redistributable", "microsoft", "visual",
        "opengl", "vulkan", "xinput", "dinput", "physx", "nvidia",
        "amd", "intel", "steam", "epic", "gog", "origin", "ubisoft",
        "ea", "rockstar", "bethesda", "2k", "sega", "square enix",
        "capcom", "konami", "bandai namco", "activision", "blizzard",
        "microsoft studios", "xbox", "playstation", "nintendo"
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

/// Main title extraction with multi-level fallback strategy
fn extract_title_with_fallback(
    game_path: &Path,
    dir_name: &str,
    local_metadata: &Option<LocalMetadata>,
    exe_metadata: &Option<ExeMetadata>,
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
    let fallback = if dir_name != "Unknown" { dir_name.to_string() } else { "Unknown Game".to_string() };
    debug!("[title] Using fallback: '{}'", fallback);
    fallback
}

/// Level 0: Extract title from local metadata file
fn try_extract_from_local_metadata(metadata: &Option<LocalMetadata>) -> Option<String> {
    metadata.as_ref()
        .and_then(|m| m.name.as_ref())
        .filter(|name| !name.is_empty() && !is_generic_exe_name(name) && !is_problematic_game_name(name))
        .cloned()
}

/// Level 1: Extract title from cleaned directory name
fn try_extract_from_dir_name(dir_name: &str) -> Option<String> {
    get_non_empty_title(clean_game_title(dir_name))
}

/// Level 2: Extract title from exe metadata product name (if not in deep subfolder)
fn try_extract_from_exe_metadata(game_path: &Path, exe_metadata: &Option<ExeMetadata>) -> Option<String> {
    let path_str = game_path.to_string_lossy();
    let exe_in_deep_subfolder = path_str.contains("Engine\\Binaries") ||
                                path_str.contains("Engine/Binaries") ||
                                path_str.contains("Plugins") ||
                                path_str.contains("Binaries\\Win64") ||
                                path_str.contains("Binaries/Win64");

    exe_metadata.as_ref()
        .and_then(|m| m.product_name.clone())
        .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
        .filter(|_| !exe_in_deep_subfolder)
}

/// Level 5: Extract title from company name (as last resort)
fn try_extract_from_company_name(exe_metadata: &Option<ExeMetadata>) -> Option<String> {
    exe_metadata.as_ref()
        .and_then(|m| m.company_name.clone())
        .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
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

/// Read local metadata files (game.json, info.txt, README.md, etc.)
fn read_local_metadata(dir: &Path, metadata_files: &[String]) -> Option<LocalMetadata> {
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
    let content = fs::read_to_string(file_path).ok()?;

    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        let metadata = parse_key_value_file(|key| {
            json.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
        });

        if let Some(ref meta) = &metadata {
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

    let field_map = parse_key_value_pairs(&content, ':')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!("[local_metadata] Parsed YAML: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read TOML metadata file
fn read_toml_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;

    let field_map = parse_key_value_pairs(&content, '=')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
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

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!("[local_metadata] Parsed XML: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Read INI metadata file
fn read_ini_metadata(file_path: &Path) -> Option<LocalMetadata> {
    let content = fs::read_to_string(file_path).ok()?;

    let field_map = parse_key_value_pairs(&content, '=')?;

    let metadata = parse_key_value_file(|key| field_map.get(key).cloned());

    if let Some(ref meta) = &metadata {
        debug!("[local_metadata] Parsed INI: name={:?}, desc={:?}",
            meta.name, meta.description.as_ref().map(|d| &d[..50.min(d.len())]));
    }
    metadata
}

/// Generic function to parse key-value pairs from text formats (YAML, TOML, INI)
fn parse_key_value_pairs(content: &str, separator: char) -> Option<std::collections::HashMap<String, String>> {
    let mut field_map = std::collections::HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[') {
            continue;
        }

        if let Some(sep_pos) = line.find(separator) {
            let key = line[..sep_pos].trim().to_lowercase();
            let value = line[sep_pos + 1..].trim().trim_matches('"').trim_matches('\'');

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

    let publisher = get_field("publisher");
    let version = get_field("version").or_else(|| get_field("ver"));
    let release_date = get_field("release_date")
        .or_else(|| get_field("releasedate"))
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