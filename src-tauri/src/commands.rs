use crate::models::{Space, Game, DownloadLink, Setting, CreateSpaceRequest, CreateGameRequest, CreateGameLinkRequest, UpdateGameRequest, ScannedGame, ExeMetadata, MetadataSearchResult, SpaceSource};
use crate::AppState;
use crate::meta_service;
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
pub fn get_all_spaces(state: State<AppState>) -> Result<Vec<Space>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_spaces().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_space(state: State<AppState>, request: CreateSpaceRequest) -> Result<Space, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    db.create_space(
        &id,
        &request.name,
        None, // Space path is deprecated, use space_sources instead
        &request.space_type,
        request.icon.as_deref(),
        request.color.as_deref(),
    ).map_err(|e| e.to_string())?;

    // If initial_sources provided, add them
    if let Some(sources) = request.initial_sources {
        for source_path in sources {
            println!("➕ Adding source to space {}: {}", id, source_path);
            let _ = db.add_space_source(&id, &source_path, true);
        }
    }

    db.get_space_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_space(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_space(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_space_sources(state: State<AppState>, space_id: String) -> Result<Vec<SpaceSource>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_space_sources(&space_id).map_err(|e| e.to_string())?;
    println!("📚 get_space_sources for {}: {} sources", space_id, sources.len());
    Ok(sources)
}

#[tauri::command]
pub fn add_space_source(state: State<AppState>, space_id: String, source_path: String, scan_recursively: Option<bool>) -> Result<(), String> {
    println!("➕ add_space_source: space={}, path={}, recursive={:?}", space_id, source_path, scan_recursively);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_space_source(&space_id, &source_path, scan_recursively.unwrap_or(true)).map_err(|e| e.to_string())?;
    println!("   ✅ Source added successfully");
    Ok(())
}

#[tauri::command]
pub fn remove_space_source(state: State<AppState>, space_id: String, source_path: String) -> Result<(), String> {
    println!("➖ remove_space_source: space={}, path={}", space_id, source_path);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_space_source(&space_id, &source_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_space_source(state: State<AppState>, space_id: String, source_path: String, is_active: Option<bool>, scan_recursively: Option<bool>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_space_source(&space_id, &source_path, is_active.unwrap_or(true), scan_recursively).map_err(|e| e.to_string())?;
    Ok(())
}

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

/// Internal scan function that doesn't require a full path string
fn scan_directory_internal(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
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
        let title = {
            // Level 1: Try metadata product name (if not generic)
            if let Some(meta_name) = exe_metadata.as_ref()
                .and_then(|m| m.product_name.clone())
                .filter(|name| !is_generic_exe_name(name))
            {
                println!("      [title] Using metadata product name: '{}'", meta_name);
                meta_name
            } 
            // Level 2: Try cleaned directory name
            else if let Some(cleaned) = get_non_empty_title(clean_game_title(&dir_name)) {
                println!("      [title] Using cleaned dir name: '{}'", cleaned);
                cleaned
            }
            // Level 3: Try parent directory (up to 3 levels up)
            else if let Some(parent_title) = find_title_in_parents(&game_path, 3) {
                println!("      [title] Using parent dir: '{}'", parent_title);
                parent_title
            }
            // Level 4: Try to extract from best executable name
            else if let Some(exe_name) = extract_title_from_executable(&executable) {
                println!("      [title] Using executable name: '{}'", exe_name);
                exe_name
            }
            // Level 5: Try metadata company name as last resort
            else if let Some(company) = exe_metadata.as_ref()
                .and_then(|m| m.company_name.clone())
                .filter(|name| !is_generic_exe_name(name))
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

#[tauri::command]
pub fn get_all_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_games().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_games_by_space(state: State<AppState>, space_id: String) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_games_by_space(&space_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_game(state: State<'_, AppState>, request: CreateGameRequest) -> Result<Game, String> {
    let game_id = uuid::Uuid::new_v4().to_string();
    let install_id = uuid::Uuid::new_v4().to_string();

    // Scope for DB lock
    let game = {
        let db = state.db.lock().map_err(|e| e.to_string())?;

        // Create game
        let game = db.create_game(
            &game_id,
            &request.title,
            request.description.as_deref(),
            request.developer.as_deref(),
            request.cover_image.as_deref(),
            None, // external_link
        ).map_err(|e| e.to_string())?;

        // Create install
        db.create_install(
            &install_id,
            &game_id,
            &request.space_id,
            &request.install_path,
            request.executable_path.as_deref(),
        ).map_err(|e| e.to_string())?;

        game
    };

    // Auto-fetch metadata if requested
    if request.fetch_metadata.unwrap_or(false) {
        let client = &state.http_client;
        let query = request.title.clone();

        // Try Steam first
        let mut best_match: Option<MetadataSearchResult> = None;

        if let Ok(results) = meta_service::search_steam(client, &query).await {
            if let Some(first) = results.into_iter().next() {
                best_match = Some(first);
            }
        }

        // If no steam result, try Itch
        if best_match.is_none() {
            if let Ok(results) = meta_service::search_itch(client, &query).await {
                if let Some(first) = results.into_iter().next() {
                    best_match = Some(first);
                }
            }
        }

        // Apply metadata if found
        if let Some(meta) = best_match {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            let new_desc = if request.description.is_none() { meta.description.as_deref() } else { request.description.as_deref() };
            let new_dev = if request.developer.is_none() { meta.developer.as_deref() } else { request.developer.as_deref() };
            let new_pub = if request.developer.is_none() { meta.publisher.as_deref() } else { None };
            let new_cover = if request.cover_image.is_none() { meta.cover_url.as_deref() } else { request.cover_image.as_deref() };

            db.update_game(
                &game_id,
                Some(&meta.name),
                new_desc,
                new_dev,
                new_pub,
                new_cover,
                Some(false),
                None,
                None,
            ).map_err(|e| e.to_string())?;
        }
    }

    // Return the (possibly updated) game
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_game_by_id(&game_id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn create_game_link(state: State<'_, AppState>, request: CreateGameLinkRequest) -> Result<Game, String> {
    let game_id = uuid::Uuid::new_v4().to_string();

    // Auto-fill title if missing (simple fallback)
    let title = request.title.unwrap_or_else(|| "New Link".to_string());

    // Create game without install
    let mut game = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_game(
            &game_id,
            &title,
            Some("External Link"),
            None,
            None,
            Some(&request.url),
        ).map_err(|e| e.to_string())?
    };

    // Auto-fetch metadata
    let client = &state.http_client;
    let query = title.clone();

    let mut best_match: Option<MetadataSearchResult> = None;

    if let Ok(results) = meta_service::search_itch(client, &query).await {
        if let Some(first) = results.into_iter().next() {
            best_match = Some(first);
        }
    }

    if best_match.is_none() {
        if let Ok(results) = meta_service::search_steam(client, &query).await {
            if let Some(first) = results.into_iter().next() {
                best_match = Some(first);
            }
        }
    }

    if let Some(meta) = best_match {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.update_game(
            &game_id,
            Some(&meta.name),
            meta.description.as_deref(),
            meta.developer.as_deref(),
            meta.publisher.as_deref(),
            meta.cover_url.as_deref(),
            Some(false),
            Some("on_hold"),
            None,
        ).map_err(|e| e.to_string())?;

        game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    }

    Ok(game)
}

#[tauri::command]
pub fn update_game(state: State<AppState>, request: UpdateGameRequest) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_game(
        &request.id,
        request.title.as_deref(),
        request.description.as_deref(),
        request.developer.as_deref(),
        request.publisher.as_deref(),
        request.cover_image.as_deref(),
        request.is_favorite,
        request.completion_status.as_deref(),
        request.user_rating,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_game(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_game(&id).map_err(|e| e.to_string())
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
        "Game Launcher", "Application", "App"
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

fn clean_game_title(name: &str) -> String {
    // Remove common suffixes/prefixes
    let mut title = name.to_string();

    // Remove version numbers like v1.0, 1.0.0, V1.1_NEW, v012, etc.
    let re_version = regex_lite::Regex::new(r"[\s_]*(?:[vV]\d+(?:[\._]\d+)*|\d+(?:[\._]\d+)+).*$").ok();
    if let Some(re) = re_version {
        title = re.replace(&title, "").to_string();
    }

    // Remove platform tags
    for tag in &["(Windows)", "(PC)", "(GOG)", "(Steam)", "[GOG]", "[Steam]", "(Mac)", "(Linux)"] {
        title = title.replace(tag, "");
    }

    // Remove common generic folder names that shouldn't be game titles
    let generic_names = [
        "Windows", "BootstrapPackagedGame", "Godot Engine", "Unity", "Unreal",
        "Game", "Build", "Release", "Bin", "Binary", "Executable", "App",
        "win64", "win32", "linux", "macos", "x64", "x86"
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

#[tauri::command]
pub fn launch_game(state: State<AppState>, game_id: String, space_id: String) -> Result<String, String> {
    let install = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_install(&game_id, &space_id)
            .map_err(|e| e.to_string())?
            .ok_or("Install not found")?
    };

    let executable = install.executable_path.ok_or("No executable path set")?;
    let full_path = Path::new(&install.install_path).join(&executable);

    if !full_path.exists() {
        return Err(format!("Executable not found: {}", full_path.display()));
    }

    // Spawn the game process
    let child = std::process::Command::new(&full_path)
        .current_dir(&install.install_path)
        .spawn()
        .map_err(|e| e.to_string())?;

    let pid = child.id();

    // Start playtime tracking
    let playtime = state.playtime.lock().map_err(|e| e.to_string())?;
    let session_id = playtime.start_session(&game_id, Some(&install.id), pid)?;

    Ok(session_id)
}

#[tauri::command]
pub fn get_active_sessions(state: State<AppState>) -> Result<Vec<(String, String, i64)>, String> {
    let playtime = state.playtime.lock().map_err(|e| e.to_string())?;
    Ok(playtime.get_active_sessions())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Vec<Setting>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_setting(state: State<AppState>, key: String, value: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_download_links(state: State<AppState>) -> Result<Vec<DownloadLink>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_download_links().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_download_link(state: State<'_, AppState>, url: String) -> Result<DownloadLink, String> {
    let title = url.split('/').last().unwrap_or("Unknown Link").replace('-', " ").replace('_', " ");

    let mut link = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_download_link(&url, &title, None, None).map_err(|e| e.to_string())?
    };

    // Attempt to fetch metadata if it's a known store
    let client = &state.http_client;
    let mut meta: Option<MetadataSearchResult> = None;

    if url.contains("store.steampowered.com") {
        if let Ok(results) = meta_service::search_steam(client, &title).await {
            meta = results.into_iter().next();
        }
    } else if url.contains("itch.io") {
        if let Ok(results) = meta_service::search_itch(client, &title).await {
            meta = results.into_iter().next();
        }
    }

    Ok(link)
}

#[tauri::command]
pub fn delete_download_link(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_download_link(&id).map_err(|e| e.to_string())
}

/// Search game metadata from sources
#[tauri::command]
pub async fn search_game_metadata(state: State<'_, AppState>, query: String, sources: Vec<String>) -> Result<Vec<MetadataSearchResult>, String> {
    let client = &state.http_client;
    let mut results = Vec::new();

    let use_steam = sources.is_empty() || sources.contains(&"steam".to_string());
    let use_itch = sources.is_empty() || sources.contains(&"itch".to_string());

    if let Some(fut) = use_steam.then(|| meta_service::search_steam(client, &query)) {
        match fut.await {
            Ok(r) => results.extend(r),
            Err(e) => println!("Steam search error: {}", e),
        }
    }

    if let Some(fut) = use_itch.then(|| meta_service::search_itch(client, &query)) {
        match fut.await {
            Ok(r) => results.extend(r),
            Err(e) => println!("Itch search error: {}", e),
        }
    }

    Ok(results)
}

/// Refresh game data from local directory
#[tauri::command]
pub fn refresh_game_from_local(state: State<AppState>, game_id: String) -> Result<Game, String> {
    println!("🔄 refresh_game_from_local called for game_id: {}", game_id);
    
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get the game and its install info
    let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    
    // Get the install path for this game
    let installs = db.get_installs_for_game(&game_id).map_err(|e| e.to_string())?;
    
    if installs.is_empty() {
        return Err("No install found for this game".to_string());
    }
    
    // Use the first install path
    let install = &installs[0];
    let game_path = Path::new(&install.install_path);
    
    if !game_path.exists() {
        return Err(format!("Game directory does not exist: {}", install.install_path));
    }
    
    println!("   Scanning directory: {}", game_path.display());
    
    // Scan the directory to get fresh data
    let scanned_games = scan_directory_internal(game_path).map_err(|e| e.to_string())?;
    
    if scanned_games.is_empty() {
        return Err("No game found in directory".to_string());
    }
    
    let scanned = &scanned_games[0];
    
    // Update the game with fresh data from local directory
    let title = if !scanned.title.is_empty() {
        Some(scanned.title.as_str())
    } else {
        None
    };
    
    let developer = scanned.exe_metadata.as_ref()
        .and_then(|m| m.company_name.as_deref());
    
    let description = scanned.exe_metadata.as_ref()
        .and_then(|m| m.file_description.as_deref());
    
    // Update executable path if found
    let executable_path = scanned.executable.as_deref();
    
    // Update the game in database
    db.update_game(
        &game_id,
        title,
        description,
        developer,
        None, // publisher
        None, // cover_image - keep existing
        None, // is_favorite - keep existing
        None, // completion_status - keep existing
        None, // user_rating - keep existing
    ).map_err(|e| e.to_string())?;
    
    // Update install with new executable path if found
    if let Some(exe_path) = executable_path {
        db.update_install_executable(&install.id, exe_path).map_err(|e| e.to_string())?;
    }
    
    println!("   ✅ Game refreshed successfully");
    
    // Return updated game
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}

/// Fetch and update game metadata from external sources (Steam, itch.io)
#[tauri::command]
pub async fn fetch_and_update_game_metadata(state: State<'_, AppState>, game_id: String) -> Result<Game, String> {
    println!("🔍 fetch_and_update_game_metadata called for game_id: {}", game_id);
    
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    
    let query = game.title.clone();
    println!("   Searching for: {}", query);
    
    // Search for metadata from external sources
    let client = &state.http_client;
    let mut best_match: Option<MetadataSearchResult> = None;
    
    // Try Steam first
    if let Ok(results) = meta_service::search_steam(client, &query).await {
        if let Some(first) = results.into_iter().next() {
            println!("   Found Steam result: {}", first.name);
            best_match = Some(first);
        }
    }
    
    // If no steam result, try Itch
    if best_match.is_none() {
        if let Ok(results) = meta_service::search_itch(client, &query).await {
            if let Some(first) = results.into_iter().next() {
                println!("   Found Itch result: {}", first.name);
                best_match = Some(first);
            }
        }
    }
    
    // Apply metadata if found
    if let Some(meta) = best_match {
        println!("   Applying metadata: {}", meta.name);
        
        let new_desc = if game.description.is_none() { meta.description.as_deref() } else { None };
        let new_dev = if game.developer.is_none() { meta.developer.as_deref() } else { None };
        let new_pub = if game.publisher.is_none() { meta.publisher.as_deref() } else { None };
        let new_cover = if game.cover_image.is_none() { meta.cover_url.as_deref() } else { None };
        
        db.update_game(
            &game_id,
            Some(&meta.name),
            new_desc,
            new_dev,
            new_pub,
            new_cover,
            None, // is_favorite - keep existing
            None, // completion_status - keep existing
            None, // user_rating - keep existing
        ).map_err(|e| e.to_string())?;
        
        println!("   ✅ Metadata updated successfully");
    } else {
        println!("   ⚠️ No metadata found");
    }
    
    // Return updated game
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}
