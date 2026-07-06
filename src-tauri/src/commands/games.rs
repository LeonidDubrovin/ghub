use crate::models::{Game, CreateGameRequest, UpdateGameRequest, GameLink};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_all_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_games().map_err(|e| e.to_string())
}

const SYSTEM_QUEUE_SPACES: &[&str] = &["incoming", "online"];

#[tauri::command]
pub fn get_games_by_space(state: State<AppState>, space_id: String) -> Result<Vec<Game>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if SYSTEM_QUEUE_SPACES.contains(&space_id.as_str()) {
        db.get_games_by_queue_space(&space_id).map_err(|e| e.to_string())
    } else {
        db.get_games_by_space(&space_id).map_err(|e| e.to_string())
    }
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

    // Return the created game
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_game_by_id(&game_id).map_err(|e| e.to_string())
    }
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

// ============ GAME LINKS ============

#[tauri::command]
pub fn get_game_links(state: State<AppState>, game_id: String) -> Result<Vec<GameLink>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_game_links(&game_id).map_err(|e: rusqlite::Error| e.to_string())
}

#[tauri::command]
pub fn add_game_link(
    state: State<AppState>,
    game_id: String,
    url: String,
    title: Option<String>,
    source_type: Option<String>,
    download_status: Option<String>,
    queue_space: Option<String>,
) -> Result<GameLink, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let link_id = uuid::Uuid::new_v4().to_string();
    db.create_game_link(
        &link_id,
        &game_id,
        &url,
        title.as_deref(),
        source_type.as_deref(),
        download_status.as_deref(),
        queue_space.as_deref(),
    )
    .map_err(|e: rusqlite::Error| e.to_string())
}
