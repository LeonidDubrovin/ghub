use crate::download_service::{self, ItchDownloadResolution};
use crate::models::Game;
use crate::AppState;
use log::{error, info};
use std::path::Path;
use tauri::State;

#[tauri::command]
pub fn get_download_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_download_games().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_game_from_link(
    state: State<'_, AppState>,
    url: String,
) -> Result<Game, String> {
    let (source_type, query) = download_service::parse_link_url(&url);

    // Search metadata while not holding the DB lock.
    let best_match = state
        .metadata_aggregator
        .search_best(&state.http_client, &query)
        .await;

    let title = best_match
        .as_ref()
        .map(|m| m.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| query.clone());

    let description = best_match.as_ref().and_then(|m| m.description.clone());
    let developer = best_match.as_ref().and_then(|m| m.developer.clone());
    let cover_image = best_match.as_ref().and_then(|m| m.cover_url.clone());

    let download_status = match source_type {
        Some("itch") => "pending",
        _ => "external",
    };

    let game_id = uuid::Uuid::new_v4().to_string();
    let link_id = uuid::Uuid::new_v4().to_string();

    let game = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_game(
            &game_id,
            &title,
            description.as_deref(),
            developer.as_deref(),
            cover_image.as_deref(),
            Some(&url),
        )
        .map_err(|e| e.to_string())?;

        db.create_game_link(
            &link_id,
            &game_id,
            &url,
            Some(&title),
            source_type,
            Some(download_status),
        )
        .map_err(|e| e.to_string())?;

        db.get_game_by_id(&game_id).map_err(|e| e.to_string())?
    };

    Ok(game)
}

#[tauri::command]
pub async fn download_game_link(
    state: State<'_, AppState>,
    game_id: String,
    link_id: String,
    space_id: String,
    source_path: String,
) -> Result<Game, String> {
    // Load the link and game without holding the lock across awaits.
    let (link, game) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let link = db.get_game_link_by_id(&link_id).map_err(|e| e.to_string())?;
        let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
        (link, game)
    };

    if link.source_type.as_deref() != Some("itch") {
        return Err("Only itch.io links can be downloaded".to_string());
    }

    info!("Downloading game {} from link {}", game_id, link_id);

    let resolution = match download_service::resolve_itch_download_url(&state.http_client, &link.url).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to resolve itch download: {}", e);
            let _ = state
                .db
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|db| db.update_game_link_status(&link.id, "error").map_err(|e| e.to_string()));
            return Err(e);
        }
    };

    match resolution {
        ItchDownloadResolution::Browser => {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db.update_game_link_status(&link.id, "browser")
                .map_err(|e| e.to_string())?;
            db.get_game_by_id(&game_id)
                .map_err(|e| e.to_string())
        }
        ItchDownloadResolution::Direct(direct_url) => {
            let target_path = Path::new(&source_path);
            std::fs::create_dir_all(target_path)
                .map_err(|e| format!("Failed to create source directory: {}", e))?;

            let downloaded = match download_service::download_itch_game(
                &state.http_client,
                &direct_url,
                target_path,
                &game.title,
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    error!("Failed to download itch game: {}", e);
                    let _ = state
                        .db
                        .lock()
                        .map_err(|e| e.to_string())
                        .and_then(|db| db.update_game_link_status(&link.id, "error").map_err(|e| e.to_string()));
                    return Err(e);
                }
            };

            let db = state.db.lock().map_err(|e| e.to_string())?;
            let install_id = uuid::Uuid::new_v4().to_string();
            db.create_install(
                &install_id,
                &game_id,
                &space_id,
                &downloaded.install_path,
                downloaded.executable_path.as_deref(),
            )
            .map_err(|e| e.to_string())?;

            db.update_game_link_status(&link.id, "downloaded")
                .map_err(|e| e.to_string())?;

            db.get_game_by_id(&game_id)
                .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn open_game_link(url: String, source_type: Option<String>) -> Result<(), String> {
    let open_url = download_service::build_open_url(&url, source_type.as_deref());
    download_service::open_url(&open_url)
}

#[tauri::command]
pub fn remove_game_link(state: State<AppState>, link_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_game_link(&link_id).map_err(|e| e.to_string())
}

