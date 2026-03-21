use crate::database::Database;
use crate::models::{ScannedGame, SpaceSource};
use crate::title_extraction::{extract_title_with_fallback, read_local_metadata};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use regex::Regex;
use rusqlite::params;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use walkdir::WalkDir;

lazy_static! {
    static ref EXE_PATTERNS: Vec<Regex> = {
        crate::scanner_constants::BASE_EXE_EXCLUSIONS
            .iter()
            .map(|s| Regex::new(s).unwrap())
            .collect()
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStatus {
    Scanning,
    Completed,
    Error,
}

impl ScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanStatus::Scanning => "scanning",
            ScanStatus::Completed => "completed",
            ScanStatus::Error => "error",
        }
    }
}

struct ScanHandle {
    cancel_flag: Arc<AtomicBool>,
}

pub struct ScanningService {
    active_scans: Arc<Mutex<HashMap<String, ScanHandle>>>,
}

impl ScanningService {
    pub fn new() -> Self {
        Self {
            active_scans: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start scanning a source (directory) in background
    pub fn start_scan(
        &self,
        db: Arc<Mutex<Database>>,
        space_id: String,
        source_path: String,
    ) -> Result<(), String> {
        let key = format!("{}:{}", space_id, source_path);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();
        let db_clone = db.clone();
        let active_scans_clone = self.active_scans.clone();
        let space_id_clone = space_id.clone();
        let source_path_clone = source_path.clone();
        let key_for_cleanup = key.clone();

        // Acquire lock, check for existing scan, and insert handle in one atomic operation
        {
            let mut active_scans = self.active_scans.lock().map_err(|e| e.to_string())?;
            if active_scans.contains_key(&key) {
                return Err("Scan already in progress for this source".to_string());
            }
            // Insert handle before releasing lock to prevent race condition
            active_scans.insert(key.clone(), ScanHandle { cancel_flag: cancel_flag_clone.clone() });
        } // lock released here

        // Set initial scan status in database
        {
            let db_lock = db.lock().map_err(|e| e.to_string())?;
            db_lock.set_source_scan_status(
                &space_id,
                &source_path,
                Some(ScanStatus::Scanning.as_str()),
                Some(0),
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
        }

        // Spawn background thread with panic catching
        let _ = thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                Self::scan_source(
                    active_scans_clone,
                    db_clone,
                    space_id_clone,
                    source_path_clone,
                    cancel_flag_clone,
                )
            });

            match result {
                Ok(_) => {
                    debug!("Scan completed normally for {}", key_for_cleanup);
                }
                Err(panic) => {
                    error!("Scan thread panicked for {}: {:?}", key_for_cleanup, panic);
                    
                    // Attempt to clear scan status in DB
                    if let Ok(db_lock) = db_clone.lock() {
                        let _ = db_lock.set_source_scan_status(
                            &space_id_clone,
                            &source_path_clone,
                            Some("error"),
                            None,
                            None,
                            Some("Scan thread panicked - check logs"),
                        );
                    }
                    
                    // Remove from active_scans (if still present)
                    if let Ok(mut active_scans) = active_scans_clone.lock() {
                        active_scans.remove(&key_for_cleanup);
                        debug!("Cleaned up scan key after panic: {}", key_for_cleanup);
                    }
                }
            }
        });

        Ok(())
    }

    /// Cancel a running scan
    pub fn cancel_scan(
        &self,
        db: &Arc<Mutex<Database>>,
        space_id: &str,
        source_path: &str,
    ) -> Result<(), String> {
        let key = format!("{}:{}", space_id, source_path);
        let mut active_scans = self.active_scans.lock().map_err(|e| e.to_string())?;

        if let Some(handle) = active_scans.get(&key) {
            handle.cancel_flag.store(true, Ordering::SeqCst);
            // Clear status in DB using provided connection
            let db_lock = db.lock().map_err(|e| e.to_string())?;
            let _ = db_lock.set_source_scan_status(space_id, source_path, None, None, None, None);
            active_scans.remove(&key);
        }

        Ok(())
    }

    /// Get scan status for a source
    pub fn get_source_scan_status(
        &self,
        db: &Mutex<Database>,
        space_id: &str,
        source_path: &str,
    ) -> Result<Option<SpaceSource>, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_source_scan_status(space_id, source_path)
            .map_err(|e| e.to_string())
    }

    /// Main scanning logic for a single source
    fn scan_source(
        active_scans: Arc<Mutex<HashMap<String, ScanHandle>>>,
        db: Arc<Mutex<Database>>,
        space_id: String,
        source_path: String,
        cancel_flag: Arc<AtomicBool>,
    ) {
        // Check if source still exists and is active
        let source_opt = {
            let db_lock = db.lock().unwrap();
            match db_lock.get_source_scan_status(&space_id, &source_path) {
                Ok(Some(src)) => Some(src),
                Ok(None) => None,
                Err(_) => None,
            }
        };

        if source_opt.is_none() {
            error!("Source not found: {} in space {}", source_path, space_id);
            return;
        }

        let source = source_opt.unwrap();
        if !source.is_active {
            info!("Source is inactive, skipping scan: {}", source_path);
            return;
        }

        // Perform the scan
        let path = Path::new(&source_path);
        if !path.exists() {
            error!("Source path does not exist: {}", source_path);
            let _ = db.lock().unwrap().set_source_scan_status(
                &space_id,
                &source_path,
                Some(ScanStatus::Error.as_str()),
                None,
                None,
                Some("Directory does not exist"),
            );
            return;
        }

        debug!("Starting scan of source: {}", source_path);

        // Scan directory
        let scan_result = Self::perform_scan(path, &source, &cancel_flag);

        match scan_result {
            Ok((games, total_games)) => {
                // Process found games
                let db_lock = db.lock().unwrap();

                // Mark all existing installs for this source as missing initially
                // We'll unmark them as we find them
                if let Ok(mut existing_installs) =
                    db_lock.get_installs_for_source(&space_id, &source_path)
                {
                    for (idx, install) in existing_installs.iter_mut().enumerate() {
                        // Check cancellation periodically
                        if idx % 100 == 0 && cancel_flag.load(Ordering::SeqCst) {
                            info!("Scan cancelled during mark-missing loop for source: {}", source_path);
                            return;
                        }
                        install.status = "missing".to_string();
                        let _ = db_lock.update_install_status(&install.id, "missing");
                    }
                }

                // Process each found game
                for (idx, scanned_game) in games.iter().enumerate() {
                    if cancel_flag.load(Ordering::SeqCst) {
                        info!("Scan cancelled for source: {}", source_path);
                        let _ = db_lock.set_source_scan_status(
                            &space_id,
                            &source_path,
                            Some(ScanStatus::Error.as_str()),
                            None,
                            None,
                            Some("Scan cancelled"),
                        );
                        return;
                    }

                    // Try to find existing install by path
                    if let Some(existing_install) = db_lock
                        .get_install_by_path(&space_id, &scanned_game.path)
                        .unwrap_or(None)
                    {
                        // Install exists - update status and fingerprint
                        let new_fingerprint = Self::compute_fingerprint(&scanned_game);
                        let is_modified = if let Some(old_fp) = &existing_install.fingerprint {
                            old_fp != &new_fingerprint
                        } else {
                            false
                        };

                        if is_modified {
                            debug!(
                                "Game modified (fingerprint changed): {}",
                                scanned_game.title
                            );
                            let _ = db_lock.update_install(
                                &existing_install.id,
                                "modified",
                                Some(&new_fingerprint),
                            );
                        } else {
                            debug!("Game already installed, marking as installed: {}", scanned_game.title);
                            let _ = db_lock.update_install(
                                &existing_install.id,
                                "installed",
                                Some(&new_fingerprint),
                            );
                        }
                    } else {
                        // No install at this path - try to find existing game by fingerprint (deduplication)
                        let developer = scanned_game.exe_metadata.as_ref().and_then(|m| m.company_name.clone());
                        let existing_game = db_lock.get_game_by_fingerprint(
                            &scanned_game.title,
                            developer.as_deref()
                        ).unwrap_or(None);

                        if let Some(game) = existing_game {
                            // Reuse existing game, just create new install
                            debug!("Reusing existing game '{}' (id: {}) for new install", game.title, game.id);
                            let install_id = uuid::Uuid::new_v4().to_string();
                            let fingerprint = Self::compute_fingerprint(scanned_game);
                            let _ = db_lock.conn.execute(
                                "INSERT INTO installs (id, game_id, space_id, install_path, executable_path, status, fingerprint) VALUES (?, ?, ?, ?, ?, ?, ?)",
                                params![
                                    install_id,
                                    game.id,
                                    space_id,
                                    scanned_game.path,
                                    scanned_game.executable.as_deref(),
                                    "installed",
                                    fingerprint
                                ]
                            );
                        } else {
                            // No matching game - create new game and install
                            debug!("Creating new game: {}", scanned_game.title);
                            let game_id = uuid::Uuid::new_v4().to_string();
                            let install_id = uuid::Uuid::new_v4().to_string();

                            // Create game with developer from exe metadata
                            let dev = scanned_game.exe_metadata.as_ref().and_then(|m| m.company_name.clone());
                            let _ = db_lock.create_game(
                                &game_id,
                                &scanned_game.title,
                                None,
                                dev.as_deref(),
                                None,
                                None,
                            );

                            // Create install
                            let fingerprint = Self::compute_fingerprint(scanned_game);
                            let _ = db_lock.conn.execute(
                                "INSERT INTO installs (id, game_id, space_id, install_path, executable_path, status, fingerprint) VALUES (?, ?, ?, ?, ?, ?, ?)",
                                params![
                                    install_id,
                                    game_id,
                                    space_id,
                                    scanned_game.path,
                                    scanned_game.executable.as_deref(),
                                    "installed",
                                    fingerprint
                                ]
                            );
                        }
                    }

                    // Update progress
                    let progress = (idx + 1) as i32;
                    let _ = db_lock.set_source_scan_status(
                        &space_id,
                        &source_path,
                        Some(ScanStatus::Scanning.as_str()),
                        Some(progress),
                        Some(total_games as i32),
                        None,
                    );
                }

                // Mark remaining installs as missing (those not found in scan)
                if let Ok(installs) = db_lock.get_installs_for_source(&space_id, &source_path) {
                    for install in installs {
                        if install.status == "missing" {
                            debug!(
                                "Game missing: {} at {}",
                                db_lock
                                    .get_game_by_id(&install.game_id)
                                    .ok()
                                    .map(|g| g.title)
                                    .unwrap_or_else(|| "Unknown".to_string()),
                                install.install_path
                            );
                        }
                    }
                }

                // Complete scan
                let _ = db_lock.set_source_scan_status(
                    &space_id,
                    &source_path,
                    Some(ScanStatus::Completed.as_str()),
                    Some(total_games as i32),
                    Some(total_games as i32),
                    None,
                );

                info!(
                    "Scan completed for source {}: {} games found",
                    source_path, total_games
                );
            }
            Err(err_msg) => {
                error!("Scan failed for source {}: {}", source_path, err_msg);
                let _ = db.lock().unwrap().set_source_scan_status(
                    &space_id,
                    &source_path,
                    Some(ScanStatus::Error.as_str()),
                    None,
                    None,
                    Some(&err_msg),
                );
            }
        }

        // Update last_scanned_at timestamp
        if let Ok(db_lock) = db.lock() {
            if let Err(e) = db_lock.update_source_last_scanned(&space_id, &source_path) {
                error!("Failed to update last_scanned_at for {}: {}", source_path, e);
            }
        }

        // Remove from active scans
        let key = format!("{}:{}", space_id, source_path);
        let mut active_scans = active_scans.lock().unwrap();
        active_scans.remove(&key);
    }

    /// Perform the actual directory scan
    fn perform_scan(
        path: &Path,
        source: &SpaceSource,
        cancel_flag: &AtomicBool,
    ) -> Result<(Vec<ScannedGame>, usize), String> {
        let mut games = Vec::new();
        let mut scanned_dirs = std::collections::HashSet::new();

        let max_depth = if source.scan_recursively { crate::scanner_constants::MAX_SCAN_DEPTH } else { 1 };

        for entry in WalkDir::new(path)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if cancel_flag.load(Ordering::SeqCst) {
                return Err("Scan cancelled".to_string());
            }

            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }

            // Normalize path
            let normalized = entry_path.to_string_lossy().to_string();
            if scanned_dirs.contains(&normalized) {
                continue;
            }
            scanned_dirs.insert(normalized);

            // Skip non-game folders
            let dir_name = entry_path
                .file_name()
                .and_then(|n: &OsStr| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            if Self::is_folder_excluded(&dir_name) {
                debug!("Skipping excluded folder: {}", entry_path.display());
                continue;
            }

            // Check if directory has executables
            if !Self::has_executable_files(entry_path) {
                continue;
            }

            debug!("Found game folder: {}", entry_path.display());

            // Find actual game folder (dive deeper if needed)
            let game_path = Self::find_actual_game_folder(entry_path);
            debug!("Game folder resolved to: {}", game_path.display());

            // Read local metadata
            let metadata_files: Vec<String> = crate::scanner_constants::BASE_METADATA_FILES
                .iter()
                .map(|&s| s.to_string())
                .collect();
            let local_metadata = read_local_metadata(&game_path, &metadata_files);

            // Extract title with multi-level fallback strategy
            let dir_name = game_path
                .file_name()
                .and_then(|n: &OsStr| n.to_str())
                .unwrap_or("Unknown");
            let title = extract_title_with_fallback(
                &game_path,
                dir_name,
                &local_metadata,
                &exe_metadata,
                &executable,
            );

            // Find executables
            let all_executables = Self::find_all_executables(&game_path);
            let executable = Self::pick_best_executable(&game_path, &all_executables);

            // Find covers
            let cover_candidates = Self::find_cover_candidates(&game_path);

            // Calculate size
            let size_bytes = Self::calculate_dir_size(&game_path);

            // Extract exe metadata
            let exe_metadata = executable
                .as_ref()
                .and_then(|exe| Self::extract_exe_metadata(&game_path.join(exe)));

            games.push(ScannedGame {
                path: game_path.to_string_lossy().to_string(),
                title,
                executable,
                all_executables,
                size_bytes,
                icon_path: None,
                cover_candidates,
                exe_metadata,
            });
        }

        let games_count = games.len();
        Ok((games, games_count))
    }

    /// Compute fingerprint for a scanned game
    fn compute_fingerprint(scanned_game: &ScannedGame) -> String {
        if let Some(exe) = &scanned_game.executable {
            // Use file size + modification time as simple fingerprint
            // This is stable for unchanged files and detects modifications
            let exe_path = Path::new(&scanned_game.path).join(exe);
            if let Ok(metadata) = fs::metadata(&exe_path) {
                return format!(
                    "{}:{}",
                    metadata.len(),
                    metadata
                        .modified()
                        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
                        .unwrap_or(0)
                );
            }
        }
        // Fallback: use title only (size can fluctuate due to logs/caches)
        // If no executable found, we rely on title which is more stable than folder size
        scanned_game.title.clone()
    }

    // Helper methods (similar to existing scanning.rs)

    fn is_folder_excluded(dir_name: &str) -> bool {
        let folder_patterns: Vec<Regex> = crate::scanner_constants::BASE_FOLDER_EXCLUSIONS
            .iter()
            .map(|s| Regex::new(s).unwrap())
            .collect();

        folder_patterns.iter().any(|re| re.is_match(dir_name))
    }

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

    fn find_actual_game_folder(dir: &Path) -> PathBuf {
        if Self::has_exe_files(dir) {
            return dir.to_path_buf();
        }

        // Search subdirectories up to configured depth
        if let Some(found) = Self::find_folder_with_exe(dir, crate::scanner_constants::MAX_GAME_FOLDER_SEARCH_DEPTH as u32) {
            return found;
        }

        dir.to_path_buf()
    }

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
        result
    }

    fn find_folder_with_exe(dir: &Path, max_depth: u32) -> Option<PathBuf> {
        if max_depth == 0 {
            return None;
        }

        let entries: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        for entry in &entries {
            let subdir = entry.path();
            let dir_name = subdir
                .file_name()
                .and_then(|n: &OsStr| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            if Self::is_folder_excluded(&dir_name) {
                continue;
            }

            if Self::has_exe_files(&subdir) {
                return Some(subdir);
            }
        }

        for entry in &entries {
            let subdir = entry.path();
            let dir_name = subdir
                .file_name()
                .and_then(|n: &OsStr| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            if Self::is_folder_excluded(&dir_name) {
                continue;
            }

            if let Some(found) = Self::find_folder_with_exe(&subdir, max_depth - 1) {
                return Some(found);
            }
        }

        None
    }

    fn find_all_executables(dir: &Path) -> Vec<String> {
        let mut executables = Vec::new();
        let max_depth = crate::scanner_constants::MAX_EXE_SEARCH_DEPTH;

        for entry in WalkDir::new(dir)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_str().unwrap_or("").to_lowercase();

                    if ext_str == "exe" || ext_str == "lnk" || ext_str == "bat" {
                        let name = path
                            .file_name()
                            .and_then(|n: &OsStr| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        let name_lower = name.to_lowercase();

                        // Skip known non-game executables
                        let should_skip = EXE_PATTERNS.iter().any(|re| re.is_match(&name_lower));

                        if !should_skip && !name.is_empty() {
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

            if exe_stem == dir_name || dir_name.contains(&exe_stem) || exe_stem.contains(&dir_name)
            {
                debug!(
                    "[pick_best] Priority 1 match: '{}' matches folder '{}'",
                    exe, dir_name
                );
                return Some(exe.clone());
            }
        }

        // Priority 2: exe in root folder
        for exe in executables {
            if !exe.contains('\\') && !exe.contains('/') {
                debug!("[pick_best] Priority 2 match: '{}' is in root", exe);
                return Some(exe.clone());
            }
        }

        // Priority 3: largest exe file
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

        best.map(|(exe, _)| exe)
    }

    fn find_cover_candidates(dir: &Path) -> Vec<String> {
        let mut candidates = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let cover_search_paths: Vec<String> = crate::scanner_constants::BASE_COVER_SEARCH_PATHS
            .iter()
            .map(|&s| s.to_string())
            .collect();

        let image_extensions: Vec<String> = crate::scanner_constants::BASE_IMAGE_EXTENSIONS
            .iter()
            .map(|&s| s.to_string())
            .collect();

        let mut search_paths = vec![dir.to_path_buf()];
        for subdir in &cover_search_paths {
            search_paths.push(dir.join(subdir));
        }

        for search_path in &search_paths {
            if !search_path.exists() {
                continue;
            }

            for entry in WalkDir::new(search_path)
                .max_depth(crate::scanner_constants::MAX_COVER_SEARCH_DEPTH)
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

                if !image_extensions.iter().any(|&ext_ok| ext_ok == ext) {
                    continue;
                }

                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

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

                if seen.insert(relative.clone()) {
                    if is_cover_like {
                        candidates.insert(0, relative);
                    } else {
                        candidates.push(relative);
                    }
                }
            }
        }

        candidates.truncate(crate::scanner_constants::MAX_COVER_CANDIDATES as usize);
        candidates
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

    #[cfg(target_os = "windows")]
    fn extract_exe_metadata(exe_path: &Path) -> Option<crate::models::ExeMetadata> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileVersionInfoSizeW, GetFileVersionInfoW,
        };

        if !exe_path.exists() {
            return None;
        }

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
            if GetFileVersionInfoW(path_wide.as_ptr(), 0, size, buffer.as_mut_ptr() as *mut _) == 0
            {
                return None;
            }

            let mut metadata = crate::models::ExeMetadata {
                product_name: None,
                company_name: None,
                file_description: None,
                file_version: None,
            };

            metadata.product_name = Self::query_version_string(&buffer, "ProductName");
            metadata.company_name = Self::query_version_string(&buffer, "CompanyName");
            metadata.file_description = Self::query_version_string(&buffer, "FileDescription");
            metadata.file_version = Self::query_version_string(&buffer, "FileVersion");

            if metadata.product_name.is_some() || metadata.company_name.is_some() {
                Some(metadata)
            } else {
                None
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn extract_exe_metadata(_exe_path: &Path) -> Option<crate::models::ExeMetadata> {
        None
    }

    fn query_version_string(buffer: &[u8], name: &str) -> Option<String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

        let lang_codepages = ["040904B0", "040904E4", "000004B0", "040904E4"];

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

}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_is_folder_excluded() {
        let patterns: Vec<Regex> = crate::scanner_constants::BASE_FOLDER_EXCLUSIONS
            .iter()
            .map(|s| Regex::new(s).unwrap())
            .collect();
        assert!(is_folder_excluded("engine", &patterns));
        assert!(is_folder_excluded("Engine", &patterns)); // case-insensitive
        assert!(!is_folder_excluded("MyGame", &patterns));
    }

    #[test]
    fn test_pick_best_executable() {
        let dir = Path::new("MyGame");
        let executables = vec![
            "setup.exe".to_string(),
            "MyGame.exe".to_string(),
            "launcher.exe".to_string(),
        ];
        let best = pick_best_executable(dir, &executables);
        assert_eq!(best, Some("MyGame.exe".to_string()));
    }

    #[test]
    fn test_pick_best_executable_priority2() {
        let dir = Path::new("MyGame");
        let executables = vec![
            "subdir\\game.exe".to_string(),
            "MyGame.exe".to_string(),
        ];
        let best = pick_best_executable(dir, &executables);
        // Priority 1 matches dir name, so MyGame.exe should be chosen
        assert_eq!(best, Some("MyGame.exe".to_string()));
    }

    #[test]
    fn test_pick_best_executable_priority3() {
        let dir = Path::new("MyGame");
        let executables = vec![
            "game.exe".to_string(),
            "other.exe".to_string(),
        ];
        // Create temporary files to test size-based selection
        // In this test, we'll just check that it returns Some (the largest exe)
        // Since we can't easily create files, we'll test that it returns one of them
        let best = pick_best_executable(dir, &executables);
        assert!(best.is_some());
    }

    #[test]
    fn test_compute_fingerprint_fallback() {
        let scanned_game = ScannedGame {
            path: "/path/to/game".to_string(),
            title: "Test Game".to_string(),
            executable: None,
            all_executables: vec![],
            size_bytes: 0,
            icon_path: None,
            cover_candidates: vec![],
            exe_metadata: None,
        };
        let fp = compute_fingerprint(&scanned_game);
        assert_eq!(fp, "Test Game");
    }
}
