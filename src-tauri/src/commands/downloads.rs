use crate::download_service;
use crate::models::{Game, GameLink};
use crate::AppState;
use log::{error, info};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

#[derive(Serialize)]
pub struct DownloadGameLinkResponse {
    pub game: Game,
    pub status: String,
}

#[derive(Serialize)]
pub struct CreateGameFromLinkResponse {
    pub game: Game,
    pub is_duplicate: bool,
    pub existing_link: Option<GameLink>,
}

#[tauri::command]
pub fn get_download_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_download_games().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_game_from_link(
    state: State<'_, AppState>,
    url: String,
) -> Result<CreateGameFromLinkResponse, String> {
    let (source_type, query) = download_service::parse_link_url(&url);
    let canonical_url = crate::url_utils::canonical_url(&url, source_type);

    // Check whether the same link already points to an existing game card.
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        if let Some((existing_game, existing_link)) = db
            .find_game_by_canonical_url(&canonical_url)
            .map_err(|e| e.to_string())?
        {
            return Ok(CreateGameFromLinkResponse {
                game: existing_game,
                is_duplicate: true,
                existing_link: Some(existing_link),
            });
        }
    }

    // Fetch exact metadata from the source page when the source type is known.
    let best_match = if let Some(st) = source_type {
        crate::commands::metadata::fetch_metadata_by_url(&state.http_client, st, &url)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };

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
            Some(&canonical_url),
            Some(&title),
            source_type,
            Some(download_status),
            Some("incoming"),
        )
        .map_err(|e| e.to_string())?;

        db.get_game_by_id(&game_id).map_err(|e| e.to_string())?
    };

    Ok(CreateGameFromLinkResponse {
        game,
        is_duplicate: false,
        existing_link: None,
    })
}

#[tauri::command]
pub async fn download_game_link(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
    link_id: String,
    upload_id: i64,
    upload_name: String,
    upload_filename: Option<String>,
    upload_platforms: Option<Vec<String>>,
    space_id: String,
    source_path: String,
) -> Result<DownloadGameLinkResponse, String> {
    // Require an itch.io API key to download.
    let api_key = crate::commands::get_itch_api_key(app.clone())?
        .ok_or("itch.io API key is not set. Please add it in Settings.")?;

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

    info!(
        "Downloading itch upload {} ({}) for game {} into {}",
        upload_id, upload_name, game_id, source_path
    );

    let api_client = crate::itch_api::ItchApiClient::new(api_key.clone());
    let direct_url = api_client
        .get_download_url(&state.http_client, upload_id)
        .await
        .map_err(|e| {
            error!("Failed to resolve itch download URL: {}", e);
            let _ = state
                .db
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|db| db.update_game_link_status(&link.id, "error").map_err(|e| e.to_string()));
            e
        })?;

    let target_path = Path::new(&source_path);
    std::fs::create_dir_all(target_path)
        .map_err(|e| format!("Failed to create source directory: {}", e))?;

    let downloaded = match download_service::download_itch_game(
        &app,
        &state.http_client,
        &link_id,
        &direct_url,
        Some(&api_key),
        target_path,
        &game.title,
        Some(&upload_name),
        upload_filename.as_deref(),
        upload_platforms,
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
        Some(&upload_name),
        Some(upload_id),
    )
    .map_err(|e| e.to_string())?;

    db.update_game_link_status(&link.id, "downloaded")
        .map_err(|e| e.to_string())?;
    db.update_game_link_queue_space(&link.id, None)
        .map_err(|e| e.to_string())?;

    let game = db.get_game_by_id(&game_id).map_err(|e| e.to_string())?;
    Ok(DownloadGameLinkResponse { game, status: "downloaded".to_string() })
}

#[tauri::command]
pub fn move_game_link(
    state: State<AppState>,
    link_id: String,
    queue_space: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_game_link_queue_space(&link_id, queue_space.as_deref())
        .map_err(|e| e.to_string())
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