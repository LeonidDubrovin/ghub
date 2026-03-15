use crate::models::{DownloadLink, MetadataSearchResult};
use crate::AppState;
use crate::meta_service;
use tauri::State;

#[tauri::command]
pub fn get_download_links(state: State<AppState>) -> Result<Vec<DownloadLink>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_download_links().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_download_link(state: State<'_, AppState>, url: String) -> Result<DownloadLink, String> {
    let title = url.split('/').last().unwrap_or("Unknown Link").replace('-', " ").replace('_', " ");

    let link = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_download_link(&url, &title, None, None).map_err(|e| e.to_string())?
    };

    // Attempt to fetch metadata if it's a known store
    let client = &state.http_client;
    let _meta: Option<MetadataSearchResult> = None;

    if url.contains("store.steampowered.com") {
        if let Ok(results) = meta_service::search_steam(client, &title).await {
            let _first = results.into_iter().next();
        }
    } else if url.contains("itch.io") {
        if let Ok(results) = meta_service::search_itch(client, &title).await {
            let _first = results.into_iter().next();
        }
    }

    Ok(link)
}

#[tauri::command]
pub fn delete_download_link(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_download_link(&id).map_err(|e| e.to_string())
}