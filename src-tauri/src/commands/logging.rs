use log::{debug, error, info, warn};

/// Command to receive logs from the frontend and log them in the backend
/// This allows unified logging with backend logs in a single file
#[tauri::command]
pub fn log_frontend(level: String, message: String, context: String) {
    let full_message = format!("[Frontend:{}] {}", context, message);
    
    match level.to_lowercase().as_str() {
        "error" => error!("{}", full_message),
        "warn" => warn!("{}", full_message),
        "info" => info!("{}", full_message),
        "debug" => debug!("{}", full_message),
        _ => info!("{}", full_message),
    }
}
