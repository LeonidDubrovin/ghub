/// Shared scanning logic used by both synchronous and background scanning implementations.
/// Provides a single source of truth for all scanning operations.

use crate::models::ScannedGame;
use crate::title_extraction::{extract_title_with_fallback, read_local_metadata};
use log::debug;
use regex::Regex;
use rusqlite::params;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
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
    let mut games = Vec::new();
    let mut scanned_dirs = HashSet::new();

    let max_depth = config.max_scan_depth;

    for entry in WalkDir::new(base_path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        // Check cancellation if flag provided
        if let Some(flag) = cancel_flag {
            if flag.load(Ordering::SeqCst) {
                return Err("Scan cancelled".to_string());
            }
        }

        let entry_path = entry.path();
        if !entry_path.is_dir() {
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
            debug!("Skipping excluded folder: {}", entry_path.display());
            continue;
        }

        // Check if directory has executables
        if !has_executable_files(entry_path) {
            continue;
        }

        debug!("Found game folder: {}", entry_path.display());

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
        let title = extract_title_with_fallback(
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
    Ok((games, games_count))
}

/// Check if folder name matches exclusion patterns
fn is_folder_excluded(dir_name: &str, patterns: &[Regex]) -> bool {
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
    fn test_has_executable_files() {
        let temp_dir = std::env::temp_dir();
        // This test would need a real directory with exe files to be meaningful
        // For now, just test that function compiles and runs
        let result = has_executable_files(&temp_dir);
        // Just checking it doesn't panic
        let _ = result;
    }
}
