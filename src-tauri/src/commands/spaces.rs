use crate::models::{CreateSpaceRequest, Space, SpaceSource};
use crate::AppState;
use log::{debug, error};
use tauri::State;

#[tauri::command]
pub fn get_all_spaces(state: State<AppState>) -> Result<Vec<Space>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_spaces().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_space(state: State<AppState>, request: CreateSpaceRequest) -> Result<Space, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    db.create_space(
        &id,
        &request.name,
        None, // Space path is deprecated, use space_sources instead
        &request.space_type,
        request.icon.as_deref(),
        request.color.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // If initial_sources provided, add them
    if let Some(sources) = request.initial_sources {
        for source_path in sources {
            debug!("Adding source to space {}: {}", id, source_path);
            db.add_space_source(&id, &source_path, true)
                .map_err(|e| format!("Failed to add source '{}': {}", source_path, e))?;
        }
    }

    db.get_space_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_space_sources(
    state: State<AppState>,
    space_id: String,
) -> Result<Vec<SpaceSource>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_space_sources(&space_id).map_err(|e| e.to_string())?;
    debug!("📚 get_space_sources for {}: {} sources", space_id, sources.len());
    Ok(sources)
}

#[tauri::command]
pub fn add_space_source(
    state: State<AppState>,
    space_id: String,
    source_path: String,
    scan_recursively: Option<bool>,
) -> Result<(), String> {
    debug!("add_space_source: space={}, path={}, recursive={:?}", space_id, source_path, scan_recursively);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_space_source(&space_id, &source_path, scan_recursively.unwrap_or(true))
        .map_err(|e| e.to_string())?;
    debug!("Source added successfully");
    Ok(())
}

#[tauri::command]
pub fn remove_space_source(
    state: State<AppState>,
    space_id: String,
    source_path: String,
    delete_games: Option<bool>,
) -> Result<(), String> {
    debug!("remove_space_source: space={}, path={}, delete_games={:?}", space_id, source_path, delete_games);
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    match db.remove_space_source(&space_id, &source_path, delete_games.unwrap_or(false)) {
        Ok(_) => {
            debug!("Successfully removed source {} from space {}", source_path, space_id);
            Ok(())
        }
        Err(e) => {
            error!("Failed to remove source {} from space {}: {}", source_path, space_id, e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn delete_space(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_space(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_space_source(
    state: State<AppState>,
    space_id: String,
    source_path: String,
    is_active: Option<bool>,
    scan_recursively: Option<bool>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_space_source(
        &space_id,
        &source_path,
        is_active.unwrap_or(true),
        scan_recursively,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============ ASYNC SCANNING COMMANDS ============

#[tauri::command]
pub fn start_source_scan(
    state: State<AppState>,
    space_id: String,
    source_path: String,
) -> Result<(), String> {
    debug!(
        "start_source_scan: space={}, source={}",
        space_id, source_path
    );
    let scanning_service = match state.scanning_service.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(e.to_string()),
    };
    let db = state.db.clone();
    scanning_service.start_scan(db, space_id, source_path)?;
    Ok(())
}

#[tauri::command]
pub fn get_source_scan_status(
    state: State<AppState>,
    space_id: String,
    source_path: String,
) -> Result<SpaceSource, String> {
    debug!(
        "get_source_scan_status: space={}, source={}",
        space_id, source_path
    );
    let scanning_service = match state.scanning_service.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(e.to_string()),
    };

    // Check if there's an actual active scan for this source using the public method
    let has_active_scan = scanning_service.is_scan_active(&space_id, &source_path);

    // If DB says scanning but no active scan, the scan is stuck - reset it
    if !has_active_scan {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        if let Ok(Some(source)) = db.get_source_scan_status(&space_id, &source_path) {
            if source.scan_status == Some("scanning".to_string()) {
                debug!("Stuck scan detected (DB says scanning but no active scan) - resetting status for {}:{}", space_id, source_path);
                db.set_source_scan_status(&space_id, &source_path, None, None, None, None)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    match scanning_service.get_source_scan_status(&state.db, &space_id, &source_path) {
        Ok(Some(status)) => Ok(status),
        Ok(None) => {
            // Return source without scan status
            let db = state.db.lock().map_err(|e| e.to_string())?;
            let source = db
                .get_space_sources(&space_id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|s| s.source_path == source_path)
                .ok_or_else(|| "Source not found".to_string())?;
            Ok(source)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub fn cancel_source_scan(
    state: State<AppState>,
    space_id: String,
    source_path: String,
) -> Result<(), String> {
    debug!(
        "cancel_source_scan: space={}, source={}",
        space_id, source_path
    );
    let scanning_service = match state.scanning_service.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(e.to_string()),
    };
    scanning_service.cancel_scan(&state.db, &space_id, &source_path)?;
    Ok(())
}
