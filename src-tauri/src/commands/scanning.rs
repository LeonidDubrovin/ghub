use crate::models::{ExeMetadata, ScannedGame};
use crate::scanner_constants;
use crate::title_extraction::{clean_game_title, is_generic_exe_name, is_problematic_game_name, read_local_metadata, extract_title_with_fallback};
use crate::AppState;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use walkdir::WalkDir;

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
            max_scan_depth: scanner_constants::MAX_SCAN_DEPTH,
            max_exe_search_depth: scanner_constants::MAX_EXE_SEARCH_DEPTH,
            max_cover_candidates: scanner_constants::MAX_COVER_CANDIDATES,
            max_cover_search_depth: scanner_constants::MAX_COVER_SEARCH_DEPTH,
            scan_local_metadata: true,
            extract_exe_metadata: true,
            base_exe_exclusions: scanner_constants::BASE_EXE_EXCLUSIONS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: scanner_constants::BASE_FOLDER_EXCLUSIONS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: scanner_constants::BASE_IMAGE_EXTENSIONS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_image_extensions: Vec::new(),
            base_metadata_files: scanner_constants::BASE_METADATA_FILES
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: scanner_constants::BASE_COVER_SEARCH_PATHS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
        }
    }
}

#[tauri::command]
pub fn scan_space_sources(
    state: State<AppState>,
    space_id: String,
) -> Result<Vec<ScannedGame>, String> {
    debug!("scan_space_sources called with space_id: {}", space_id);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db
        .get_active_sources_for_space(&space_id)
        .map_err(|e| e.to_string())?;

    debug!(
        "Found {} active source(s) for space {}",
        sources.len(),
        space_id
    );
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

        // Set scan status to scanning
        {
            if let Ok(db) = state.db.lock() {
                let _ = db.set_source_scan_status(
                    &space_id,
                    &source_path,
                    Some("scanning"),
                    Some(0),
                    None,
                    None,
                );
            }
        }

        if !path.exists() {
            warn!("Source path does not exist: {}", source_path);
            // Set error status
            if let Ok(db) = state.db.lock() {
                let _ = db.set_source_scan_status(
                    &space_id,
                    &source_path,
                    Some("error"),
                    None,
                    None,
                    Some("Directory does not exist"),
                );
            }
            continue;
        }

        debug!("Scanning directory: {}", source_path);
        let scan_result = scan_directory_internal(path);
        match scan_result {
            Ok(mut games) => {
                info!("Found {} games in {}", games.len(), source_path);
                for game in &games {
                    debug!("Game '{}' (path: {})", game.title, game.path);
                    if let Some(exe) = &game.executable {
                        debug!("Executable: {}", exe);
                    }
                }
                all_games.append(&mut games);
                // Set completed status
                if let Ok(db) = state.db.lock() {
                    let count = games.len() as i32;
                    let _ = db.set_source_scan_status(
                        &space_id,
                        &source_path,
                        Some("completed"),
                        Some(count),
                        Some(count),
                        None,
                    );
                }
            }
            Err(e) => {
                error!("Scan error in {}: {}", source_path, e);
                // Set error status
                if let Ok(db) = state.db.lock() {
                    let _ = db.set_source_scan_status(
                        &space_id,
                        &source_path,
                        Some("error"),
                        None,
                        None,
                        Some(&e.to_string()),
                    );
                }
            }
        }

        // Update last_scanned_at for this source
        if let Ok(db) = state.db.lock() {
            if let Err(e) = db.update_source_last_scanned(&space_id, &source_path) {
                error!("Failed to update last_scanned_at for {}: {}", source_path, e);
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
    debug!(
        "scan_directory result: {:?}",
        result.as_ref().map(|games| games.len())
    );
    result
}

/// Start scanning a source (directory) in background
#[tauri::command]
pub fn start_source_scan(state: State<AppState>, space_id: String, source_path: String) -> Result<(), String> {
    state.scanning_service.lock().map_err(|e| e.to_string())?.start_scan(
        state.db.clone(),
        space_id,
        source_path,
    )
}

/// Cancel a running scan
#[tauri::command]
pub fn cancel_source_scan(state: State<AppState>, space_id: String, source_path: String) -> Result<(), String> {
    state.scanning_service.lock().map_err(|e| e.to_string())?.cancel_scan(
        &state.db,
        &space_id,
        &source_path,
    )
}

/// Get scan status for a source
#[tauri::command]
pub fn get_source_scan_status(state: State<AppState>, space_id: String, source_path: String) -> Result<Option<SpaceSource>, String> {
    state.scanning_service.lock().map_err(|e| e.to_string())?.get_source_scan_status(
        &state.db,
        &space_id,
        &source_path,
    )
}

/// Extract game title from directory name with cleaning
#[tauri::command]
fn extract_game_title(dir_name: &str) -> String {
    let mut title = dir_name.to_string();

    // Remove common prefixes/suffixes
    let prefixes = ["the ", "a ", "an "];
    let suffixes = [
        " (windows)",
        " (pc)",
        " (steam)",
        " (gog)",
        " (epic)",
        " - windows",
        " - pc",
        " - steam",
        " - gog",
        " - epic",
        " [windows]",
        " [pc]",
        " [steam]",
        " [gog]",
        " [epic]",
        " v1",
        " v2",
        " v3",
        " v4",
        " v5",
        " version 1",
        " version 2",
        " version 3",
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
pub fn scan_directory_internal_with_config(
    base_path: &Path,
    config: &ScanConfig,
) -> Result<Vec<ScannedGame>, String> {
    debug!(
        "[scan_directory_internal] base_path: {}",
        base_path.display()
    );

    // Compile regex patterns from config (base + extra)
    let exe_patterns = compile_patterns(&config.base_exe_exclusions, &config.extra_exe_exclusions);
    let folder_patterns = compile_patterns(
        &config.base_folder_exclusions,
        &config.extra_folder_exclusions,
    );

    // Combine metadata files and image extensions
    let all_metadata_files: Vec<String> = config
        .base_metadata_files
        .iter()
        .chain(&config.extra_metadata_files)
        .cloned()
        .collect();
    let all_image_extensions: Vec<String> = config
        .base_image_extensions
        .iter()
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
        let dir_name = path
            .file_name()
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

        let dir_name = game_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Title will be extracted later with fallback strategy

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
        let cover_candidates =
            find_cover_candidates_with_config(&game_path, config, &all_image_extensions);
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
        let exe_metadata = executable
            .as_ref()
            .and_then(|exe| extract_exe_metadata(&game_path.join(exe)));
        if let Some(meta) = &exe_metadata {
            debug!(
                "[scan] Exe metadata: product='{}', company='{}'",
                meta.product_name.as_deref().unwrap_or("n/a"),
                meta.company_name.as_deref().unwrap_or("n/a")
            );
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
        debug!(
            "[find_actual_game_folder] {} has exe directly, using it",
            dir.display()
        );
        return dir.to_path_buf();
    }

    // No exe in current folder - search in subdirectories (up to configured depth)
    // But avoid diving into Engine, Redist, and other non-game folders
    if let Some(found) = find_folder_with_exe(dir, crate::scanner_constants::MAX_GAME_FOLDER_SEARCH_DEPTH as u32, folder_patterns) {
        debug!(
            "[find_actual_game_folder] Found exe in subfolder: {}",
            found.display()
        );
        return found;
    }

    debug!(
        "[find_actual_game_folder] No exe found, returning original dir: {}",
        dir.display()
    );
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
                        return ext_str == "exe" || ext_str == "lnk" || ext_str == "bat";
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
                path.is_file()
                    && path
                        .extension()
                        .map(|ext| {
                            ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("bat")
                        })
                        .unwrap_or(false)
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
        let dir_name = subdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if is_folder_excluded(&dir_name, folder_patterns) {
            debug!(
                "[find_folder_with_exe] Skipping non-game folder: {}",
                subdir.display()
            );
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
        let dir_name = subdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if is_folder_excluded(&dir_name, folder_patterns) {
            debug!(
                "[find_folder_with_exe] Skipping non-game folder: {}",
                subdir.display()
            );
            continue;
        }

        if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1, folder_patterns) {
            return Some(found);
        }
    }

    None
}

/// Find all executable files in directory (including subdirs up to configured depth)
#[allow(dead_code)]
fn find_all_executables(dir: &Path) -> Vec<String> {
    let config = ScanConfig::default();
    let exe_patterns = compile_patterns(&config.base_exe_exclusions, &config.extra_exe_exclusions);
    find_all_executables_with_config(dir, &config, &exe_patterns)
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables_with_config(
    dir: &Path,
    config: &ScanConfig,
    exe_patterns: &[Regex],
) -> Vec<String> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(dir)
        .max_depth(config.max_exe_search_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_str().unwrap_or("").to_lowercase();

                // Support .exe files, .lnk files (Windows shortcuts), and .bat files
                if ext_str == "exe" || ext_str == "lnk" || ext_str == "bat" {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let name_lower = name.to_lowercase();

                    // Skip known non-game executables using regex patterns
                    let should_skip = is_exe_excluded(&name_lower, exe_patterns);

                    if !should_skip && !name.is_empty() {
                        // Store relative path from game dir
                        let relative = path
                            .strip_prefix(dir)
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
            debug!(
                "[pick_best] Priority 1 match: '{}' matches folder '{}'",
                exe, dir_name
            );
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
        debug!(
            "[pick_best] Priority 3: selected largest '{}' ({} bytes)",
            exe, size
        );
    }
    best.map(|(exe, _)| exe)
}

/// Find potential cover/icon images
#[allow(dead_code)]
fn find_cover_candidates(dir: &Path) -> Vec<String> {
    let config = ScanConfig::default();
    let image_extensions: Vec<String> = config
        .base_image_extensions
        .iter()
        .chain(&config.extra_image_extensions)
        .cloned()
        .collect();
    find_cover_candidates_with_config(dir, &config, &image_extensions)
}

/// Find potential cover/icon images with custom configuration
fn find_cover_candidates_with_config(
    dir: &Path,
    config: &ScanConfig,
    image_extensions: &[String],
) -> Vec<String> {
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
        for entry in WalkDir::new(search_path)
            .max_depth(config.max_cover_search_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            // Check if extension is in the configured list (case-insensitive)
            if !image_extensions
                .iter()
                .any(|ext_ok| ext_ok.eq_ignore_ascii_case(&ext))
            {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Prioritize files with cover-like names
            let cover_keywords = [
                "cover",
                "poster",
                "banner",
                "icon",
                "logo",
                "header",
                "art",
                "thumb",
                "image",
                "box",
                "front",
                "back",
                "screenshot",
                "promo",
                "keyart",
                "key_art",
                "key-art",
                "capsule",
                "library",
                "hero",
                "background",
                "bg",
                "wallpaper",
                "tile",
            ];
            let is_cover_like = cover_keywords.iter().any(|kw| name.contains(kw));

            let relative = path
                .strip_prefix(dir)
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
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW};

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
                    if !is_generic_exe_name(&internal_name)
                        && !is_problematic_game_name(&internal_name)
                    {
                        metadata.product_name = Some(internal_name);
                    }
                }
                // If still generic, try original filename
                if metadata
                    .product_name
                    .as_ref()
                    .map_or(true, |p| is_generic_exe_name(p))
                {
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
            ) != 0
                && len > 0
            {
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

