use crate::models::{ScannedGame, ExeMetadata};
use crate::AppState;
use tauri::State;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Image extensions to look for covers/icons
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ico", "bmp", "webp"];
const COVER_KEYWORDS: &[&str] = &["cover", "poster", "banner", "icon", "logo", "header", "art", "thumb", "image"];

// EXE names to skip during scanning
const SKIP_EXE_PATTERNS: &[&str] = &[
    "unins", "setup", "redist", "vcredist", "dxsetup", "directx",
    "dotnet", "crashreport", "crash", "launcher", "updater", "ue4prereq",
    "installerdata", "physx", "steamcmd", "easyanticheat", "battleye"
];

#[tauri::command]
pub fn scan_space_sources(state: State<AppState>, space_id: String) -> Result<Vec<ScannedGame>, String> {
    println!("🔍 scan_space_sources called with space_id: {}", space_id);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_active_sources_for_space(&space_id).map_err(|e| e.to_string())?;
    
    println!("   Found {} active source(s) for space {}", sources.len(), space_id);
    for (sp_id, source_path) in &sources {
        println!("   • Source: {} -> {}", sp_id, source_path);
        let path = Path::new(source_path);
        println!("     Exists? {}", path.exists());
    }

    if sources.is_empty() {
        println!("   ⚠️ No active sources found - returning empty vector");
        return Ok(vec![]);
    }

    let mut all_games: Vec<ScannedGame> = Vec::new();

    for (_, source_path) in sources {
        println!("   📁 Processing source: {}", source_path);
        let path = Path::new(&source_path);
        
        if !path.exists() {
            println!("   ⚠️ Source path does not exist: {}", source_path);
            continue;
        }
        
        println!("   🔎 Scanning directory: {}", source_path);
        match scan_directory_internal(path) {
            Ok(mut games) => {
                println!("   ✅ Found {} games in {}", games.len(), source_path);
                for game in &games {
                    println!("      🎮 '{}' (path: {})", game.title, game.path);
                    if let Some(exe) = &game.executable {
                        println!("         └─ Executable: {}", exe);
                    }
                }
                all_games.append(&mut games);
            }
            Err(e) => {
                println!("   ❌ Scan error in {}: {}", source_path, e);
            }
        }
    }

    // Deduplicate by path
    all_games.sort_by(|a, b| a.path.cmp(&b.path));
    all_games.dedup_by(|a, b| a.path == b.path);

    println!("✅ Total unique games found: {}", all_games.len());
    Ok(all_games)
}

#[tauri::command]
pub fn scan_directory(path: String) -> Result<Vec<ScannedGame>, String> {
    println!("🔍 scan_directory called with path: {}", path);
    let base_path = Path::new(&path);

    if !base_path.exists() {
        return Err("Directory does not exist".to_string());
    }

    let result = scan_directory_internal(base_path).map_err(|e| e.to_string());
    println!("   scan_directory result: {:?}", result.as_ref().map(|games| games.len()));
    result
}

/// Internal scan function that doesn't require a full path string
pub fn scan_directory_internal(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    println!("      [scan_directory_internal] base_path: {}", base_path.display());
    
    let mut games: Vec<ScannedGame> = Vec::new();

    // Scan first-level subdirectories
    let entries = std::fs::read_dir(base_path).map_err(|e| format!("Failed to read dir: {}", e))?;
    let dirs: Vec<_> = entries
        .filter_map(|entry| {
            let e = entry.ok()?;
            let path = e.path();
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    println!("      [scan] Found {} first-level subdirectories", dirs.len());
    
    if dirs.is_empty() {
        println!("      [scan] No subdirectories found in {}", base_path.display());
        return Ok(vec![]);
    }

    for path in dirs {
        println!("      [scan] Checking subdirectory: {}", path.display());
        
        // Check if folder has only one subfolder and no exe - dive deeper
        let game_path = find_actual_game_folder(&path);
        println!("      [scan] Game folder resolved to: {}", game_path.display());

        let dir_name = game_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Find ALL executables
        let all_executables = find_all_executables(&game_path);
        println!("      [scan] Found {} executables", all_executables.len());
        for exe in &all_executables {
            println!("         - {}", exe);
        }

        // Pick the best executable
        let executable = pick_best_executable(&game_path, &all_executables);
        if let Some(exe) = &executable {
            println!("      [scan] Selected executable: {}", exe);
        } else {
            println!("      [scan] No suitable executable selected");
        }

        // Find cover/icon candidates
        let cover_candidates = find_cover_candidates(&game_path);
        if !cover_candidates.is_empty() {
            println!("      [scan] Found {} cover candidates", cover_candidates.len());
        }

        // Find icon
        let icon_path = find_icon(&game_path);
        if let Some(icon) = &icon_path {
            println!("      [scan] Found icon: {}", icon);
        }

        // Calculate folder size
        let size_bytes = calculate_dir_size(&game_path);
        println!("      [scan] Folder size: {} bytes", size_bytes);

        // Extract exe metadata (if we have an executable)
        let exe_metadata = executable.as_ref()
            .and_then(|exe| extract_exe_metadata(&game_path.join(exe)));
        if let Some(meta) = &exe_metadata {
            println!("      [scan] Exe metadata: product='{}', company='{}'", 
                meta.product_name.as_deref().unwrap_or("n/a"),
                meta.company_name.as_deref().unwrap_or("n/a"));
        }

        // Enhanced title extraction with multi-level fallback
        // Priority: Folder name > Parent folder > EXE metadata > Executable name
        let title = {
            // Level 1: Try cleaned directory name (most reliable for local games)
            if let Some(cleaned) = get_non_empty_title(clean_game_title(&dir_name)) {
                println!("      [title] Using cleaned dir name: '{}'", cleaned);
                cleaned
            }
            // Level 2: Try parent directory (up to 3 levels up)
            else if let Some(parent_title) = find_title_in_parents(&game_path, 3) {
                println!("      [title] Using parent dir: '{}'", parent_title);
                parent_title
            }
            // Level 3: Try metadata product name (only if not generic AND not in our blacklist)
            else if let Some(meta_name) = exe_metadata.as_ref()
                .and_then(|m| m.product_name.clone())
                .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
            {
                println!("      [title] Using metadata product name: '{}'", meta_name);
                meta_name
            }
            // Level 4: Try to extract from best executable name
            else if let Some(exe_name) = extract_title_from_executable(&executable) {
                println!("      [title] Using executable name: '{}'", exe_name);
                exe_name
            }
            // Level 5: Try metadata company name as last resort (only if not generic)
            else if let Some(company) = exe_metadata.as_ref()
                .and_then(|m| m.company_name.clone())
                .filter(|name| !is_generic_exe_name(name) && !is_problematic_game_name(name))
            {
                println!("      [title] Using company name: '{}'", company);
                company
            }
            // Final fallback: use original dir name or "Unknown Game"
            else {
                let fallback = if dir_name != "Unknown" { dir_name.clone() } else { "Unknown Game".to_string() };
                println!("      [title] Using fallback: '{}'", fallback);
                fallback
            }
        };
        println!("      [scan] Final title: '{}'", title);

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
        println!("      [find_actual_game_folder] {} has exe directly, using it", dir.display());
        return dir.to_path_buf();
    }

    // No exe in current folder - search in subdirectories (up to 3 levels deep)
    if let Some(found) = find_folder_with_exe(dir, 3) {
        println!("      [find_actual_game_folder] Found exe in subfolder: {}", found.display());
        return found;
    }

    println!("      [find_actual_game_folder] No exe found, returning original dir: {}", dir.display());
    dir.to_path_buf()
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
        println!("      [has_exe_files] {} contains exe files", dir.display());
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
        if has_exe_files(&subdir) {
            return Some(subdir);
        }
    }

    // If no direct subfolder has exe, search deeper
    for entry in &entries {
        let subdir = entry.path();
        if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1) {
            return Some(found);
        }
    }

    None
}

/// Find all executable files in directory (including subdirs up to 2 levels)
fn find_all_executables(dir: &Path) -> Vec<String> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_str().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let name_lower = name.to_lowercase();

                    // Skip known non-game executables
                    let should_skip = SKIP_EXE_PATTERNS.iter()
                        .any(|pattern| name_lower.contains(pattern));

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
            println!("      [pick_best] Priority 1 match: '{}' matches folder '{}'", exe, dir_name);
            return Some(exe.clone());
        }
    }

    // Priority 2: exe in root folder (not subdir)
    for exe in executables {
        if !exe.contains('\\') && !exe.contains('/') {
            println!("      [pick_best] Priority 2 match: '{}' is in root", exe);
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
        println!("      [pick_best] Priority 3: selected largest '{}' ({} bytes)", exe, size);
    }
    best.map(|(exe, _)| exe)
}

/// Find potential cover/icon images
fn find_cover_candidates(dir: &Path) -> Vec<String> {
    let mut candidates = Vec::new();

    // Search in root and common subdirs
    let search_paths = [
        dir.to_path_buf(),
        dir.join("images"),
        dir.join("art"),
        dir.join("assets"),
        dir.join("media"),
    ];

    for search_path in &search_paths {
        if !search_path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(search_path) {
            for entry in entries.filter_map(|e| e.ok()) {
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
                let is_cover_like = COVER_KEYWORDS.iter().any(|kw| name.contains(kw));

                let relative = path.strip_prefix(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());

                if is_cover_like {
                    candidates.insert(0, relative);
                } else {
                    candidates.push(relative);
                }
            }
        }
    }

    // Limit to 10 candidates
    candidates.truncate(10);
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

        // Try to get ProductName
        metadata.product_name = query_version_string(&buffer, "ProductName");
        metadata.company_name = query_version_string(&buffer, "CompanyName");
        metadata.file_description = query_version_string(&buffer, "FileDescription");
        metadata.file_version = query_version_string(&buffer, "FileVersion");

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
    let generic_names = [
        "Godot Engine", "BootstrapPackagedGame", "Unity", "Unreal Engine",
        "Game", "Windows", "Launcher", "Setup", "Installer", "Updater",
        "CrashReport", "Crash Handler", "Unity Player", "UE4 Game",
        "Game Launcher", "Application", "App", "ICARUS", "Life Makeover",
        "Microphage", "WindowsNoEditor", "Win64", "Win32", "Shipping",
        "Development", "Debug", "Release"
    ];
    
    let name_lower = name.to_lowercase();
    for generic in &generic_names {
        if name_lower == generic.to_lowercase() {
            return true;
        }
    }
    
    // Check if name is too short or just version numbers
    if name.len() < 3 || name.chars().all(|c| c.is_numeric() || c == '.' || c == '_') {
        return true;
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