use crate::models::{Game, CreateGameRequest, CreateGameLinkRequest, UpdateGameRequest, MetadataSearchResult};
use crate::AppState;
use crate::meta_service;
use tauri::State;

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
pub fn get_games_by_source(
    state: State<AppState>,
    space_id: String,
    source_path: String,
) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_games_for_source(&space_id, &source_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_game(state: State<'_, AppState>, request: CreateGameRequest) -> Result<Game, String> {
    let game_id = uuid::Uuid::new_v4().to_string();
    let install_id = uuid::Uuid::new_v4().to_string();

    // Scope for DB lock
    let _game = {
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