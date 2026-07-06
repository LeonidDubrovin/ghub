use crate::AppState;
use chrono::Local;
use rusqlite::backup::Backup;
use rusqlite::Connection;
use std::fs;
use tauri::State;

#[tauri::command]
pub fn backup_database(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let db_path = &state.db_path;

    // Create backups directory
    let backup_dir = db_path.parent().ok_or("Invalid db path")?.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    // Verify backup directory is writable
    let test_file = backup_dir.join(".write_test");
    fs::write(&test_file, "test").map_err(|e| format!("Backup directory is not writable: {}", e))?;
    fs::remove_file(&test_file).map_err(|e| format!("Failed to clean up test file: {}", e))?;

    // Generate timestamped backup filename
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let backup_filename = format!("ghub_{}.db", timestamp);
    let backup_path = backup_dir.join(backup_filename);

    // Use SQLite online backup API to create a consistent backup
    let mut backup_conn = Connection::open(&backup_path)
        .map_err(|e| format!("Failed to open backup connection: {}", e))?;
    let backup = Backup::new(&db.conn, &mut backup_conn)
        .map_err(|e| format!("Failed to initialize backup: {}", e))?;
    backup
        .step(-1)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    Ok(backup_path.to_string_lossy().to_string())
}
