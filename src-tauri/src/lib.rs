mod database;
mod commands;
mod models;
mod playtime;
pub mod meta_service;

use tauri::Manager;
use std::sync::{Arc, Mutex};

pub use database::Database;
pub use playtime::PlaytimeTracker;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub playtime: Mutex<PlaytimeTracker>,
    pub http_client: reqwest::Client,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Get app data directory
            let app_data_dir = app.path().app_data_dir()?;
            
            // Create directory if it doesn't exist
            std::fs::create_dir_all(&app_data_dir)?;
            
            // Initialize database
            let db_path = app_data_dir.join("ghub.db");
            let db = Database::new(&db_path)?;
            let db = Arc::new(Mutex::new(db));
            
            // Initialize playtime tracker
            let playtime = PlaytimeTracker::new(Arc::clone(&db));
            let http_client = reqwest::Client::new();
            
            // Manage app state
            app.manage(AppState { 
                db,
                playtime: Mutex::new(playtime),
                http_client,
            });
            
            println!("✅ GHub initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_all_spaces,
            commands::create_space,
            commands::delete_space,
            commands::get_space_sources,
            commands::add_space_source,
            commands::remove_space_source,
            commands::update_space_source,
            commands::scan_space_sources,
            commands::get_all_games,
            commands::get_games_by_space,
            commands::create_game,
            commands::create_game_link,
            commands::create_download_link,
            commands::get_download_links,
            commands::delete_download_link,
            commands::update_game,
            commands::delete_game,
            commands::scan_directory,
            commands::launch_game,
            commands::get_active_sessions,
            commands::get_settings,
            commands::update_setting,
            commands::search_game_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}