// download.rs - Download functionality for VSCode server

use crate::{Platform, ServerError, ServerInfo};
use futures_util::StreamExt;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[derive(Deserialize)]
#[allow(dead_code)] // Fields are for deserialization structure, not all are directly used
struct GitHubTag {
    name: String,
    commit: CommitInfo,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Fields are for deserialization structure, not all are directly used
struct CommitInfo {
    sha: String,
}

#[derive(Deserialize)]
struct PackageJson {
    config: ConfigSection,
}

#[derive(Deserialize)]
struct ConfigSection {
    vscode: VscodeConfig,
}

#[derive(Deserialize)]
struct VscodeConfig {
    commit: String,
    // version: String, // Example: if we also wanted the version string like "1.98.2"
    // #[serde(rename = "ref")] // Example: if we wanted the field named "ref"
    // ref_field: String,
}

/// Detects the latest compatible VSCode server version information.
///
/// This function queries the GitHub API for the latest tag of the `CodinGame/monaco-vscode-api`
/// repository. It then fetches the `package.json` for that tag to extract the
/// corresponding VSCode commit SHA (`vscodeRef`).
///
/// Finally, it constructs a `ServerInfo` struct containing the `monaco-vscode-api` version,
/// the VSCode commit SHA, the current platform, and the direct download URL for the server.
///
/// # Errors
///
/// Returns `ServerError` if:
/// - Network requests fail (`ServerError::Network`).
/// - No tags are found for `monaco-vscode-api` (`ServerError::VersionDetectionFailed`).
/// - `package.json` cannot be fetched or parsed (`ServerError::Network`, `ServerError::VersionDetectionFailed`).
/// - The current platform is unsupported (`ServerError::UnsupportedPlatform`).
pub async fn detect_version() -> Result<ServerInfo, ServerError> {
    let client = create_client()?;
    
    // Get latest monaco-vscode-api tag
    let tags: Vec<GitHubTag> = client
        .get("https://api.github.com/repos/CodinGame/monaco-vscode-api/tags")
        .send()
        .await?
        .json()
        .await?;
    
    let latest_tag = tags.first()
        .ok_or_else(|| ServerError::VersionDetectionFailed(
            "No tags found in monaco-vscode-api repository".to_string()
        ))?;
    
    // Get VSCode commit from package.json
    let package_json: PackageJson = client
        .get(format!(
            "https://raw.githubusercontent.com/CodinGame/monaco-vscode-api/{}/package.json",
            latest_tag.name
        ))
        .send()
        .await?
        .json()
        .await?;
    
    let platform = Platform::current()
        .map_err(ServerError::UnsupportedPlatform)?;
    
    let vscode_commit_sha = &package_json.config.vscode.commit;
    let download_url = format!(
        "https://update.code.visualstudio.com/commit:{}/{}/{}",
        vscode_commit_sha,
        platform.server_flavor(),
        platform.url_suffix()
    );
    
    Ok(ServerInfo {
        monaco_api_version: latest_tag.name.clone(),
        vscode_commit: vscode_commit_sha.clone(),
        platform,
        download_url,
    })
}

/// Downloads and extracts the VSCode server based on the provided `ServerInfo`.
///
/// The server is downloaded from `info.download_url` and extracted into a subdirectory
/// named after `info.vscode_commit` within the `target_dir`.
/// If the server directory for the specific commit already exists, the download is skipped.
///
/// # Arguments
///
/// * `info` - A `ServerInfo` struct containing details about the server version to download.
/// * `target_dir` - The base directory where the server (and its version-specific subdirectory)
///                  will be placed. This directory will be created if it doesn't exist.
///
/// # Errors
///
/// Returns `ServerError` if:
/// - `target_dir` cannot be created (`ServerError::Io`).
/// - The download fails (e.g., network error, HTTP error status) (`ServerError::Network`, `ServerError::DownloadFailed`).
/// - The archive extraction fails (`ServerError::ExtractionFailed`, `ServerError::Io`).
pub async fn download_server(info: &ServerInfo, target_dir: &Path) -> Result<(), ServerError> {
    let client = create_client()?;
    
    // Create target directory
    fs::create_dir_all(target_dir)?;
    
    let server_dir = target_dir.join(&info.vscode_commit);
    if server_dir.exists() {
        return Ok(());
    }
    
    println!("Downloading VSCode server...");
    println!("Version: {} ({})", info.monaco_api_version, info.vscode_commit);
    println!("URL: {}", info.download_url);
    
    // Download the archive
    let response = client.get(&info.download_url).send().await?;
    
    if !response.status().is_success() {
        return Err(ServerError::DownloadFailed(
            format!("Failed to download: {}", response.status())
        ));
    }
    
    let total_size = response.content_length().unwrap_or(0);
    let archive_name = if info.platform.uses_zip() {
        format!("vscode-server-{}.zip", info.vscode_commit)
    } else {
        format!("vscode-server-{}.tar.gz", info.vscode_commit)
    };
    
    let archive_path = target_dir.join(archive_name);
    let mut file = File::create(&archive_path)?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        
        if total_size > 0 {
            let progress = (downloaded as f64 / total_size as f64) * 100.0;
            print!("\rDownloading: {:.1}%", progress);
            let _ = std::io::stdout().flush();
        }
    }
    println!("\nDownload complete!");
    
    // Extract the archive
    println!("Extracting server...");
    extract_archive(&archive_path, &server_dir, info.platform)?;
    
    // Clean up archive
    fs::remove_file(archive_path)?;
    
    println!("Server ready at: {}", server_dir.display());
    Ok(())
}

/// Creates an HTTP client
fn create_client() -> Result<reqwest::Client, ServerError> {
    reqwest::Client::builder()
        .user_agent("vscode-server-backend")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(Into::into)
}

/// Extracts the downloaded archive
fn extract_archive(archive_path: &Path, target_dir: &Path, platform: Platform) -> Result<(), ServerError> {
    if platform.uses_zip() {
        extract_zip(archive_path, target_dir)
    } else {
        extract_tar_gz(archive_path, target_dir)
    }
}

/// Extracts a tar.gz archive
fn extract_tar_gz(archive_path: &Path, target_dir: &Path) -> Result<(), ServerError> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    
    let file = File::open(archive_path)?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);
    
    // Create a temporary directory
    let temp_dir = target_dir.with_extension("tmp");
    fs::create_dir_all(&temp_dir)?;
    
    // Extract to temp directory
    archive.unpack(&temp_dir)
        .map_err(|e| ServerError::ExtractionFailed(e.to_string()))?;
    
// Find the vscode directory and rename it
     let entries = fs::read_dir(&temp_dir)?;
    let mut found_dirs = Vec::new();
     for entry in entries {
         let entry = entry?;
         if entry.path().is_dir() {
            found_dirs.push(entry.path());
         }
     }
     
    if found_dirs.len() != 1 {
        return Err(ServerError::ExtractionFailed(
            format!("Expected exactly one directory in archive, found {}", found_dirs.len())
        ));
    }
    
    fs::rename(&found_dirs[0], target_dir)?;
    
     // Clean up temp directory
    if let Err(e) = fs::remove_dir_all(&temp_dir) {
        eprintln!("Warning: Failed to clean up temp directory: {}", e);
    }
    
    Ok(())
}

/// Extracts a zip archive (Windows)
#[cfg(target_os = "windows")]
fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<(), ServerError> {
    use zip::ZipArchive;
    
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| ServerError::ExtractionFailed(e.to_string()))?;
    
    fs::create_dir_all(target_dir)?;
    archive.extract(target_dir)
        .map_err(|e| ServerError::ExtractionFailed(e.to_string()))?;
    
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn extract_zip(_archive_path: &Path, _target_dir: &Path) -> Result<(), ServerError> {
    Err(ServerError::ExtractionFailed(
        "ZIP extraction not supported on this platform".to_string()
    ))
}