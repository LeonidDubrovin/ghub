use crate::database::Database;
use crate::models::{ScannedGame, SpaceSource};
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
use std::thread::{self, JoinHandle};
use walkdir::WalkDir;

lazy_static! {
    static ref EXE_PATTERNS: Vec<Regex> = {
        vec![
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
        ]
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStatus {
    Idle,
    Scanning,
    Completed,
    Error,
}

impl ScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanStatus::Idle => "idle",
            ScanStatus::Scanning => "scanning",
            ScanStatus::Completed => "completed",
            ScanStatus::Error => "error",
        }
    }
}

struct ScanHandle {
    thread: JoinHandle<()>,
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
        // Check if already scanning
        let key = format!("{}:{}", space_id, source_path);
        {
            let active_scans = self.active_scans.lock().map_err(|e| e.to_string())?;
            if active_scans.contains_key(&key) {
                return Err("Scan already in progress for this source".to_string());
            }
        }

        // Set initial status in DB
        {
            let db = db.lock().map_err(|e| e.to_string())?;
            db.set_source_scan_status(
                &space_id,
                &source_path,
                Some(ScanStatus::Scanning.as_str()),
                Some(0),
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
        }

        // Spawn background thread
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();
        let db_clone = db.clone();
        let active_scans_clone = self.active_scans.clone();
        let space_id_clone = space_id.clone();
        let source_path_clone = source_path.clone();

        let thread = thread::spawn(move || {
            Self::scan_source(
                active_scans_clone,
                db_clone,
                space_id_clone,
                source_path_clone,
                cancel_flag_clone,
            );
        });

        let handle = ScanHandle {
            thread,
            cancel_flag,
        };

        let mut active_scans = self.active_scans.lock().map_err(|e| e.to_string())?;
        active_scans.insert(key, handle);

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
                let mut db_lock = db.lock().unwrap();

                // Mark all existing installs for this source as missing initially
                // We'll unmark them as we find them
                if let Ok(mut existing_installs) =
                    db_lock.get_installs_for_source(&space_id, &source_path)
                {
                    for install in existing_installs.iter_mut() {
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
                    if let Some(mut existing_install) = db_lock
                        .get_install_by_path(&space_id, &scanned_game.path)
                        .unwrap_or(None)
                    {
                        // Check fingerprint
                        let new_fingerprint = Self::compute_fingerprint(&scanned_game);
                        let is_modified = if let Some(old_fp) = &existing_install.fingerprint {
                            old_fp != &new_fingerprint
                        } else {
                            false
                        };

                        if is_modified {
                            // Update game info and mark as modified
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
                            // Update install to installed and update fingerprint if needed
                            let _ = db_lock.update_install(
                                &existing_install.id,
                                "installed",
                                Some(&new_fingerprint),
                            );
                        }
                    } else {
                        // Create new game and install
                        let game_id = uuid::Uuid::new_v4().to_string();
                        let install_id = uuid::Uuid::new_v4().to_string();

                        // Create game
                        let _ = db_lock.create_game(
                            &game_id,
                            &scanned_game.title,
                            None,
                            None,
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

        let max_depth = if source.scan_recursively { 5 } else { 1 };

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

            // Extract title from directory name
            let title = Self::extract_game_title(
                game_path
                    .file_name()
                    .and_then(|n: &OsStr| n.to_str())
                    .unwrap_or("Unknown"),
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
            // Could also use hash of first few bytes
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
        // Fallback: use title + size
        format!("{}:{}", scanned_game.title, scanned_game.size_bytes)
    }

    // Helper methods (similar to existing scanning.rs)

    fn is_folder_excluded(dir_name: &str) -> bool {
        let folder_patterns = vec![
            regex::Regex::new(r"(?i)^(engine|redist|redistributables)$").unwrap(),
            regex::Regex::new(r"(?i)^(directx|dotnet|vcredist|physx)$").unwrap(),
            regex::Regex::new(r"(?i)^(prereqs?|prerequisites|support)$").unwrap(),
            regex::Regex::new(r"(?i)^(commonredist|installer|install|setup)$").unwrap(),
            regex::Regex::new(r"(?i)^(update|patch(es)?|backup)$").unwrap(),
            regex::Regex::new(r"(?i)^(temp|tmp|cache|logs)$").unwrap(),
            regex::Regex::new(r"(?i)^(saves?|screenshots?|mods?|plugins?)$").unwrap(),
            regex::Regex::new(r"(?i)^binaries$").unwrap(),
            regex::Regex::new(r"(?i)^__pycache__$").unwrap(),
            regex::Regex::new(r"(?i)^\.git$").unwrap(),
        ];

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

        // Search subdirectories up to 2 levels
        if let Some(found) = Self::find_folder_with_exe(dir, 2) {
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
        let max_depth = 4;

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

        let cover_search_paths = vec![
            "images",
            "image",
            "img",
            "art",
            "assets",
            "media",
            "resources",
            "gfx",
            "graphics",
            "covers",
            "cover",
            "box",
            "boxart",
            "screenshots",
            "screenshot",
            "promo",
        ];

        let image_extensions = vec!["png", "jpg", "jpeg", "ico", "bmp", "webp", "gif"];

        let mut search_paths = vec![dir.to_path_buf()];
        for subdir in &cover_search_paths {
            search_paths.push(dir.join(subdir));
        }

        for search_path in &search_paths {
            if !search_path.exists() {
                continue;
            }

            for entry in WalkDir::new(search_path)
                .max_depth(3)
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

        candidates.truncate(15);
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

    fn extract_game_title(dir_name: &str) -> String {
        let mut title = dir_name.to_string();

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

        for prefix in &prefixes {
            if lower_title.starts_with(prefix) {
                title = title[prefix.len()..].to_string();
                break;
            }
        }

        let lower_title = title.to_lowercase();
        for suffix in &suffixes {
            if lower_title.ends_with(suffix) {
                title = title[..title.len() - suffix.len()].to_string();
                break;
            }
        }

        title = title.split_whitespace().collect::<Vec<&str>>().join(" ");
        title.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_game_title() {
        assert_eq!(
            ScanningService::extract_game_title("The Game (windows)"),
            "Game"
        );
        assert_eq!(ScanningService::extract_game_title("MyGame v1.0"), "MyGame");
        assert_eq!(ScanningService::extract_game_title("  Game  "), "Game");
    }
}
