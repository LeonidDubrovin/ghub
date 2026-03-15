use crate::AppState;
use tauri::State;
use std::path::Path;

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