use std::path::{Path, PathBuf};
use std::process::Command;
use crate::http_constants::{ACCEPT_LANGUAGE, USER_AGENT};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use log::{debug, info, warn};

/// Result of resolving an itch.io download URL.
pub enum ItchDownloadResolution {
    /// A signed URL that can be downloaded directly.
    Direct(String),
    /// Game is browser-only or otherwise not downloadable without a session/login.
    Browser,
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

/// Resolve the best direct download URL for an itch.io game page.
/// Returns `Browser` if the game is HTML/web-only or no direct download form is present.
pub async fn resolve_itch_download_url(client: &Client, url: &str) -> Result<ItchDownloadResolution, String> {
    info!("Resolving itch download URL for {}", url);

    let resp = client.get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept-Language", ACCEPT_LANGUAGE)
        .send().await
        .map_err(|e| format!("Failed to fetch itch page: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("itch page returned status: {}", resp.status()));
    }

    let html = resp.text().await.map_err(|e| format!("Failed to read itch page: {}", e))?;

    // Extract the form data inside a block so the Html/ElementRef values are dropped
    // before the await below, keeping the returned future Send.
    let form_data = {
        let document = Html::parse_document(&html);

        // If this is an HTML/web game, there is an iframe placeholder with a playable URL.
        let iframe_selector = Selector::parse("[data-iframe-url]").map_err(|e| e.to_string())?;
        if document.select(&iframe_selector).next().is_some() {
            debug!("itch page is a web/HTML game; deferring to browser");
            return Ok(ItchDownloadResolution::Browser);
        }

        // Find the download form used by downloadable itch games.
        let form_selector = Selector::parse("form[action=\"/download_url\"]").map_err(|e| e.to_string())?;
        let form = match document.select(&form_selector).next() {
            Some(f) => f,
            None => {
                debug!("No /download_url form found on itch page; treating as browser-only");
                return Ok(ItchDownloadResolution::Browser);
            }
        };

        let mut data = std::collections::HashMap::new();
        let input_selector = Selector::parse("input[type=\"hidden\"]").map_err(|e| e.to_string())?;
        for input in form.select(&input_selector) {
            let name = input.value().attr("name").unwrap_or("");
            let value = input.value().attr("value").unwrap_or("");
            if !name.is_empty() {
                data.insert(name.to_string(), value.to_string());
            }
        }

        if data.get("csrf_token").is_none() || data.get("game_id").is_none() {
            warn!("itch download form is missing required fields; treating as browser-only");
            return Ok(ItchDownloadResolution::Browser);
        }

        data
    };

    // Submit the form to obtain the signed download URL.
    let post_url = "https://itch.io/download_url";
    let post_resp = client.post(post_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36")
        .header("Referer", url)
        .form(&form_data)
        .send().await
        .map_err(|e| format!("Failed to request download_url: {}", e))?;

    if !post_resp.status().is_success() {
        return Err(format!("download_url returned status: {}", post_resp.status()));
    }

    let json = post_resp.json::<serde_json::Value>().await
        .map_err(|e| format!("Failed to parse download_url response: {}", e))?;

    if let Some(direct_url) = json.get("url").and_then(|v| v.as_str()) {
        info!("Resolved itch direct download URL");
        return Ok(ItchDownloadResolution::Direct(direct_url.to_string()));
    }

    // If the response has an error such as requiring purchase/login, fallback to browser.
    debug!("download_url response did not contain a URL: {:?}", json);
    Ok(ItchDownloadResolution::Browser)
}

/// Result of a successful download + extraction.
pub struct DownloadedInstall {
    pub install_path: String,
    pub executable_path: Option<String>,
}

/// Download an itch game and extract it into a subfolder of the given source.
/// The returned install path is the folder that contains the game executable.
pub async fn download_itch_game(
    client: &Client,
    direct_url: &str,
    source_path: &Path,
    title: &str,
) -> Result<DownloadedInstall, String> {
    let base_name = sanitize_folder_name(title);
    let mut target_dir = source_path.join(&base_name);
    ensure_unique_dir(&mut target_dir);

    // Download to a temporary archive file next to the target folder.
    let archive_path = source_path.join(format!("{}.download", base_name));
    let mut unique_archive = archive_path.clone();
    ensure_unique_path(&mut unique_archive);

    info!("Downloading itch game to {:?}", unique_archive);
    download_file(client, direct_url, &unique_archive).await?;

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

    // Move the game directory to the final target.
    std::fs::rename(&game_dir, target_dir)
        .map_err(|e| format!("Failed to move extracted game to target: {}", e))?;
    let _ = std::fs::remove_dir_all(&unique_extract);

    // Scan the target directory for the best executable.
    let scanned = crate::commands::scan_directory_internal(target_dir)
        .map_err(|e| format!("Failed to scan installed directory: {}", e))?;

    let executable_path = scanned.into_iter().next().and_then(|g| g.executable);

    Ok(DownloadedInstall {
        install_path: target_dir.to_string_lossy().to_string(),
        executable_path,
    })
}

/// Download a file from a URL to a local path with streaming.
async fn download_file(client: &Client, url: &str, path: &Path) -> Result<(), String> {
    let resp = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36")
        .send().await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download returned status: {}", resp.status()));
    }

    let mut file = tokio::fs::File::create(path).await
        .map_err(|e| format!("Failed to create download file: {}", e))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await
            .map_err(|e| format!("Failed to write download chunk: {}", e))?;
    }

    Ok(())
}

/// Extract a zip or tar.gz archive into the given directory.
fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let extension = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
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
