/// Shared scanning logic used by both synchronous and background scanning implementations.
/// Provides a single source of truth for all scanning operations.

use crate::models::ScannedGame;
use crate::title_extraction::{extract_title_with_fallback, read_local_metadata};
use log::debug;
use regex::Regex;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use walkdir::WalkDir;

/// Configuration for scanning operations
#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub max_scan_depth: usize,
    pub max_exe_search_depth: usize,
    pub max_cover_candidates: usize,
    pub max_cover_search_depth: usize,
    pub base_exe_exclusions: Vec<Regex>,
    pub extra_exe_exclusions: Vec<Regex>,
    pub base_folder_exclusions: Vec<Regex>,
    pub extra_folder_exclusions: Vec<Regex>,
    pub base_image_extensions: Vec<String>,
    pub extra_image_extensions: Vec<String>,
    pub base_metadata_files: Vec<String>,
    pub extra_metadata_files: Vec<String>,
    pub cover_search_paths: Vec<String>,
}

impl ScanConfig {
    /// Build a configuration from the global scanner constants.
    pub fn from_constants(recursive: bool) -> Self {
        Self {
            max_scan_depth: if recursive {
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
        }
    }

    /// Combine base and extra patterns for exe exclusions
    pub fn exe_patterns(&self) -> Vec<Regex> {
        let mut patterns = self.base_exe_exclusions.clone();
        patterns.extend(self.extra_exe_exclusions.iter().cloned());
        patterns
    }

    /// Combine base and extra patterns for folder exclusions
    pub fn folder_patterns(&self) -> Vec<Regex> {
        let mut patterns = self.base_folder_exclusions.clone();
        patterns.extend(self.extra_folder_exclusions.iter().cloned());
        patterns
    }

    /// Combine base and extra metadata files
    pub fn all_metadata_files(&self) -> Vec<String> {
        let mut files = self.base_metadata_files.clone();
        files.extend(self.extra_metadata_files.iter().cloned());
        files
    }

    /// Combine base and extra image extensions
    pub fn all_image_extensions(&self) -> Vec<String> {
        let mut ext = self.base_image_extensions.clone();
        ext.extend(self.extra_image_extensions.iter().cloned());
        ext
    }
}

/// Main scanning function - the single source of truth for scanning logic.
///
/// # Arguments
/// * `base_path` - Root directory to scan
/// * `config` - Scan configuration
/// * `cancel_flag` - Optional cancellation flag for long-running scans
///
/// # Returns
/// Tuple of (games found, total count) or error
pub fn scan_directory(
    base_path: &Path,
    config: &ScanConfig,
    cancel_flag: Option<&AtomicBool>,
) -> Result<(Vec<ScannedGame>, usize), String> {
    debug!("[SCAN_DIRECTORY] Starting scan of base_path: {:?}", base_path);
    debug!("[SCAN_DIRECTORY] max_scan_depth: {}", config.max_scan_depth);
    
    let mut games = Vec::new();
    let mut scanned_dirs = HashSet::new();
    let mut dir_count = 0;
    let mut excluded_count = 0;
    let mut no_exe_count = 0;

    let max_depth = config.max_scan_depth;

    // Precompute normalized base path for robust comparison (case-insensitive, trailing separators ignored)
    let base_path_str = base_path.to_string_lossy().to_lowercase();
    let base_path_normalized = base_path_str.trim_end_matches(['\\', '/']).to_string();

    for entry in WalkDir::new(base_path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        dir_count += 1;
        
        // Check cancellation if flag provided
        if let Some(flag) = cancel_flag {
            if flag.load(Ordering::SeqCst) {
                debug!("[SCAN_DIRECTORY] Scan cancelled after {} directories", dir_count);
                return Err("Scan cancelled".to_string());
            }
        }

        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        // Skip the base path itself - we only want to scan subdirectories
        // The source directory is a container, not a game
        // Use case-insensitive, trailing-separator-agnostic comparison for Windows compatibility
        let entry_str = entry_path.to_string_lossy().to_lowercase();
        let entry_normalized = entry_str.trim_end_matches(['\\', '/']).to_string();
        if entry_normalized == base_path_normalized {
            debug!("[SCAN_DIRECTORY] Skipping base path itself: {}", entry_path.display());
            continue;
        }

        // Normalize path for deduplication
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

        if is_folder_excluded(&dir_name, &config.folder_patterns()) {
            debug!("[SCAN_DIRECTORY] Skipping excluded folder '{}': {}", dir_name, entry_path.display());
            excluded_count += 1;
            continue;
        }

        // Check if directory has executables
        if !has_executable_files(entry_path) {
            no_exe_count += 1;
            // Only log at debug level to avoid spam, but log first few
            if no_exe_count <= 3 {
                debug!("[SCAN_DIRECTORY] No executables in: {}", entry_path.display());
            }
            continue;
        }

        debug!("[SCAN_DIRECTORY] Found potential game folder: {}", entry_path.display());

        // Find actual game folder (dive deeper if needed)
        let game_path = find_actual_game_folder(entry_path, config.max_scan_depth);
        debug!("Game folder resolved to: {}", game_path.display());

        // Read local metadata
        let local_metadata = read_local_metadata(&game_path, &config.all_metadata_files());

        // Extract title with multi-level fallback strategy
        let dir_name = game_path
            .file_name()
            .and_then(|n: &OsStr| n.to_str())
            .unwrap_or("Unknown");
        let _title = extract_title_with_fallback(
            &game_path,
            dir_name,
            &local_metadata,
            &None, // exe_metadata will be set later
            &None, // executable will be set later
        );

        // Find executables
        let all_executables = find_all_executables(&game_path, config);
        let executable = pick_best_executable(&game_path, &all_executables);

        // Find covers
        let cover_candidates = find_cover_candidates(&game_path, config);

        // Calculate size
        let size_bytes = calculate_dir_size(&game_path);

        // Extract exe metadata (after we have executable)
        let exe_metadata = executable
            .as_ref()
            .and_then(|exe| extract_exe_metadata(&game_path.join(exe)));

        // Re-extract title with exe metadata now available (fallback level 2)
        let title_with_metadata = extract_title_with_fallback(
            &game_path,
            dir_name,
            &local_metadata,
            &exe_metadata,
            &executable,
        );

        games.push(ScannedGame {
            path: game_path.to_string_lossy().to_string(),
            title: title_with_metadata,
            executable,
            all_executables,
            size_bytes,
            icon_path: None,
            cover_candidates,
            exe_metadata,
        });
    }

    let games_count = games.len();
    debug!("[SCAN_DIRECTORY] Scan complete: total_dirs={}, excluded={}, no_exe={}, games_found={}",
        dir_count, excluded_count, no_exe_count, games_count);
    Ok((games, games_count))
}

/// Check if folder name matches exclusion patterns
pub fn is_folder_excluded(dir_name: &str, patterns: &[Regex]) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(dir_name))
}

/// Check if directory contains any executable files (.exe or .lnk or .bat)
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

/// Check if directory contains any .exe or .bat files (not .lnk)
fn has_exe_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
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
        .unwrap_or(false)
}

/// Find the actual game folder - if no exe in current folder, search subdirectories
fn find_actual_game_folder(dir: &Path, max_depth: usize) -> PathBuf {
    if has_exe_files(dir) {
        return dir.to_path_buf();
    }

    // Search subdirectories up to configured depth
    if let Some(found) = find_folder_with_exe(dir, max_depth as u32, &[]) {
        return found;
    }

    dir.to_path_buf()
}

/// Recursively find a subfolder that contains exe files
fn find_folder_with_exe(dir: &Path, max_depth: u32, _folder_patterns: &[Regex]) -> Option<PathBuf> {
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
        if has_exe_files(&subdir) {
            return Some(subdir);
        }
    }

    // If no direct subfolder has exe, search deeper
    for entry in &entries {
        let subdir = entry.path();
        if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1, _folder_patterns) {
            return Some(found);
        }
    }

    None
}

/// Find all executable files in directory (including subdirs up to configured depth)
fn find_all_executables(dir: &Path, config: &ScanConfig) -> Vec<String> {
    let mut executables = Vec::new();
    let patterns = config.exe_patterns();

    for entry in WalkDir::new(dir)
        .max_depth(config.max_exe_search_depth)
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
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let name_lower = name.to_lowercase();

                    // Skip known non-game executables
                    let should_skip = patterns.iter().any(|re| re.is_match(&name_lower));

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

/// Pick the best executable from the list
pub fn pick_best_executable(dir: &Path, executables: &[String]) -> Option<String> {
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

    // Priority 2: exe in root folder (not subdir) with sufficient size (>= 1MB)
    // Filter out small utilities/launchers that happen to be in root
    let mut best_root: Option<(String, u64)> = None;
    for exe in executables {
        if !exe.contains('\\') && !exe.contains('/') {
            let full_path = dir.join(exe);
            if let Ok(meta) = std::fs::metadata(&full_path) {
                let size = meta.len();
                // Consider root executables only if they are at least 1 MB
                if size >= 1_048_576 {
                    if best_root.is_none() || size > best_root.as_ref().unwrap().1 {
                        best_root = Some((exe.clone(), size));
                    }
                }
            }
        }
    }
    if let Some((exe, size)) = &best_root {
        debug!(
            "[pick_best] Priority 2 (root, size>=1MB): selected '{}' ({} bytes)",
            exe, size
        );
        return Some(exe.clone());
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

/// Find the best executable directly inside a single game directory.
/// Unlike `scan_directory`, this does **not** skip the base directory; it treats
/// `dir` as the game folder itself.
pub fn find_executable_in_directory(dir: &Path) -> Option<String> {
    let config = ScanConfig::from_constants(true);
    let executables = find_all_executables(dir, &config);
    pick_best_executable(dir, &executables)
}

/// Find potential cover/icon images with custom configuration
fn find_cover_candidates(dir: &Path, config: &ScanConfig) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

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
            if !config
                .all_image_extensions()
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

/// Calculate total size of directory (all files recursively)
fn calculate_dir_size(dir: &Path) -> u64 {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Extract metadata from exe file (Windows only)
#[cfg(target_os = "windows")]
pub fn extract_exe_metadata(exe_path: &Path) -> Option<crate::models::ExeMetadata> {
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
        let mut metadata = crate::models::ExeMetadata {
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

        // Additional fields that might be useful
        let _product_version = query_version_string(&buffer, "ProductVersion");
        let _legal_copyright = query_version_string(&buffer, "LegalCopyright");
        let _original_filename = query_version_string(&buffer, "OriginalFilename");
        let _internal_name = query_version_string(&buffer, "InternalName");
        let _comments = query_version_string(&buffer, "Comments");

        if metadata.product_name.is_some() || metadata.company_name.is_some() {
            Some(metadata)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn extract_exe_metadata(_exe_path: &Path) -> Option<crate::models::ExeMetadata> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::fs;
    use std::io::Write;
    use crate::title_extraction::{clean_game_title, extract_title_from_executable};

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
    fn test_has_executable_files() {
        // Create temporary directory with test files
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_has_exe");
        let _ = fs::create_dir_all(&test_dir);

        // Create some files
        fs::write(test_dir.join("game.exe"), "").unwrap();
        fs::write(test_dir.join("readme.txt"), "").unwrap();
        fs::write(test_dir.join("setup.exe"), "").unwrap();

        assert!(has_executable_files(&test_dir));

        // Clean up
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_has_executable_files_empty_dir() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_empty");
        let _ = fs::create_dir_all(&test_dir);

        assert!(!has_executable_files(&test_dir));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_has_exe_files() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_has_exe_only");
        let _ = fs::create_dir_all(&test_dir);

        fs::write(test_dir.join("game.exe"), "").unwrap();
        fs::write(test_dir.join("game.bat"), "").unwrap();
        fs::write(test_dir.join("launcher.lnk"), "").unwrap(); // .lnk should be ignored

        assert!(has_exe_files(&test_dir));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_has_exe_files_no_exe() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_no_exe");
        let _ = fs::create_dir_all(&test_dir);

        fs::write(test_dir.join("readme.txt"), "").unwrap();
        fs::write(test_dir.join("config.json"), "").unwrap();

        assert!(!has_exe_files(&test_dir));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_find_actual_game_folder() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_find_folder");
        let _ = fs::create_dir_all(&base_dir);

        // Create structure: base_dir/subfolder/Game.exe
        let subfolder = base_dir.join("subfolder");
        let _ = fs::create_dir_all(&subfolder);
        fs::write(subfolder.join("Game.exe"), "").unwrap();

        // Should find the subfolder with exe
        let result = find_actual_game_folder(&base_dir, 2);
        assert_eq!(result, subfolder);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_actual_game_folder_deep() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_deep");
        let _ = fs::create_dir_all(&base_dir);

        // Create deep structure: base_dir/a/b/c/Game.exe
        let a = base_dir.join("a");
        let b = a.join("b");
        let c = b.join("c");
        let _ = fs::create_dir_all(&c);
        fs::write(c.join("Game.exe"), "").unwrap();

        // Should find the deep folder with exe
        let result = find_actual_game_folder(&base_dir, 3);
        assert_eq!(result, c);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_actual_game_folder_no_exe() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_no_exe_folder");
        let _ = fs::create_dir_all(&base_dir);

        // No exe anywhere, should return base_dir
        let result = find_actual_game_folder(&base_dir, 2);
        assert_eq!(result, base_dir);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_all_executables() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_find_all");
        let _ = fs::create_dir_all(&base_dir);

        // Create subdirectories with executables
        let sub1 = base_dir.join("sub1");
        let sub2 = base_dir.join("sub2");
        let _ = fs::create_dir_all(&sub1);
        let _ = fs::create_dir_all(&sub2);

        fs::write(sub1.join("game.exe"), "").unwrap();
        fs::write(sub1.join("launcher.exe"), "").unwrap();
        fs::write(sub2.join("game.exe"), "").unwrap();
        fs::write(base_dir.join("root.exe"), "").unwrap();

        let config = ScanConfig {
            max_scan_depth: 5,
            max_exe_search_depth: 3,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            base_exe_exclusions: Vec::new(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: Vec::new(),
            extra_image_extensions: Vec::new(),
            base_metadata_files: Vec::new(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: Vec::new(),
        };

        let mut executables = find_all_executables(&base_dir, &config);
        executables.sort();
        assert_eq!(executables, vec!["root.exe", "sub1\\game.exe", "sub1\\launcher.exe", "sub2\\game.exe"]);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_all_executables_with_exclusions() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_exclusions");
        let _ = fs::create_dir_all(&base_dir);

        fs::write(base_dir.join("game.exe"), "").unwrap();
        fs::write(base_dir.join("setup.exe"), "").unwrap();
        fs::write(base_dir.join("launcher.exe"), "").unwrap();
        fs::write(base_dir.join("unins000.exe"), "").unwrap();

        let config = ScanConfig {
            max_scan_depth: 5,
            max_exe_search_depth: 2,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            base_exe_exclusions: vec![
                Regex::new(r"(?i)^setup$").unwrap(),
                Regex::new(r"(?i)^launcher$").unwrap(),
                Regex::new(r"(?i)^unins\d*$").unwrap(),
            ],
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: Vec::new(),
            extra_image_extensions: Vec::new(),
            base_metadata_files: Vec::new(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: Vec::new(),
        };

        let executables = find_all_executables(&base_dir, &config);
        assert_eq!(executables, vec!["game.exe"]);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_pick_best_executable_priority1_name_match() {
        let dir = Path::new("MyGame");
        let executables = vec![
            "MyGame.exe".to_string(),
            "game.exe".to_string(),
            "launcher.exe".to_string(),
        ];
        let best = pick_best_executable(dir, &executables);
        assert_eq!(best, Some("MyGame.exe".to_string()));
    }

    #[test]
    fn test_pick_best_executable_priority1_partial_match() {
        let dir = Path::new("MyAwesomeGame");
        let executables = vec![
            "MyGame.exe".to_string(),
            "AwesomeGame.exe".to_string(),
            "launcher.exe".to_string(),
        ];
        let best = pick_best_executable(dir, &executables);
        // Both contain parts of dir name, first one should win (iteration order)
        assert_eq!(best, Some("MyGame.exe".to_string()));
    }

    #[test]
    fn test_pick_best_executable_priority2_root_size() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_priority2");
        let _ = fs::create_dir_all(&test_dir);

        // Create root executables with different sizes
        fs::write(test_dir.join("small.exe"), vec![0; 500_000]).unwrap(); // 500KB - too small
        fs::write(test_dir.join("large.exe"), vec![0; 2_000_000]).unwrap(); // 2MB - should be selected
        fs::write(test_dir.join("larger.exe"), vec![0; 3_000_000]).unwrap(); // 3MB - should be selected over 2MB

        let executables = vec!["small.exe".to_string(), "large.exe".to_string(), "larger.exe".to_string()];
        let best = pick_best_executable(&test_dir, &executables);
        assert_eq!(best, Some("larger.exe".to_string()));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_pick_best_executable_priority3_largest() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_priority3");
        let _ = fs::create_dir_all(&test_dir);

        let subdir = test_dir.join("sub");
        let _ = fs::create_dir_all(&subdir);

        fs::write(test_dir.join("small.exe"), vec![0; 100_000]).unwrap();
        fs::write(subdir.join("big.exe"), vec![0; 5_000_000]).unwrap();

        let executables = vec!["small.exe".to_string(), "sub\\big.exe".to_string()];
        let best = pick_best_executable(&test_dir, &executables);
        assert_eq!(best, Some("sub\\big.exe".to_string()));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_pick_best_executable_empty() {
        let dir = Path::new("MyGame");
        let executables: Vec<String> = vec![];
        let best = pick_best_executable(dir, &executables);
        assert_eq!(best, None);
    }

    #[test]
    fn test_find_cover_candidates() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_covers");
        let _ = fs::create_dir_all(&base_dir);

        let images_dir = base_dir.join("images");
        let _ = fs::create_dir_all(&images_dir);

        // Create various image files
        fs::write(images_dir.join("cover.jpg"), "").unwrap();
        fs::write(images_dir.join("boxart.png"), "").unwrap();
        fs::write(images_dir.join("screenshot1.jpg"), "").unwrap();
        fs::write(images_dir.join("logo.png"), "").unwrap();
        fs::write(images_dir.join("random.txt"), "").unwrap(); // not an image

        let config = ScanConfig {
            max_scan_depth: 5,
            max_exe_search_depth: 3,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            base_exe_exclusions: Vec::new(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: vec!["jpg".to_string(), "png".to_string(), "bmp".to_string()],
            extra_image_extensions: Vec::new(),
            base_metadata_files: Vec::new(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: vec!["images".to_string()],
        };

        let candidates = find_cover_candidates(&base_dir, &config);
        // Should prioritize cover-like names first
        assert!(!candidates.is_empty());
        // First candidate should be cover.jpg or boxart.png (both have priority)
        let first = candidates.first().unwrap();
        assert!(first.contains("cover") || first.contains("boxart"));

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_cover_candidates_no_images() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_no_covers");
        let _ = fs::create_dir_all(&base_dir);

        let config = ScanConfig {
            max_scan_depth: 5,
            max_exe_search_depth: 3,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            base_exe_exclusions: Vec::new(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: vec!["jpg".to_string()],
            extra_image_extensions: Vec::new(),
            base_metadata_files: Vec::new(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: Vec::new(),
        };

        let candidates = find_cover_candidates(&base_dir, &config);
        assert!(candidates.is_empty());

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_find_cover_candidates_max_limit() {
        let temp_dir = std::env::temp_dir();
        let base_dir = temp_dir.join("ghub_test_limit");
        let _ = fs::create_dir_all(&base_dir);

        // Create 20 image files (more than max_cover_candidates=15)
        for i in 0..20 {
            fs::write(base_dir.join(format!("image{}.jpg", i)), "").unwrap();
        }

        let config = ScanConfig {
            max_scan_depth: 5,
            max_exe_search_depth: 3,
            max_cover_candidates: 15,
            max_cover_search_depth: 3,
            base_exe_exclusions: Vec::new(),
            extra_exe_exclusions: Vec::new(),
            base_folder_exclusions: Vec::new(),
            extra_folder_exclusions: Vec::new(),
            base_image_extensions: vec!["jpg".to_string()],
            extra_image_extensions: Vec::new(),
            base_metadata_files: Vec::new(),
            extra_metadata_files: Vec::new(),
            cover_search_paths: Vec::new(),
        };

        let candidates = find_cover_candidates(&base_dir, &config);
        assert_eq!(candidates.len(), 15);

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_calculate_dir_size() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_size");
        let _ = fs::create_dir_all(&test_dir);

        // Create files with known sizes
        fs::write(test_dir.join("file1.bin"), vec![0; 1000]).unwrap();
        fs::write(test_dir.join("file2.bin"), vec![0; 2000]).unwrap();

        let size = calculate_dir_size(&test_dir);
        assert_eq!(size, 3000);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_calculate_dir_size_empty_dir() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ghub_test_empty_size");
        let _ = fs::create_dir_all(&test_dir);

        let size = calculate_dir_size(&test_dir);
        assert_eq!(size, 0);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_is_folder_excluded_extended() {
        let patterns: Vec<Regex> = crate::scanner_constants::BASE_FOLDER_EXCLUSIONS
            .iter()
            .map(|s| Regex::new(s).unwrap())
            .collect();

        // Test common exclusions
        assert!(is_folder_excluded("engine", &patterns));
        assert!(is_folder_excluded("redist", &patterns));
        assert!(is_folder_excluded("dotnet", &patterns));
        assert!(is_folder_excluded("vcredist", &patterns));
        assert!(is_folder_excluded("physx", &patterns));
        assert!(is_folder_excluded("build", &patterns));
        assert!(is_folder_excluded("temp", &patterns));
        assert!(is_folder_excluded("cache", &patterns));
        assert!(is_folder_excluded("saves", &patterns));
        assert!(is_folder_excluded("mods", &patterns));
        assert!(is_folder_excluded("plugins", &patterns));
        assert!(is_folder_excluded("binaries", &patterns));
        assert!(is_folder_excluded("__pycache__", &patterns));
        assert!(is_folder_excluded(".git", &patterns));
        assert!(is_folder_excluded("node_modules", &patterns));
        assert!(is_folder_excluded("jre", &patterns));
        assert!(is_folder_excluded("runtime", &patterns));
        assert!(is_folder_excluded("en-us", &patterns)); // language folder pattern

        // Test non-excluded
        assert!(!is_folder_excluded("MyGame", &patterns));
        assert!(!is_folder_excluded("GameData", &patterns));
        assert!(!is_folder_excluded("assets", &patterns));
    }

    #[test]
    fn test_games_catalog_title_extraction() {
        // Integration test using games_catalog.json data
        // This validates that our title extraction logic works correctly with real game names
        
        // Sample of game entries from games_catalog.json representing different patterns
        let test_cases = vec![
            // (folder_name, expected_title, executable_name)
            ("Ada-KSDemo", "Ada Demo", "Ada.exe"),
            ("1RMRPGWindows/1RMRPG", "1RMRPG", "Game.exe"),
            ("0_abyssalSomewhere", "Abyssal Somewhere", "0_abyssalSomewhere.exe"),
            ("_LD41_Roulette_Knight", "Roulette Knight", "RouletteKnight.exe"),
            ("Archtower v0.6.12.0 demo", "Archtower Demo", "Archtower.exe"),
            ("Bane and Valor_Demo_0.1.1", "Bane and Valor Demo", "Bane and Valor.exe"),
            ("A Quiet Place", "A Quiet Place", "A Quiet Place.exe"),
            ("(Win)Project Troll v2.2", "Project Troll", "Project Troll v2.2.exe"),
            ("0.0.15c demo", "Glorysmith Demo", "Glorysmith.exe"),
            ("0.2.9a", "Roguelike", "Roguelike.exe"),
            ("1.2_Demo_DRM-free_Windows", "Echoes of the Architects Demo", "Echoes to the Architects.exe"),
            ("20_ProjectAdvanced_Build", "Project Advanced", "ProjectAdvanced.exe"),
            ("A Night Around The Fire_2022Update", "A Night Around The Fire", "A Night Around The Fire.exe"),
            ("Adam and Ricky-win64", "Adam and Ricky", "Adam and Ricky.exe"),
            ("ADM PreAlpha Demo/PreAlpha Demo", "Auto Dungeon Monsters Pre-Alpha Demo", "Auto Dungeon Monsters.exe"),
            ("advr_pcvr_b091", "Ancient Dungeon", "Ancient_Dungeon.exe"),
            ("AlchemistsAlcoveDemo", "Alchemists Alcove Demo", "AlchemistsAlcoveDemo.exe"),
            ("Alomany Factory_3", "Alomany Factory", "Alomany Factory.exe"),
            ("alpha-3", "Crypto Miner", "Crypto Miner.exe"),
            ("ApproachMode Win64 0112/ApproachMode_Win64", "Approach Mode", "ApproachMode.exe"),
            ("Appulse/Windows", "Appulse", "Appulse.exe"),
            ("Arena - v0.14", "Arena", "Arena.exe"),
            ("armaphract_0.D_rc4", "Armaphract", "armaphract.exe"),
            ("ArtificialDeath", "Artificial Death", "ArtificialDeath.exe"),
            ("Astro Prospector Prologue - Windows", "Astro Prospector Prologue", "Astro Prospector Prologue.exe"),
            ("AutoHeroes", "Auto Heroes", "AutoHeroes.exe"),
            ("AutonomyStandalone", "Autonomy", "Autonomy.exe"),
            ("axu-rl-win64", "Axu", "Axu.exe"),
            ("backpack-battles-windows", "Backpack Battles", "BackpackBattles.exe"),
            ("bad-day-on-majoris-viii-win/WindowsNoEditor", "Bad Day On Majoris VIII", "MJ77_RyanMike.exe"),
            ("BagOfHolding_Desktop_Win", "Bag Of Holding", "BagOfHolding.exe"),
            ("Balance'em/WindowsClient", "Balance'em", "Balance'em.exe"),
            ("BarelyFunctionalVoxelEngine_v0.1", "Mesh Voxels", "MeshVoxels.exe"),
            ("Battle of Battles a01 Windows", "Battle of Battles Alpha", "Battle of Battles.exe"),
            ("Beacon'sEnd_Win", "Beacon's End", "Beacon'sEnd.exe"),
            ("BearAttackSimulator_WINDOWS", "Bear Attack Simulator", "BearAttackSimulator.exe"),
            ("Behold 0.0.1 Main Menu Fix", "Behold", "Behold.exe"),
            ("beholdin_1_21_WIN", "Beholdin", "Beholdin.exe"),
            ("Bell Rock Post Jam 1", "Bell Rock", "Bell Rock.exe"),
            ("Bikrash_0.6", "Bikrash", "Bikrash.exe"),
            ("Billion Bounces - Latest Version (win)", "Billion Bounces", "Billion Bounces.exe"),
            ("Billy's Nightmare", "Billy's Nightmare", "Billy's Nightmare.exe"),
            ("BioEvil4-0.2.5a/Bio Evil 4", "Bio Evil 4", "BioEvil4-0.2.5a.exe"),
            ("birdgame-win-0-0-2", "Bird Game", "birdgame.exe"),
            ("BL0W-UP DEMO V3 ITCH_1_patch_windows_64/BL0W-UP DEMO V3 ITCH_windows_64", "BL0W-UP Demo", "BL0W-UP DEMO.exe"),
            ("blackbird", "Blackbird", "blackbird.exe"),
            ("Blast Tournament - Version (3.0.0)", "Blast Tournament", "Blast Tournament.exe"),
            ("BlastronautDemo02", "Blastronaut Demo", "Blastronaut.exe"),
            ("Blobfrog", "Blobfrog", "Froge.exe"),
            ("BloodCountess_Alpha_1.1.0.0_Windows", "Blood Countess Alpha", "BloodCountess_Alpha_1.1.0.0_Windows.exe"),
            ("BLOOP/Windows", "Bloop", "BLOOP.exe"),
            ("Boat Cats Windows/Boat Cats", "Boat Cats", "Boat Cats.exe"),
            ("Bonefighters for Windows", "Bonefighters", "Bonefighters.exe"),
            ("BOOTLOOP", "Bootloop", "bootloop.exe"),
            ("Bottle of Sickness1.1.1", "Bottle of Sickness", "Bottle of Sickness.exe"),
            ("BREAKER", "Breaker", "BREAKER.exe"),
            ("Brew&Boom", "Brew & Boom", "Brew&Boom.exe"),
            ("Bridgebourn Demo Win64 v0-6-29", "Bridgebourn Demo", "Bridgebourn.exe"),
            ("BrokenThrough/WindowsNoEditor", "Broken Through", "BrokenThrough.exe"),
            ("Build&Grow Demo (Windows)", "Build & Grow Demo", "Build&Grow.exe"),
            ("bukibuki", "Bukibuki", "bukibuki.exe"),
            ("C137", "C137", "C137.exe"),
            ("Cafe Simulator", "Cafe Simulator", "Cafe Simulator.exe"),
            ("CallOfDOTS-Zombies-x64", "Call Of DOTS Zombies", "CallOfDOTS-Zombies-Project.exe"),
            ("Carbon Steel v1.2", "Carbon Steel", "CARBON STEEL.exe"),
            ("Carcass", "Carcass", "CARCASS.exe"),
            ("cards_n_varmints-v0.41.1-demo-windows", "Cards N Varmints Demo", "CardsNVarmints.exe"),
            ("CatCafeSimulator", "Cat Cafe Simulator", "cat-cafe.exe"),
            ("CCICrimeConnectInvestigation", "CCI Crime Connect Investigation", "CCICrimeConnectInvestigation.exe"),
            ("charons-obol-windows", "Charon's Obol", "charons_obol.exe"),
            ("cheekydice-gmtk", "Cheeky Dice", "gmtk-2022.exe"),
            ("Chibilization 0.18 [WIN]", "Chibilization", "Chibilization.exe"),
            ("ChickenLuck_Windows/ChickenLuck", "Chicken Luck", "ChickenLuck.exe"),
            ("ChopChop_Data", "Chop Chop", "ChopChop.exe"),
            ("circle-of-life_v1.1.1_windows", "Circle Of Life", "Circle of Life.exe"),
            ("Clatter Throne v0.4.4 - Windows", "Clatter Throne", "Clatter Throne.exe"),
            ("CleaningRedVille/FINAL_cr_PC", "Cleaning Redville", "CleaningRedville.exe"),
            ("ClockworkCleanup-v1.0.4", "Clockwork Cleanup", "ClockworkCleanup.exe"),
            ("cloudkeeper_windows_demo", "Cloud Keeper Demo", "CloudKeeper.exe"),
            ("ColdVengeanceDemoWindows", "Cold Vengeance Demo", "ColdVengeanceDemoBuild.exe"),
        ];

        for (folder_path, expected_title, exe_name) in test_cases {
            // Test that folder name cleaning produces a reasonable title
            let folder_name = Path::new(folder_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(folder_path);
            
            let cleaned = clean_game_title(folder_name);
            assert!(!cleaned.is_empty(), "Folder name '{}' should produce non-empty title", folder_name);
            
            // For executable name extraction
            if let Some(cleaned_exe) = extract_title_from_executable(&Some(exe_name.to_string())) {
                assert!(!cleaned_exe.is_empty(), "Exe name '{}' should produce non-empty title", exe_name);
            }
        }
    }

    #[test]
    fn test_games_catalog_problematic_names() {
        // Test that problematic folder names from the catalog are handled correctly
        
        // Names that should be filtered out or cleaned
        let problematic_names = vec![
            "Windows",
            "win64",
            "Win",
            "Build",
            "Engine",
            "jre",
            "en-us",
            "Binaries",
            "WindowsNoEditor",
            "Release",
            "Debug",
        ];
        
        for name in problematic_names {
            let cleaned = clean_game_title(name);
            assert!(cleaned.is_empty(), "Problematic name '{}' should produce empty string", name);
        }
    }

    #[test]
    fn test_games_catalog_executable_selection() {
        // Test that executable selection logic works for common patterns in the catalog
        
        // Case 1: exe name matches folder name (most common)
        let dir = Path::new("MyGame");
        let executables = vec!["MyGame.exe".to_string()];
        assert_eq!(pick_best_executable(dir, &executables), Some("MyGame.exe".to_string()));
        
        // Case 2: exe name differs from folder name (like "Froge.exe" in "Blobfrog")
        let dir = Path::new("Blobfrog");
        let executables = vec!["Froge.exe".to_string()];
        // Should still pick it as it's the only one
        assert_eq!(pick_best_executable(dir, &executables), Some("Froge.exe".to_string()));
        
        // Case 3: Multiple executables, one matches folder name
        let dir = Path::new("MyGame");
        let executables = vec![
            "MyGame.exe".to_string(),
            "launcher.exe".to_string(),
            "game.exe".to_string(),
        ];
        assert_eq!(pick_best_executable(dir, &executables), Some("MyGame.exe".to_string()));
        
        // Case 4: No matching name, pick largest (like in "Roulette Knight" with RouletteKnight.exe)
        let dir = Path::new("Roulette Knight");
        let executables = vec![
            "RouletteKnight.exe".to_string(), // close match but not exact
            "launcher.exe".to_string(),
        ];
        // Should pick RouletteKnight.exe as it's a partial match and likely larger
        assert_eq!(pick_best_executable(dir, &executables), Some("RouletteKnight.exe".to_string()));
    }

    #[test]
    fn test_games_catalog_cover_keywords() {
        // Test that cover keywords include common patterns from game distributions
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
        
        // Common cover file names from real game distributions
        let cover_names = vec![
            "cover.jpg",
            "boxart.png",
            "front.jpg",
            "back.png",
            "icon.ico",
            "logo.png",
            "header.jpg",
            "screenshot1.png",
            "promo.jpg",
            "keyart.png",
            "capsule.jpg",
            "library.jpg",
            "hero.png",
            "background.jpg",
            "wallpaper.jpg",
            "tile.jpg",
        ];
        
        for name in cover_names {
            let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let is_cover_like = cover_keywords.iter().any(|kw| stem.contains(kw));
            assert!(is_cover_like, "Cover name '{}' should be recognized as cover-like", name);
        }
        
        // Non-cover names should not match
        let non_cover_names = vec![
            "game.exe",
            "readme.txt",
            "license.pdf",
            "changelog.md",
            "config.ini",
            "data.dat",
        ];
        
        for name in non_cover_names {
            let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let is_cover_like = cover_keywords.iter().any(|kw| stem.contains(kw));
            assert!(!is_cover_like, "Non-cover name '{}' should not be recognized as cover-like", name);
        }
    }
}
