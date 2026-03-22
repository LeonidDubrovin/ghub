// Filesystem-related commands
use std::path::Path;

/// Open a folder in the system file explorer (reveal in Finder/Explorer)
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    if path.is_empty() {
        return Err("Path is empty".to_string());
    }

    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, use explorer.exe to select the folder/file
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg("/select,");
        cmd.arg(path_obj.canonicalize().map_err(|e| e.to_string())?);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, use 'open -R' to reveal in Finder
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-R");
        cmd.arg(path_obj.canonicalize().map_err(|e| e.to_string())?);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, try to reveal in default file manager
        // Try common file managers
        let canonical_path = path_obj.canonicalize().map_err(|e| e.to_string())?;
        let path_str = canonical_path.to_string_lossy().to_string();

        // Try xdg-open with parent directory (most Linux file managers will select the file if passed with filename)
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(path_str);
        cmd.spawn().map_err(|e| e.to_string())?;
    }

    Ok(())
}
