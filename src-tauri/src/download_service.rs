use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use regex::Regex;
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use log::info;

/// Progress payload emitted while a file is being downloaded.
#[derive(Serialize, Clone)]
pub struct DownloadProgress {
    pub link_id: String,
    pub downloaded: u64,
    pub total: u64,
}

/// Parse a store URL into a source type and a search-friendly title.
pub fn parse_link_url(url: &str) -> (Option<&'static str>, String) {
    if url.contains("store.steampowered.com") || url.contains("steamcommunity.com/app") {
        let query = url.split('/')
            .last()
            .unwrap_or("")
            .replace('-', " ")
            .replace('_', " ");
        (Some("steam"), query)
    } else if url.contains("itch.io") {
        // itch.io URLs are usually https://developer.itch.io/game-name
        let mut query = url.trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or("")
            .replace('-', " ")
            .replace('_', " ");
        if query.is_empty() {
            query = url.to_string();
        }
        (Some("itch"), query)
    } else if url.contains("gog.com") {
        let query = url.split('/')
            .last()
            .unwrap_or("")
            .replace('-', " ")
            .replace('_', " ");
        (Some("gog"), query)
    } else if url.contains("epicgames.com") {
        let query = url.split('/')
            .last()
            .unwrap_or("")
            .replace('-', " ")
            .replace('_', " ");
        (Some("epic"), query)
    } else {
        (None, url.to_string())
    }
}

/// Extract the Steam app ID from a Steam store/community URL.
pub fn extract_steam_app_id(url: &str) -> Option<String> {
    let re = Regex::new(r"/app/(\d+)").ok()?;
    re.captures(url).and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Build a launch URL for a store link.
pub fn build_open_url(url: &str, source_type: Option<&str>) -> String {
    if source_type == Some("steam") {
        if let Some(app_id) = extract_steam_app_id(url) {
            return format!("steam://store/{}", app_id);
        }
    }
    url.to_string()
}

/// Open a URL in the default application (browser / Steam client).
/// On Windows this uses `start`; on Linux `xdg-open`; on macOS `open`.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    Ok(())
}

/// Result of a successful download + extraction.
pub struct DownloadedInstall {
    pub install_path: String,
    pub executable_path: Option<String>,
}

/// Download an itch game and extract it into a subfolder of the given source.
/// The returned install path is the folder that contains the game executable.
pub async fn download_itch_game(
    app: &AppHandle,
    client: &Client,
    link_id: &str,
    direct_url: &str,
    api_key: Option<&str>,
    source_path: &Path,
    title: &str,
    variant_name: Option<&str>,
    archive_filename: Option<&str>,
    upload_platforms: Option<Vec<String>>,
) -> Result<DownloadedInstall, String> {
    let base_name = sanitize_folder_name(title);
    let clean_variant = clean_variant_name(title, variant_name.unwrap_or(""), upload_platforms.as_deref());
    let base_dir = source_path.join(&base_name);
    let target_dir = match clean_variant {
        Some(ref v) if !v.trim().is_empty() => {
            let variant = sanitize_folder_name(v);
            base_dir.join(variant)
        }
        _ => base_dir.join("default"),
    };
    let mut target_dir = target_dir;
    ensure_unique_dir(&mut target_dir);
    info!(
        "Download folder: title='{}' raw_variant='{:?}' clean_variant='{:?}' platforms='{:?}' base_dir='{}' final_folder='{}'",
        title, variant_name, clean_variant, upload_platforms, base_dir.display(), target_dir.display()
    );

    // Download to a temporary archive file next to the target folder.
    let archive_path = match archive_filename {
        Some(name) if !name.trim().is_empty() => {
            let name = sanitize_file_name(name);
            source_path.join(name)
        }
        _ => source_path.join(format!("{}.download", base_name)),
    };
    let mut unique_archive = archive_path.clone();
    ensure_unique_path(&mut unique_archive);

    info!("Downloading itch game to {:?}", unique_archive);
    download_file(app, link_id, client, direct_url, api_key, &unique_archive).await?;

    // The heavy extraction/filesystem work runs in a blocking thread pool
    // so the async runtime stays responsive.
    let source_path_owned = source_path.to_path_buf();
    let unique_archive_owned = unique_archive.clone();
    let base_name_owned = base_name.clone();
    let target_dir_owned = target_dir.clone();

    let result = tokio::task::spawn_blocking(move || {
        process_downloaded_archive(
            &unique_archive_owned,
            &source_path_owned,
            &base_name_owned,
            &target_dir_owned,
        )
    })
    .await
    .map_err(|e| format!("Failed to run archive processing: {}", e))?;

    result
}

/// Synchronous extraction / scanning step for a downloaded itch archive.
fn process_downloaded_archive(
    archive_path: &Path,
    source_path: &Path,
    base_name: &str,
    target_dir: &Path,
) -> Result<DownloadedInstall, String> {
    // Extract into a temporary directory, then move the contents to the target folder.
    let extract_temp = source_path.join(format!("{}-extract", base_name));
    let mut unique_extract = extract_temp.clone();
    ensure_unique_dir(&mut unique_extract);

    std::fs::create_dir_all(&unique_extract)
        .map_err(|e| format!("Failed to create extract temp directory: {}", e))?;

    let extraction_result = extract_archive(archive_path, &unique_extract);

    // Remove the archive now that it is extracted (or if extraction failed).
    let _ = std::fs::remove_file(archive_path);

    extraction_result?;

    // Determine the actual game directory within the extracted contents.
    let game_dir = find_game_directory(&unique_extract)?;

    // Ensure the shared game folder exists before moving the variant into it.
    if let Some(parent) = target_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory for target: {}", e))?;
    }

    // Move the game directory to the final target.
    std::fs::rename(&game_dir, target_dir)
        .map_err(|e| format!("Failed to move extracted game to target: {}", e))?;
    let _ = std::fs::remove_dir_all(&unique_extract);

    // Scan the target directory for the best executable.
    let executable_path = crate::scanner::find_executable_in_directory(target_dir);
    info!(
        "Scanned install directory '{}' for executable: {:?}",
        target_dir.display(),
        executable_path
    );

    Ok(DownloadedInstall {
        install_path: target_dir.to_string_lossy().to_string(),
        executable_path,
    })
}

/// Download a file from a URL to a local path with streaming, emitting progress events.
async fn download_file(
    app: &AppHandle,
    link_id: &str,
    client: &Client,
    url: &str,
    api_key: Option<&str>,
    path: &Path,
) -> Result<(), String> {
    info!("Downloading file from {} to {:?}", url, path);
    let mut req = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(600));
    // If the URL is the itch API download endpoint, include the API key so the server accepts the request.
    if let Some(key) = api_key {
        if url.contains(crate::itch_api::ITCH_API_BASE) || url.contains("/uploads/") {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
    }
    let resp = req.send().await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download returned status: {}", resp.status()));
    }

    let total = resp.content_length();
    let mut file = tokio::fs::File::create(path).await
        .map_err(|e| format!("Failed to create download file: {}", e))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emitted: u64 = 0;
    const EMIT_INTERVAL: u64 = 100_000; // ~100 KiB

    let emit = |downloaded: u64, total: Option<u64>| {
        let payload = DownloadProgress {
            link_id: link_id.to_string(),
            downloaded,
            total: total.unwrap_or(0),
        };
        let _ = app.emit("download-progress", payload);
    };

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
        let n = chunk.len() as u64;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await
            .map_err(|e| format!("Failed to write download chunk: {}", e))?;
        downloaded += n;

        let should_emit = if let Some(t) = total {
            downloaded >= t || downloaded - last_emitted >= EMIT_INTERVAL
        } else {
            downloaded - last_emitted >= EMIT_INTERVAL
        };

        if should_emit {
            emit(downloaded, total);
            last_emitted = downloaded;
        }
    }

    // Final progress update so the UI reaches 100%.
    emit(total.unwrap_or(downloaded), total);

    Ok(())
}

/// Detect archive format from the file's magic bytes.
fn detect_archive_format(path: &Path) -> Option<&'static str> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 512];
    let n = std::io::Read::read(&mut file, &mut buf).ok()?;
    if n < 4 {
        return None;
    }
    if &buf[0..4] == b"PK\x03\x04" || &buf[0..4] == b"PK\x05\x06" || &buf[0..4] == b"PK\x07\x08" {
        return Some("zip");
    }
    if &buf[0..2] == b"\x1f\x8b" {
        return Some("gz");
    }
    // tar magic: "ustar" at offset 257
    if n >= 262 && &buf[257..262] == b"ustar" {
        return Some("tar");
    }
    None
}

/// Extract a zip or tar.gz archive into the given directory.
fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let mut extension = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if extension.is_empty() || !matches!(extension.as_str(), "zip" | "tar" | "gz" | "tgz") {
        if let Some(magic) = detect_archive_format(archive_path) {
            extension = magic.to_string();
        }
    }
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Failed to open archive: {}", e))?;

    if extension == "zip" {
        let mut archive = zip::read::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read zip archive: {}", e))?;
        archive.extract(target_dir)
            .map_err(|e| format!("Failed to extract zip archive: {}", e))?;
    } else if extension == "tar" {
        let mut archive = tar::Archive::new(file);
        archive.unpack(target_dir)
            .map_err(|e| format!("Failed to extract tar archive: {}", e))?;
    } else if extension == "gz" || extension == "tgz" || archive_path.to_string_lossy().ends_with(".tar.gz") {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(target_dir)
            .map_err(|e| format!("Failed to extract tar archive: {}", e))?;
    } else {
        // Not an archive we understand; just copy it as-is into the target dir.
        let file_name = archive_path.file_name().unwrap_or("game-file".as_ref());
        let dest = target_dir.join(file_name);
        std::fs::copy(archive_path, dest)
            .map_err(|e| format!("Failed to copy downloaded file: {}", e))?;
    }

    Ok(())
}

/// If a directory contains exactly one subdirectory and no files at the root,
/// return that subdirectory as the actual game directory. Otherwise return the root.
fn find_game_directory(root: &Path) -> Result<PathBuf, String> {
    let entries: Vec<std::fs::DirEntry> = std::fs::read_dir(root)
        .map_err(|e| format!("Failed to read extracted directory: {}", e))?
        .filter_map(|e| e.ok())
        .collect();

    let dirs: Vec<&std::fs::DirEntry> = entries.iter().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).collect();
    let files: Vec<&std::fs::DirEntry> = entries.iter().filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)).collect();

    if files.is_empty() && dirs.len() == 1 {
        return Ok(dirs[0].path());
    }

    Ok(root.to_path_buf())
}

fn sanitize_folder_name(name: &str) -> String {
    let mut s = name.trim().replace(|c: char| c.is_ascii_control() || r#"\/:*?"<>|"#.contains(c), "_");
    if s.is_empty() {
        s = "game".to_string();
    }
    s.trim_matches(|c: char| c == ' ' || c == '_' || c == '.').to_string()
}

fn sanitize_file_name(name: &str) -> String {
    let mut s = name.trim().replace(|c: char| c.is_ascii_control() || r#"\/:*?"<>|"#.contains(c), "_");
    if s.is_empty() {
        s = "download".to_string();
    }
    s.trim_matches(|c: char| c == ' ' || c == '.').to_string()
}

fn remove_archive_extension(name: &str) -> &str {
    let lower = name.to_lowercase();
    for ext in [".tar.gz", ".zip", ".tar", ".gz", ".tgz", ".rar", ".7z"] {
        if lower.ends_with(ext) {
            return &name[..name.len() - ext.len()];
        }
    }
    name
}

fn strip_title_prefix(title: &str, raw: &str) -> String {
    let normalized_title = title.to_lowercase().replace([' ', '-', '_'], "");
    let normalized_raw = raw.to_lowercase().replace([' ', '-', '_'], "");
    if normalized_title.is_empty() || !normalized_raw.starts_with(&normalized_title) {
        return raw.to_string();
    }

    let mut raw_chars = raw.char_indices().peekable();
    let mut title_chars = normalized_title.chars().peekable();

    while let Some(tc) = title_chars.peek() {
        if let Some((_, c)) = raw_chars.peek() {
            if c.to_lowercase().next() == Some(*tc) {
                raw_chars.next();
                title_chars.next();
                continue;
            }
        }
        break;
    }

    while let Some((_, c)) = raw_chars.peek() {
        if [' ', '-', '_', '.'].contains(c) {
            raw_chars.next();
        } else {
            break;
        }
    }

    let start = raw_chars.peek().map(|(i, _)| *i).unwrap_or(raw.len());
    raw[start..].to_string()
}

fn platform_label(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "windows" => "Windows".to_string(),
        "linux" => "Linux".to_string(),
        "osx" | "mac" => "macOS".to_string(),
        "android" => "Android".to_string(),
        other => other.to_string(),
    }
}

fn clean_variant_name(title: &str, raw: &str, platforms: Option<&[String]>) -> Option<String> {
    let title = title.trim();
    let mut name = remove_archive_extension(raw).to_string();

    if !title.is_empty() {
        name = strip_title_prefix(title, &name);
    }

    name = name
        .trim_matches(|c: char| c == ' ' || c == '-' || c == '_' || c == '.')
        .to_string();

    if !name.is_empty() {
        return Some(name);
    }

    if let Some(platforms) = platforms {
        let labels: Vec<String> = platforms.iter().map(|p| platform_label(p)).collect();
        if !labels.is_empty() {
            return Some(labels.join(" "));
        }
    }

    None
}

fn ensure_unique_dir(path: &mut PathBuf) {
    if !path.exists() {
        return;
    }
    let base = path.file_name().and_then(|n| n.to_str()).unwrap_or("game").to_string();
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let mut n = 1;
    loop {
        *path = parent.join(format!("{}-{}", base, n));
        if !path.exists() {
            return;
        }
        n += 1;
    }
}

fn ensure_unique_path(path: &mut PathBuf) {
    if !path.exists() {
        return;
    }
    let stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("download").to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let mut n = 1;
    loop {
        let name = if ext.is_empty() {
            format!("{}-{}", stem, n)
        } else {
            format!("{}-{}.{}", stem, n, ext)
        };
        *path = parent.join(name);
        if !path.exists() {
            return;
        }
        n += 1;
    }
}

// Required because reqwest::Response::bytes_stream returns a stream that is not Send?
// Actually we only need StreamExt; keep the trait in scope.
use futures_util::StreamExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_variant_name() {
        assert_eq!(
            clean_variant_name("TinyTowns", "TinyTowns_Windows.zip", None),
            Some("Windows".to_string())
        );
        assert_eq!(
            clean_variant_name("TinyTowns", "Windows.zip", None),
            Some("Windows".to_string())
        );
        assert_eq!(
            clean_variant_name("TinyTowns", "TinyTowns.zip", None),
            None
        );
        assert_eq!(
            clean_variant_name("TinyTowns", "TinyTowns.zip", Some(&["windows".to_string()])),
            Some("Windows".to_string())
        );
        assert_eq!(
            clean_variant_name("TinyTowns", "TinyTowns.zip", Some(&["windows".to_string(), "linux".to_string()])),
            Some("Windows Linux".to_string())
        );
        assert_eq!(
            clean_variant_name("TinyTowns", "Linux", None),
            Some("Linux".to_string())
        );
    }
}
