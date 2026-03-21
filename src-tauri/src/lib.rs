mod database;
mod commands;
mod models;
mod playtime;
mod scanning_service;
mod scanner;
mod scanner_constants;
mod title_extraction;
pub mod meta_service;
pub mod metadata;

use tauri::Manager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use database::Database;
pub use playtime::PlaytimeTracker;
pub use metadata::MetadataAggregator;
pub use scanning_service::ScanningService;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub db_path: PathBuf,
    pub playtime: Mutex<PlaytimeTracker>,
    pub http_client: reqwest::Client,
    pub metadata_aggregator: MetadataAggregator,
    pub scanning_service: Mutex<ScanningService>,
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
            
            // Initialize scanning service
            let scanning_service = ScanningService::new();
            
            // Manage app state
            app.manage(AppState {
                db,
                db_path: db_path.clone(),
                playtime: Mutex::new(playtime),
                http_client,
                metadata_aggregator: MetadataAggregator::new(),
                scanning_service: Mutex::new(scanning_service),
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
            commands::start_source_scan,
            commands::get_source_scan_status,
            commands::cancel_source_scan,
            commands::get_all_games,
            commands::get_games_by_space,
            commands::get_games_by_source,
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
            commands::refresh_game_from_local,
            commands::fetch_and_update_game_metadata,
            commands::backup_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}