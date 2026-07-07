use crate::models::{Game, MetadataSearchResult, ScannedGame};
use crate::metadata::{ItchStrategy, MetadataStrategy, SteamStrategy};
use crate::title_extraction::{is_generic_company_name, is_generic_description};
use crate::AppState;
use crate::itch_api::ItchApiClient;
use tauri::{AppHandle, State};
use std::path::Path;
use log::{debug, info, warn};

/// Search game metadata from sources.
/// For itch.io, uses the authenticated search API when the user has stored an API key;
/// otherwise falls back to the public/aggregator strategy.
#[tauri::command]
pub async fn search_game_metadata(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
    sources: Vec<String>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let client = &state.http_client;
    info!("🔍 search_game_metadata: query='{}', sources={:?}", query, sources);

    let sources_refs: Vec<&str> = if sources.is_empty() {
        vec!["steam", "itch"]
    } else {
        sources.iter().map(|s| s.as_str()).collect()
    };

    let mut results = Vec::new();

    // Authenticated itch search has priority because it is more reliable.
    if sources_refs.contains(&"itch") {
        match crate::commands::itch_api::get_itch_api_key(app.clone()) {
            Ok(Some(api_key)) => {
                info!("   itch API key present, using authenticated search");
                let itch_client = ItchApiClient::new(api_key);
                match itch_client.search_games(client, &query).await {
                    Ok(games) if !games.is_empty() => {
                        info!("   authenticated itch search returned {} games", games.len());
                        results.extend(games.into_iter().map(|g| {
                            let author = g.author();
                            MetadataSearchResult {
                                id: g.id.to_string(),
                                name: g.title,
                                cover_url: g.cover_url,
                                release_date: None,
                                developer: author,
                                publisher: None,
                                description: g.short_text,
                                rating: None,
                                source: "itch".to_string(),
                                url: Some(g.url),
                                tags: None,
                                genres: None,
                            }
                        }));
                    }
                    Ok(_) => {
                        info!("   authenticated itch search returned empty, falling back to aggregator");
                        results.extend(
                            state
                                .metadata_aggregator
                                .search_sources(client, &query, &["itch"])
                                .await,
                        );
                    }
                    Err(e) => {
                        warn!("   authenticated itch search failed ({}), falling back to aggregator", e);
                        results.extend(
                            state
                                .metadata_aggregator
                                .search_sources(client, &query, &["itch"])
                                .await,
                        );
                    }
                }
            }
            _ => {
                info!("   no itch API key, falling back to public aggregator");
                results.extend(
                    state
                        .metadata_aggregator
                        .search_sources(client, &query, &["itch"])
                        .await,
                );
            }
        }
    }

    if sources_refs.contains(&"steam") {
        info!("   searching steam via public aggregator");
        results.extend(
            state
                .metadata_aggregator
                .search_sources(client, &query, &["steam"])
                .await,
        );
    }

    info!("🔍 search_game_metadata: returning {} results", results.len());
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
    info!("🌐 fetch_metadata_by_url_command: source_type={}, url={}", source_type, url);
    let result = fetch_metadata_by_url(&state.http_client, &source_type, &url).await;
    match &result {
        Ok(Some(_)) => info!("   fetch_metadata_by_url_command: returned metadata"),
        Ok(None) => info!("   fetch_metadata_by_url_command: no metadata returned"),
        Err(e) => info!("   fetch_metadata_by_url_command: error: {}", e),
    }
    result
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

/// Scan the install directory of a game and return the discovered metadata
/// without writing anything to the database. Used by the "Local files" tab
/// in the unified metadata update dialog.
#[tauri::command]
pub fn scan_local_metadata(state: State<AppState>, game_id: String) -> Result<Option<ScannedGame>, String> {
    info!("📁 scan_local_metadata: game_id={}", game_id);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let installs = db.get_installs_for_game(&game_id).map_err(|e| e.to_string())?;
    if installs.is_empty() {
        info!("   scan_local_metadata: no installs, returning None");
        return Ok(None);
    }
    let install = &installs[0];
    let game_path = Path::new(&install.install_path);
    if !game_path.exists() {
        info!("   scan_local_metadata: install path does not exist: {}", install.install_path);
        return Ok(None);
    }
    let mut scanned = crate::scanner::scan_single_directory(game_path)
        .map_err(|e| format!("Failed to scan install directory: {}", e))?;
    if let Some(scanned) = scanned.as_mut() {
        scanned.executable = scanned.executable.as_ref().map(|e| game_path.join(e).to_string_lossy().to_string());
        scanned.cover_candidates = scanned.cover_candidates.iter()
            .map(|c| game_path.join(c).to_string_lossy().to_string())
            .collect();
        info!("   scan_local_metadata: title='{}', executable={:?}, covers={}",
            scanned.title, scanned.executable, scanned.cover_candidates.len());
    }
    info!("   scan_local_metadata: returning {}", if scanned.is_some() { "data" } else { "None" });
    Ok(scanned)
}

/// Refresh game data from the local install directory.
/// Only fills missing metadata fields and updates the executable path.
#[tauri::command]
pub fn refresh_game_from_local(state: State<AppState>, game_id: String) -> Result<Game, String> {
    info!("🔄 refresh_game_from_local called for game_id: {}", game_id);

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    let installs = db.get_installs_for_game(&game_id).map_err(|e| e.to_string())?;

    if installs.is_empty() {
        return Err("No install found for this game".to_string());
    }

    let install = &installs[0];
    let game_path = Path::new(&install.install_path);
    if !game_path.exists() {
        return Err(format!("Game directory does not exist: {}", install.install_path));
    }

    debug!("   Scanning directory: {}", game_path.display());
    let scanned = crate::scanner::scan_single_directory(game_path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No game found in install directory".to_string())?;

    // Only overwrite fields that are currently empty/missing so existing
    // metadata (especially from online sources) is preserved.
    let new_title = if game.title.is_empty() && !scanned.title.is_empty() {
        Some(scanned.title.as_str())
    } else {
        None
    };

    let new_description = if game.description.is_none() {
        scanned.exe_metadata.as_ref()
            .and_then(|m| m.file_description.as_deref())
            .filter(|desc| !is_generic_description(desc))
    } else {
        None
    };

    let new_developer = if game.developer.is_none() {
        scanned.exe_metadata.as_ref()
            .and_then(|m| m.company_name.as_deref())
            .filter(|name| !is_generic_company_name(name))
    } else {
        None
    };

    let new_cover = if game.cover_image.is_none() && !scanned.cover_candidates.is_empty() {
        Some(game_path.join(&scanned.cover_candidates[0]).to_string_lossy().to_string())
    } else {
        None
    };

    info!("   refresh_game_from_local: updating title={:?}, desc={:?}, dev={:?}, cover={:?}",
        new_title, new_description, new_developer, new_cover);

    db.update_game(
        &game_id,
        new_title,
        new_description,
        new_developer,
        None, // publisher — keep existing
        new_cover.as_deref(),
        None, // is_favorite — keep existing
        None, // completion_status — keep existing
        None, // user_rating — keep existing
    ).map_err(|e| e.to_string())?;

    if let Some(exe_path) = &scanned.executable {
        info!("   refresh_game_from_local: updating install executable to {}", exe_path);
        db.update_install_executable(&install.id, exe_path).map_err(|e| e.to_string())?;
    }

    info!("   ✅ Game refreshed successfully");
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

