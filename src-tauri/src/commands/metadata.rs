use crate::models::{Game, MetadataSearchResult};
use crate::AppState;
use crate::meta_service;
use crate::commands::scanning::scan_directory_internal;
use tauri::State;
use std::path::Path;

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
    let _game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    
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
    
    // Get game info first (need to drop lock before await)
    let (original_title, install_path) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
        let installs = db.get_installs_for_game(&game_id).map_err(|e| e.to_string())?;
        let install_path = installs.first().map(|i| i.install_path.clone());
        (game.title.clone(), install_path)
    };
    
    // Use install path directory name as search query if available, otherwise use title
    let query = if let Some(path) = &install_path {
        let path = Path::new(path);
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            // Clean the directory name for better search results
            let cleaned = clean_game_title(dir_name);
            if !cleaned.is_empty() {
                println!("   Using directory name for search: {}", cleaned);
                cleaned
            } else {
                println!("   Using original title for search: {}", original_title);
                original_title.clone()
            }
        } else {
            println!("   Using original title for search: {}", original_title);
            original_title.clone()
        }
    } else {
        println!("   Using original title for search: {}", original_title);
        original_title.clone()
    };
    
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
        
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
        
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
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}

/// Helper function to clean game title (used by fetch_and_update_game_metadata)
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