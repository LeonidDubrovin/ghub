use crate::crypto;
use crate::AppState;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn master_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("master.key"))
}

fn api_key_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("itch_api_key.enc"))
}

#[tauri::command]
pub fn get_itch_api_key(app: AppHandle) -> Result<Option<String>, String> {
    let key_path = master_key_path(&app)?;
    let cipher_path = api_key_file_path(&app)?;
    crypto::decrypt_from_file(&key_path, &cipher_path)
}

#[tauri::command]
pub fn set_itch_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    let key_path = master_key_path(&app)?;
    let cipher_path = api_key_file_path(&app)?;
    crypto::encrypt_to_file(&key_path, &cipher_path, &api_key)
}

#[tauri::command]
pub fn delete_itch_api_key(app: AppHandle) -> Result<(), String> {
    let cipher_path = api_key_file_path(&app)?;
    if cipher_path.exists() {
        std::fs::remove_file(&cipher_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Fetch available itch.io uploads for a game URL.
#[tauri::command]
pub async fn get_itch_game_uploads(
    app: AppHandle,
    state: State<'_, AppState>,
    game_url: String,
    game_title: Option<String>,
) -> Result<Vec<crate::itch_api::ItchApiUpload>, String> {
    let api_key = get_itch_api_key(app)?.ok_or("itch.io API key is not set")?;
    let client = crate::itch_api::ItchApiClient::new(api_key);

    // First try the documented itch.io search API. This avoids scraping and is the most reliable
    // way to resolve a URL to a game ID when the API key is valid.
    let game_id = match client
        .resolve_game_id_by_url(&state.http_client, &game_url, game_title.as_deref())
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            log::info!("Search API did not resolve game id, falling back to page parsing");
            crate::itch_api::fetch_game_id_from_url(&state.http_client, &game_url).await?
        }
        Err(e) => {
            log::info!("Search API failed ({}), falling back to page parsing", e);
            crate::itch_api::fetch_game_id_from_url(&state.http_client, &game_url).await?
        }
    };

    client.get_game_uploads(&state.http_client, game_id).await
}
