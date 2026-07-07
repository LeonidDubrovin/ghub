use crate::models::{Game, Install, CreateGameRequest, UpdateGameRequest, GameLink};
use crate::AppState;
use tauri::State;
use std::path::Path;

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
            None,
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

// ============ INSTALLS ============

#[tauri::command]
pub fn get_game_installs(state: State<AppState>, game_id: String) -> Result<Vec<Install>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_installs_for_game(&game_id).map_err(|e| e.to_string())
}

fn is_safe_to_delete(install_path: &Path, source_path: &Path) -> Result<bool, String> {
    let source_canon = std::fs::canonicalize(source_path)
        .map_err(|e| format!("Failed to canonicalize source path: {}", e))?;
    let install_canon = std::fs::canonicalize(install_path)
        .map_err(|e| format!("Failed to canonicalize install path: {}", e))?;
    Ok(install_canon.starts_with(&source_canon))
}

fn delete_install_files(db: &crate::database::Database, install: &Install) -> Result<(), String> {
    let install_path = Path::new(&install.install_path);
    if !install_path.exists() {
        return Ok(());
    }

    let space = db.get_space_by_id(&install.space_id).map_err(|e| e.to_string())?;
    let source_path_str = space.path.as_deref().ok_or("Space has no source path")?;
    let source_path = Path::new(source_path_str);

    if !is_safe_to_delete(install_path, source_path)? {
        return Err(format!(
            "Install path '{}' is not inside the configured source path '{}'. Refusing to delete files.",
            install.install_path, source_path_str
        ));
    }

    if install_path.is_dir() {
        std::fs::remove_dir_all(install_path)
            .map_err(|e| format!("Failed to delete install folder: {}", e))?;
    } else if install_path.is_file() {
        std::fs::remove_file(install_path)
            .map_err(|e| format!("Failed to delete install file: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_game_install(state: State<AppState>, install_id: String, delete_files: bool) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if delete_files {
        let install = db.get_install_by_id(&install_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Install not found".to_string())?;
        delete_install_files(&db, &install)?;
    }

    db.delete_install(&install_id).map_err(|e| e.to_string())
}
