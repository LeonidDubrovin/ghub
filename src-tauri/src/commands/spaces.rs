use crate::models::{Space, SpaceSource, CreateSpaceRequest};
use crate::AppState;
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
    ).map_err(|e| e.to_string())?;

    // If initial_sources provided, add them
    if let Some(sources) = request.initial_sources {
        for source_path in sources {
            println!("➕ Adding source to space {}: {}", id, source_path);
            let _ = db.add_space_source(&id, &source_path, true);
        }
    }

    db.get_space_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_space(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_space(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_space_sources(state: State<AppState>, space_id: String) -> Result<Vec<SpaceSource>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_space_sources(&space_id).map_err(|e| e.to_string())?;
    println!("📚 get_space_sources for {}: {} sources", space_id, sources.len());
    Ok(sources)
}

#[tauri::command]
pub fn add_space_source(state: State<AppState>, space_id: String, source_path: String, scan_recursively: Option<bool>) -> Result<(), String> {
    println!("➕ add_space_source: space={}, path={}, recursive={:?}", space_id, source_path, scan_recursively);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_space_source(&space_id, &source_path, scan_recursively.unwrap_or(true)).map_err(|e| e.to_string())?;
    println!("   ✅ Source added successfully");
    Ok(())
}

#[tauri::command]
pub fn remove_space_source(state: State<AppState>, space_id: String, source_path: String) -> Result<(), String> {
    println!("➖ remove_space_source: space={}, path={}", space_id, source_path);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_space_source(&space_id, &source_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_space_source(state: State<AppState>, space_id: String, source_path: String, is_active: Option<bool>, scan_recursively: Option<bool>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_space_source(&space_id, &source_path, is_active.unwrap_or(true), scan_recursively).map_err(|e| e.to_string())?;
    Ok(())
}