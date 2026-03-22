/// Filesystem-related commands

use std::path::PathBuf;

/// Open a folder in the system file explorer
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    if path.is_empty() {
        return Err("Path is empty".to_string());
    }

    let path_obj = PathBuf::from(&path);
    if !path_obj.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, use explorer to open the folder directly
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(&path);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, use 'open' to reveal the folder in Finder
        let mut cmd = std::process::Command::new("open");
        cmd.arg(&path);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, use xdg-open to open the folder
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(&path);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    Ok(())
}
