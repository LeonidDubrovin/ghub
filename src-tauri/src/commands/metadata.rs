use crate::models::{Game, MetadataSearchResult};
use crate::title_extraction::{clean_game_title, is_generic_company_name, is_generic_description, is_generic_title};
use crate::AppState;
use crate::commands::scanning::scan_directory_internal;
use tauri::State;
use std::path::Path;
use log::{debug, info, warn};

/// Search game metadata from sources
#[tauri::command]
pub async fn search_game_metadata(
    state: State<'_, AppState>,
    query: String,
    sources: Vec<String>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let client = &state.http_client;

    let sources_refs: Vec<&str> = if sources.is_empty() {
        vec!["steam", "itch"]
    } else {
        sources.iter().map(|s| s.as_str()).collect()
    };

    let results = state
        .metadata_aggregator
        .search_sources(client, &query, &sources_refs)
        .await;

    Ok(results)
}

/// Refresh game data from local directory
#[tauri::command]
pub fn refresh_game_from_local(state: State<AppState>, game_id: String) -> Result<Game, String> {
    info!("🔄 refresh_game_from_local called for game_id: {}", game_id);
    
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
    
    debug!("   Scanning directory: {}", game_path.display());
    
    // Scan the directory to get fresh data
    let scanned_games = scan_directory_internal(game_path).map_err(|e| e.to_string())?;
    
    if scanned_games.is_empty() {
        return Err("No game found in directory".to_string());
    }
    
    let scanned = &scanned_games[0];
    
    // Update the game with fresh data from local directory ONLY
    // IMPORTANT: Do NOT use exe metadata product name if it's generic
    let title = if !scanned.title.is_empty() {
        Some(scanned.title.as_str())
    } else {
        None
    };
    
    // Only use developer from exe metadata if it's not generic
    let developer = scanned.exe_metadata.as_ref()
        .and_then(|m| m.company_name.as_deref())
        .filter(|name| !is_generic_company_name(name));
    
    // Only use description from exe metadata if it's not generic
    let description = scanned.exe_metadata.as_ref()
        .and_then(|m| m.file_description.as_deref())
        .filter(|desc| !is_generic_description(desc));
    
    // Update executable path if found
    let executable_path = scanned.executable.as_deref();
    
    // Update the game in database - RESET all metadata fields to force fresh local data
    // Use update_game_with_reset to properly set fields to NULL when None is passed
    db.update_game_with_reset(
        &game_id,
        title,
        description,
        developer,
        None, // publisher - reset to None to get fresh data
        None, // cover_image - reset to None to get fresh data
        None, // is_favorite - keep existing
        None, // completion_status - keep existing
        None, // user_rating - keep existing
    ).map_err(|e| e.to_string())?;
    
    // Update install with new executable path if found
    if let Some(exe_path) = executable_path {
        db.update_install_executable(&install.id, exe_path).map_err(|e| e.to_string())?;
    }
    
    info!("   ✅ Game refreshed successfully");
    
    // Return updated game
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}


/// Fetch and update game metadata from external sources (Steam, itch.io)
#[tauri::command]
pub async fn fetch_and_update_game_metadata(state: State<'_, AppState>, game_id: String) -> Result<Game, String> {
    info!("🔍 fetch_and_update_game_metadata called for game_id: {}", game_id);
    
    // Get game info first (need to drop lock before await)
    let (original_title, install_path) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
        let installs = db.get_installs_for_game(&game_id).map_err(|e| e.to_string())?;
        let install_path = installs.first().map(|i| i.install_path.clone());
        (game.title.clone(), install_path)
    };
    
    // Determine search query with priority: exe metadata > game title > directory name
    let query = {
        // Priority 1: Try to get title from exe metadata (most reliable)
        if let Some(path) = &install_path {
            let game_path = Path::new(path);
            if game_path.exists() {
                // Scan directory to get exe metadata
                if let Ok(scanned_games) = scan_directory_internal(game_path) {
                    if let Some(scanned) = scanned_games.first() {
                        // Use exe product name if available and not generic
                        if let Some(exe_meta) = &scanned.exe_metadata {
                            if let Some(product_name) = &exe_meta.product_name {
                                let cleaned = clean_game_title(product_name);
                                if !cleaned.is_empty() && !is_generic_title(&cleaned) {
                                    debug!("   Using exe metadata product name for search: {}", cleaned);
                                    cleaned
                                } else {
                                    // Fall through to next priority
                                    String::new()
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    };
    
    // If we got a query from exe metadata, use it
    let query = if !query.is_empty() {
        query
    } else {
        // Priority 2: Use game's existing title from database
        let cleaned_title = clean_game_title(&original_title);
        if !cleaned_title.is_empty() && !is_generic_title(&cleaned_title) {
            debug!("   Using game title for search: {}", cleaned_title);
            cleaned_title
        } else if let Some(path) = &install_path {
            // Priority 3: Fall back to directory name (least reliable)
            let path = Path::new(path);
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                let cleaned = clean_game_title(dir_name);
                if !cleaned.is_empty() && !is_generic_title(&cleaned) {
                    debug!("   Using directory name for search: {}", cleaned);
                    cleaned
                } else {
                    debug!("   Using original title for search: {}", original_title);
                    original_title.clone()
                }
            } else {
                debug!("   Using original title for search: {}", original_title);
                original_title.clone()
            }
        } else {
            debug!("   Using original title for search: {}", original_title);
            original_title.clone()
        }
    };
    
    info!("   Searching for: {}", query);
    
    // Search for metadata from external sources
    let client = &state.http_client;
    let best_match = state.metadata_aggregator.search_best(client, &query).await;
    if let Some(ref meta) = best_match {
        debug!("   Found best match on {}: {}", meta.source, meta.name);
    }
    
    // Apply metadata if found
    if let Some(meta) = best_match {
        info!("   Applying metadata: {}", meta.name);
        
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
        
        info!("   ✅ Metadata updated successfully");
    } else {
        warn!("   ⚠️ No metadata found");
    }
    
    // Return updated game
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}

