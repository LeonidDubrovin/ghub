use crate::models::{Game, MetadataSearchResult};
use crate::metadata::{ItchStrategy, MetadataStrategy, SteamStrategy};
use crate::title_extraction::{is_generic_company_name, is_generic_description};
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

/// Try to infer a known source type from a URL.
fn infer_source_type(url: &str) -> Option<&'static str> {
    let lower = url.to_lowercase();
    if lower.contains("steampowered.com/app/") || lower.contains("store.steampowered.com/app/") {
        return Some("steam");
    }
    if lower.contains("itch.io") {
        return Some("itch");
    }
    None
}

/// Fetch exact metadata from the source page URL.
/// This is the primary metadata resolver; it does not perform any fuzzy search.
pub async fn fetch_metadata_by_url(
    client: &reqwest::Client,
    source_type: &str,
    url: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    match source_type {
        "itch" => {
            let strategy = ItchStrategy::new();
            strategy.get_details(client, url).await
        }
        "steam" => {
            let app_id = url.split("/app/").nth(1)
                .and_then(|s| s.split('/').next())
                .map(|s| s.to_string());
            if let Some(app_id) = app_id {
                let strategy = SteamStrategy::new();
                strategy.get_details(client, &app_id).await
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Tauri command wrapper for fetching exact metadata from a source URL.
#[tauri::command]
pub async fn fetch_metadata_by_url_command(
    state: State<'_, AppState>,
    source_type: String,
    url: String,
) -> Result<Option<MetadataSearchResult>, String> {
    fetch_metadata_by_url(&state.http_client, &source_type, &url).await
}

/// Apply metadata to a game while respecting existing fields.
fn apply_metadata(
    db: &crate::database::Database,
    game_id: &str,
    meta: &MetadataSearchResult,
) -> Result<(), String> {
    let game = db.get_game_by_id(game_id).map_err(|e| e.to_string())?;

    let new_title = if game.title.is_empty() || game.title == game_id {
        Some(meta.name.as_str())
    } else {
        None
    };

    let new_desc = if game.description.is_none() { meta.description.as_deref() } else { None };
    let new_dev = if game.developer.is_none() { meta.developer.as_deref() } else { None };
    let new_pub = if game.publisher.is_none() { meta.publisher.as_deref() } else { None };
    let new_cover = if game.cover_image.is_none() { meta.cover_url.as_deref() } else { None };

    db.update_game(
        game_id,
        new_title,
        new_desc,
        new_dev,
        new_pub,
        new_cover,
        None, // is_favorite
        None, // completion_status
        None, // user_rating
    ).map_err(|e| e.to_string())?;

    Ok(())
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


/// Fetch and update game metadata from the exact source page.
/// If the game has no known source link, the caller should open the manual search dialog.
#[tauri::command]
pub async fn fetch_and_update_game_metadata(state: State<'_, AppState>, game_id: String) -> Result<Game, String> {
    info!("🔍 fetch_and_update_game_metadata called for game_id: {}", game_id);

    // Load the game and its source links while dropping the lock before await.
    let (source_type, source_url) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
        let links = db.get_game_links(&game_id).map_err(|e| e.to_string())?;

        // Prefer a typed game_link (itch/steam) with a known source_type.
        let typed_link = links.iter()
            .find(|l| matches!(l.source_type.as_deref(), Some("itch") | Some("steam")) && !l.url.is_empty())
            .map(|l| (l.source_type.clone().unwrap(), l.url.clone()));

        typed_link
            .or_else(|| {
                // Otherwise try to infer from the legacy external_link field.
                game.external_link.as_ref()
                    .and_then(|url| infer_source_type(url).map(|st| (st.to_string(), url.clone())))
            })
    }
    .ok_or_else(|| {
        warn!("   ⚠️ No source link found for game {}", game_id);
        "No source link found".to_string()
    })?;

    info!("   Fetching exact metadata from {} source: {}", source_type, source_url);

    let client = &state.http_client;
    let best_match = fetch_metadata_by_url(client, &source_type, &source_url).await?;

    if let Some(meta) = best_match {
        info!("   Applying metadata: {}", meta.name);
        let db = state.db.lock().map_err(|e| e.to_string())?;
        apply_metadata(&db, &game_id, &meta)?;
        info!("   ✅ Metadata updated successfully");
    } else {
        warn!("   ⚠️ Could not fetch metadata from source page");
    }

    // Return updated game
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_game_by_id(&game_id).map_err(|e| e.to_string())
}

