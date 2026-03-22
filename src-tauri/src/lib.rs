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
    // Initialize logger early to capture all logs
    init_logger();
    
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
            
            log::info!("GHub initialized successfully");
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
            commands::log_frontend,
            commands::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Initialize logging based on build configuration
fn init_logger() {
    #[cfg(debug_assertions)]
    {
        // Development: logs in project directory ./logs/ with timestamped filenames
        use std::env::current_dir;
        use std::fs;
        
        // Get the project directory (current working directory)
        let project_dir = match current_dir() {
            Ok(path) => path,
            Err(_) => {
                // Fallback to current directory if we can't get it
                std::path::PathBuf::from(".")
            }
        };
        
        let log_dir = project_dir.join("logs");
        
        // Create logs directory
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory: {}", e);
            // Fall back to console only if we can't create directory
            fern::Dispatch::new()
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "{}[{}][{}] {}",
                        chrono::Local::now().format("[%H:%M:%S]"),
                        record.target(),
                        record.level(),
                        message
                    ))
                })
                .level(log::LevelFilter::Debug)
                .chain(std::io::stdout())
                .apply()
                .unwrap();
            log::info!("Logger initialized in DEBUG mode (console only, directory creation failed)");
            return;
        }
        
        // Generate timestamp for log filenames
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let log_file = log_dir.join(format!("ghub_{}.log", timestamp));
        let error_file = log_dir.join(format!("error_{}.log", timestamp));
        
        // Configure fern for development
        let mut dispatch = fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    chrono::Local::now().format("[%H:%M:%S]"),
                    record.target(),
                    record.level(),
                    message
                ))
            })
            .level(log::LevelFilter::Debug);
        
        // Chain to log file (all debug+)
        if let Ok(file) = fern::log_file(&log_file) {
            dispatch = dispatch.chain(file);
        } else {
            eprintln!("Failed to open log file: {}", log_file.display());
        }
        
        // Chain to error file (errors only) - create separate dispatch with filter
        if let Ok(file) = fern::log_file(&error_file) {
            let error_dispatch = fern::Dispatch::new().chain(file).filter(|metadata| metadata.level() == log::Level::Error);
            dispatch = dispatch.chain(error_dispatch);
        } else {
            eprintln!("Failed to open error log file: {}", error_file.display());
        }
        
        // Also output to stdout
        dispatch = dispatch.chain(std::io::stdout());
        
        if let Err(e) = dispatch.apply() {
            eprintln!("Failed to initialize logger: {}", e);
        } else {
            log::info!("Logger initialized in DEBUG mode, logs: {}", log_dir.display());
        }
    }
    
    #[cfg(not(debug_assertions))]
    {
        // Release: file-based logging beside the executable with timestamped filenames
        use std::env::current_exe;
        use std::fs;
        
        // Get the directory of the executable
        let exe_path = match current_exe() {
            Ok(path) => path,
            Err(_) => {
                // Fallback to current directory if we can't get exe path
                std::path::PathBuf::from(".")
            }
        };
        
        let exe_dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let log_dir = exe_dir.join("logs");
        
        // Create logs directory
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory: {}", e);
            // Fall back to console if we can't create directory
            fern::Dispatch::new()
                .level(log::LevelFilter::Info)
                .chain(std::io::stdout())
                .apply()
                .unwrap();
            return;
        }
        
        // Generate timestamp for log filenames
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let log_file = log_dir.join(format!("ghub_{}.log", timestamp));
        let error_file = log_dir.join(format!("error_{}.log", timestamp));
        
        // Configure fern for release
        let mut dispatch = fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    record.level(),
                    message
                ))
            })
            .level(log::LevelFilter::Info);
        
        // Chain to log file (all info+)
        if let Ok(file) = fern::log_file(&log_file) {
            dispatch = dispatch.chain(file);
        } else {
            eprintln!("Failed to open log file: {}", log_file.display());
        }
        
        // Chain to error file (errors only) - create separate dispatch with filter
        if let Ok(file) = fern::log_file(&error_file) {
            let error_dispatch = fern::Dispatch::new().chain(file).filter(|metadata| metadata.level() == log::Level::Error);
            dispatch = dispatch.chain(error_dispatch);
        } else {
            eprintln!("Failed to open error log file: {}", error_file.display());
        }
        
        // Also output to stdout for console visibility in release
        dispatch = dispatch.chain(std::io::stdout());
        
        if let Err(e) = dispatch.apply() {
            eprintln!("Failed to initialize logger: {}", e);
        } else {
            log::info!("Logger initialized in RELEASE mode, logs: {}", log_dir.display());
        }
    }
}
