use crate::models::ScannedGame;
use crate::scanner;
use crate::scanner_constants;
use crate::AppState;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;
use tauri::State;

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

/// Internal scan function that doesn't require a full path string
pub fn scan_directory_internal(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    scan_directory_internal_with_config(base_path, &ScanConfig::default())
}

/// Internal scan function with custom configuration - now uses shared scanner
pub fn scan_directory_internal_with_config(
    base_path: &Path,
    config: &ScanConfig,
) -> Result<Vec<ScannedGame>, String> {
    debug!(
        "[scan_directory_internal] base_path: {}",
        base_path.display()
    );

    // Build scanner::ScanConfig from command config, converting String patterns to Regex
    let scanner_config = scanner::ScanConfig {
        max_scan_depth: config.max_scan_depth,
        max_exe_search_depth: config.max_exe_search_depth,
        max_cover_candidates: config.max_cover_candidates,
        max_cover_search_depth: config.max_cover_search_depth,
        base_exe_exclusions: config
            .base_exe_exclusions
            .iter()
            .map(|s| Regex::new(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        extra_exe_exclusions: config
            .extra_exe_exclusions
            .iter()
            .map(|s| Regex::new(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        base_folder_exclusions: config
            .base_folder_exclusions
            .iter()
            .map(|s| Regex::new(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        extra_folder_exclusions: config
            .extra_folder_exclusions
            .iter()
            .map(|s| Regex::new(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        base_image_extensions: config.base_image_extensions.clone(),
        extra_image_extensions: config.extra_image_extensions.clone(),
        base_metadata_files: config.base_metadata_files.clone(),
        extra_metadata_files: config.extra_metadata_files.clone(),
        cover_search_paths: config.cover_search_paths.clone(),
    };

    // Call shared scanner (no cancellation for synchronous scan)
    let (games, _) = scanner::scan_directory(base_path, &scanner_config, None)?;
    Ok(games)
}

/// Normalize a path for deduplication (canonicalize to resolve symlinks and normalize separators)
fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

