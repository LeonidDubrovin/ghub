use crate::AppState;
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn backup_database(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let db_path = &state.db_path;

    // Create backups directory
    let backup_dir = db_path.parent().ok_or("Invalid db path")?.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    // Generate timestamped backup filename
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let backup_filename = format!("ghub_{}.db", timestamp);
    let backup_path = backup_dir.join(backup_filename);

    // Use VACUUM INTO to create a consistent backup (SQLite 3.27+)
    let sql = format!("VACUUM INTO '{}'", backup_path.to_string_lossy());
    db.conn
        .execute_batch(&sql)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    Ok(backup_path.to_string_lossy().to_string())
}
