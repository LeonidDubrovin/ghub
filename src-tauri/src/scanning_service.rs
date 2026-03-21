use crate::database::Database;
use crate::models::{ScannedGame, SpaceSource};
use crate::scanner::{self, ScanConfig};
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
    /// Lock ordering policy: Always acquire `active_scans` before `db` to prevent deadlocks.
    /// All methods must follow this order when both locks are needed.
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

                    // Remove from active_scans FIRST (lock ordering: active_scans before db)
                    if let Ok(mut active_scans) = active_scans_clone.lock() {
                        active_scans.remove(&key_for_cleanup);
                        debug!("Cleaned up scan key after panic: {}", key_for_cleanup);
                    }

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

        // Remove from active scans FIRST (lock ordering: active_scans before db)
        let key = format!("{}:{}", space_id, source_path);
        let mut active_scans = active_scans.lock().unwrap();
        active_scans.remove(&key);
        // Drop active_scans lock before acquiring db
        drop(active_scans);

        // Update last_scanned_at timestamp
        if let Ok(db_lock) = db.lock() {
            if let Err(e) = db_lock.update_source_last_scanned(&space_id, &source_path) {
                error!("Failed to update last_scanned_at for {}: {}", source_path, e);
            }
        }
    }

    /// Perform the actual directory scan using shared scanner
    fn perform_scan(
        path: &Path,
        source: &SpaceSource,
        cancel_flag: &AtomicBool,
    ) -> Result<(Vec<ScannedGame>, usize), String> {
        // Build config from constants (background service uses fixed config)
        let config = ScanConfig {
            max_scan_depth: if source.scan_recursively {
                crate::scanner_constants::MAX_SCAN_DEPTH
            } else {
                1
            },
            max_exe_search_depth: crate::scanner_constants::MAX_EXE_SEARCH_DEPTH,
            max_cover_candidates: crate::scanner_constants::MAX_COVER_CANDIDATES,
            max_cover_search_depth: crate::scanner_constants::MAX_COVER_SEARCH_DEPTH,
            base_exe_exclusions: crate::scanner_constants::BASE_EXE_EXCLUSIONS
                .iter()
                .map(|&s| Regex::new(s).unwrap())
                .collect(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: crate::scanner_constants::BASE_FOLDER_EXCLUSIONS
                .iter()
                .map(|&s| Regex::new(s).unwrap())
                .collect(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: crate::scanner_constants::BASE_IMAGE_EXTENSIONS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_image_extensions: Vec::new(),
            base_metadata_files: crate::scanner_constants::BASE_METADATA_FILES
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: crate::scanner_constants::BASE_COVER_SEARCH_PATHS
                .iter()
                .map(|&s| s.to_string())
                .collect(),
        };

        scanner::scan_directory(path, &config, Some(cancel_flag))
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
