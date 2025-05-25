//! `codingame-monaco-vscode-server` is a Rust crate for managing the VSCode server backend.
//! 
//! It provides functionalities to download, start, stop, and manage the VSCode server,
//! which is used by `monaco-vscode-api` to provide a Monaco editor with VSCode services.
//! 
//! ## Features
//! - `embed`: Enables embedding the VSCode server binary directly into your application.
//!            When this feature is active, the server can be extracted and run without
//!            needing a separate download step at runtime, unless overridden.
//! 
//! ## Quick Start
//! 
//! ```rust,no_run
//! use codingame_monaco_vscode_server::{VscodeServerManager, ServerConfig};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut manager = VscodeServerManager::new().await?;
//!     manager.ensure_server().await?; // Downloads if not present or embedded
//!     manager.start().await?;
//! 
//!     println!("Server is running at {}", manager.url());
//!     println!("Server info: {:?}", manager.info());
//! 
//!     // Keep the server running for a bit (e.g., in a real app, it runs until shutdown)
//!     tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
//! 
//!     manager.stop().await?;
//!     println!("Server stopped.");
//! 
//!     Ok(())
//! }
//! ```

// Module declarations - these correspond to other files in src/
mod download;
mod platform;

// Re-export commonly used types at the crate root
pub use platform::Platform;

// Standard library imports
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for the `codingame-monaco-vscode-server` crate.
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    /// An error occurred during network operations (e.g., downloading the server).
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    /// The VSCode server executable or related files were not found where expected.
    #[error("Server not found")]
    ServerNotFound,
    
    /// An attempt was made to start a server that is already running.
    #[error("Server already running")]
    AlreadyRunning,
    
    /// An operation was attempted that requires the server to be running, but it is not.
    #[error("Server not running")]
    NotRunning,
    
    /// The server process failed to start.
    #[error("Failed to start server: {0}")]
    StartFailed(String),
    
    /// The current operating system or architecture is not supported.
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),
    
    /// Failed to detect the required VSCode server version.
    #[error("Version detection failed: {0}")]
    VersionDetectionFailed(String),
    
    /// An error occurred while extracting the downloaded server archive.
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    
    /// An error occurred during the download process (e.g., HTTP error status).
    #[error("Download failed: {0}")]
    DownloadFailed(String),
}

/// Configuration for the VSCode server instance.
///
/// This struct allows customization of various server parameters such as port, host,
/// installation directory, and command-line arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
        /// The port number on which the server will listen.
    pub port: u16,
        /// The hostname or IP address to bind the server to.
    pub host: String,
        /// Additional command-line arguments to pass to the VSCode server executable.
    pub args: Vec<String>,
        /// The directory where the VSCode server will be installed or looked for.
    pub server_dir: PathBuf,
        /// If `true`, attempts to disable telemetry by passing relevant arguments to the server.
    pub disable_telemetry: bool,
        /// An optional connection token for securing the server.
    pub connection_token: Option<String>,
}

/// Provides default settings for `ServerConfig`.
/// - `port`: 8001
/// - `host`: "127.0.0.1"
/// - `args`: `["--accept-server-license-terms"]`
/// - `server_dir`: A platform-specific cache directory or `./vscode-server`.
/// - `disable_telemetry`: `true`
/// - `connection_token`: `None`
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8001,
            host: "127.0.0.1".to_string(),
            args: vec!["--accept-server-license-terms".to_string()],
            server_dir: default_server_dir(),
            disable_telemetry: true,
            connection_token: None,
        }
    }
}

/// Holds information about the detected or embedded VSCode server.
///
/// This includes the version of `monaco-vscode-api` it's compatible with,
/// the specific VSCode commit SHA, the target platform, and the download URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
        /// The version of `monaco-vscode-api` this server is intended to work with.
    pub monaco_api_version: String,
        /// The specific commit SHA of the VSCode repository this server is built from.
    pub vscode_commit: String,
        /// The platform (OS and architecture) for which this server is intended.
    pub platform: Platform,
        /// The direct URL from which this version of the server can be downloaded.
    pub download_url: String,
}

/// Manages the lifecycle of a VSCode server instance.
///
/// This struct is the primary entry point for interacting with the server. It handles
/// downloading (if not embedded and not already present), starting, stopping, and querying
/// the state of the VSCode server.
///
/// Instances are typically created using `VscodeServerManager::new()` or `VscodeServerManager::with_config()`.
/// The server process is cleaned up when the `VscodeServerManager` instance is dropped.
pub struct VscodeServerManager {
    config: ServerConfig,
    info: Option<ServerInfo>,
    process: Arc<Mutex<Option<Child>>>,
    server_path: Option<PathBuf>,
}

impl VscodeServerManager {
    /// Creates a new `VscodeServerManager` with default configuration.
    ///
    /// This is an asynchronous operation as it may involve initial setup.
    ///
    /// # Errors
    ///
    /// Currently, this constructor does not return errors but is `async` for future compatibility
    /// and consistency with `with_config`.
    // Constructor
    pub async fn new() -> Result<Self, ServerError> {
        Self::with_config(ServerConfig::default()).await
    }
    
        /// Creates a new `VscodeServerManager` with the specified configuration.
    ///
    /// This is an asynchronous operation.
    ///
    /// # Arguments
    ///
    /// * `config` - A `ServerConfig` struct with custom settings for the server.
    ///
    /// # Errors
    ///
    /// Currently, this constructor does not return errors but is `async` for future compatibility.
    // Constructor with custom config
    pub async fn with_config(config: ServerConfig) -> Result<Self, ServerError> {
        Ok(Self {
            config,
            info: None,
            process: Arc::new(Mutex::new(None)),
            server_path: None,
        })
    }
    
        /// Ensures that the VSCode server is available, downloading it if necessary.
    ///
    /// This method performs the following steps:
    /// 1. If the `embed` feature is enabled, it first tries to extract an embedded server.
    /// 2. If no embedded server is found or the feature is disabled, it attempts to detect
    ///    the latest compatible VSCode server version.
    /// 3. It checks if this version is already present in the configured `server_dir`.
    /// 4. If not present, it downloads and extracts the server.
    ///
    /// This method must be called before `start()` if the server's presence is not guaranteed.
    /// It is an asynchronous operation due to potential network I/O.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if:
    /// - Version detection fails (`ServerError::VersionDetectionFailed`).
    /// - Downloading fails (`ServerError::Network`, `ServerError::DownloadFailed`).
    /// - Extraction fails (`ServerError::ExtractionFailed`, `ServerError::Io`).
    /// - The platform is unsupported (`ServerError::UnsupportedPlatform`).
    // Ensure server is available (download if needed)
    pub async fn ensure_server(&mut self) -> Result<(), ServerError> {        
        // Otherwise download
        let info = download::detect_version().await?;
        self.info = Some(info.clone());
        
        let server_path = self.config.server_dir.join(&info.vscode_commit);
        
        if !server_path.exists() {
            download::download_server(&info, &self.config.server_dir).await?;
        }
        
        self.server_path = Some(server_path);
        Ok(())
    }
    
        /// Starts the VSCode server process.
    ///
    /// Before calling `start`, `ensure_server` should typically be called to make sure
    /// the server binaries are available.
    /// The server will be started with the configuration provided during the manager's creation.
    ///
    /// This is an asynchronous operation.
    ///
    /// # Errors
    ///
    /// Returns `ServerError` if:
    /// - The server is already running (`ServerError::AlreadyRunning`).
    /// - The server path has not been determined (e.g., `ensure_server` was not called) (`ServerError::ServerNotFound`).
    /// - The server executable cannot be found at the expected path (`ServerError::ServerNotFound`).
    /// - The server process fails to start (`ServerError::StartFailed`, `ServerError::Io`).
    // Start the server
    pub async fn start(&self) -> Result<(), ServerError> {
        let mut process_guard = self.process.lock().await;
        
        if process_guard.is_some() {
            return Err(ServerError::AlreadyRunning);
        }
        
        let server_path = self.server_path.as_ref()
            .ok_or(ServerError::ServerNotFound)?;
        
        let executable = self.get_executable_path(server_path)?;
        
        let mut cmd = Command::new(&executable);
        
        // Configure command
        cmd.arg("--port").arg(self.config.port.to_string())
           .arg("--host").arg(&self.config.host);
        
        if self.config.disable_telemetry {
            cmd.arg("--disable-telemetry");
        }
        
match &self.config.connection_token {
    Some(token) => {
        cmd.arg("--connection-token").arg(token);
    }
    None => {
        cmd.arg("--without-connection-token");
    }
}
        
        for arg in &self.config.args {
            cmd.arg(arg);
        }
        
        // Start process
        let child = cmd.spawn()
            .map_err(|e| ServerError::StartFailed(e.to_string()))?;
        
        *process_guard = Some(child);
        
        // Wait for server to initialize
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        Ok(())
    }
    
        /// Stops the VSCode server process if it is running.
    ///
    /// This is an asynchronous operation.
    ///
    /// # Errors
    ///
    /// Returns `ServerError::NotRunning` if the server was not running.
    /// May return `ServerError::Io` if there's an issue killing the process, though this is rare.
    // Stop the server
    pub async fn stop(&self) -> Result<(), ServerError> {
        let mut process_guard = self.process.lock().await;
        
        if let Some(mut child) = process_guard.take() {
            child.kill()?;
            child.wait()?;
            Ok(())
        } else {
            Err(ServerError::NotRunning)
        }
    }
    
        /// Checks if the VSCode server process is currently running.
    ///
    /// This method checks the status of the underlying process.
    /// It is an asynchronous operation as it involves locking the process state.
    // Check if running
    pub async fn is_running(&self) -> bool {
        let mut process_guard = self.process.lock().await;
        
        if let Some(ref mut child) = *process_guard {
            match child.try_wait() {
                Ok(None) => true,
                _ => {
                    *process_guard = None;
                    false
                }
            }
        } else {
            false
        }
    }
    
        /// Returns the URL (host and port) where the server is expected to be listening.
    ///
    /// This is constructed from the `host` and `port` in the `ServerConfig`.
    /// It does not guarantee that the server is actually listening on this URL, only that
    /// this is its configured address.
    // Get server URL
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }
    
        /// Returns a reference to the `ServerInfo` if the server version has been determined
    /// (e.g., after `ensure_server` has been called).
    ///
    /// Returns `None` if server information is not yet available.
    // Get server info
    pub fn info(&self) -> Option<&ServerInfo> {
        self.info.as_ref()
    }
    
    // Helper to get executable path
    fn get_executable_path(&self, server_path: &Path) -> Result<PathBuf, ServerError> {
        let exe = if cfg!(target_os = "windows") {
            server_path.join("bin").join("code-server.cmd")
        } else {
            server_path.join("bin").join("code-server")
        };
        
        if !exe.exists() {
            return Err(ServerError::ServerNotFound);
        }
        
        Ok(exe)
    }
    
    /// Returns a reference to the current `ServerConfig`.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

/// Ensures the server process is stopped when the `VscodeServerManager` goes out of scope.
// Cleanup on drop
impl Drop for VscodeServerManager {
    fn drop(&mut self) {
        if let Ok(mut process_guard) = self.process.try_lock() {
            if let Some(mut child) = process_guard.take() {
                let _ = child.kill();
            }
        }
    }
}

/// Configuration specific to using `VscodeServerManager` within a Tauri application.
///
/// This struct wraps a `ServerConfig` and adds Tauri-specific options like auto-starting
/// the server or stopping it on application exit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriConfig {
        /// If `true`, the VSCode server will be automatically started after `initialize` is called.
    pub auto_start: bool,
        /// If `true`, the VSCode server will be automatically stopped when the `TauriVscodeServer`
    /// instance (or its underlying `VscodeServerManager`) is dropped. This is generally desired
    /// for Tauri applications to clean up the server process on app exit.
    pub stop_on_exit: bool,
        /// The underlying `ServerConfig` to be used for the VSCode server instance.
    pub server: ServerConfig,
}

/// Provides default settings for `TauriConfig`.
/// - `auto_start`: `true`
/// - `stop_on_exit`: `true`
/// - `server`: `ServerConfig::default()`
impl Default for TauriConfig {
    fn default() -> Self {
        Self {
            auto_start: true,
            stop_on_exit: true,
            server: ServerConfig::default(),
        }
    }
}

/// A wrapper around `VscodeServerManager` tailored for use in Tauri applications.
///
/// This struct simplifies the integration of the VSCode server with Tauri by managing
/// the server's lifecycle based on `TauriConfig` settings. It's designed to be held
/// in Tauri's application state.
///
/// The underlying `VscodeServerManager` is wrapped in an `Arc<Mutex<>>` to allow
/// shared mutable access from Tauri commands.
pub struct TauriVscodeServer {
    manager: Arc<Mutex<VscodeServerManager>>,
    config: TauriConfig,
}

impl TauriVscodeServer {
    /// Creates a new `TauriVscodeServer` with the given Tauri-specific configuration.
    ///
    /// This will internally create a `VscodeServerManager`.
    ///
    /// # Arguments
    ///
    /// * `config` - A `TauriConfig` struct.
    ///
    /// # Errors
    ///
    /// Propagates errors from `VscodeServerManager::with_config` (though currently it doesn't error).
    pub async fn new(config: TauriConfig) -> Result<Self, ServerError> {
        let manager = VscodeServerManager::with_config(config.server.clone()).await?;
        
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
            config,
        })
    }
    
        /// Initializes the VSCode server.
    ///
    /// This involves ensuring the server is downloaded/extracted (via `ensure_server`)
    /// and, if `config.auto_start` is true, starting the server.
    ///
    /// Returns information about the server if successful.
    ///
    /// # Errors
    ///
    /// Propagates errors from `VscodeServerManager::ensure_server` and `VscodeServerManager::start`.
    pub async fn initialize(&self) -> Result<ServerInfo, ServerError> {
        let mut manager = self.manager.lock().await;
        manager.ensure_server().await?;
        
        if self.config.auto_start {
            manager.start().await?;
        }
        
        manager.info().cloned().ok_or(ServerError::ServerNotFound)
    }
    
        /// Returns the URL where the VSCode server is expected to be listening.
    ///
    /// This is an asynchronous operation as it requires locking the underlying manager.
    pub async fn get_url(&self) -> String {
        let manager = self.manager.lock().await;
        manager.url()
    }
    
        /// Returns a JSON representation of the server's information and configuration.
    ///
    /// This is useful for providing server details to a Tauri frontend.
    /// The JSON object includes `serverUrl`, `monacoApiVersion`, `vscodeCommit`,
    /// `platform`, and `serviceConfig` (with `baseUrl` and `connectionToken`).
    ///
    /// # Errors
    ///
    /// Returns `ServerError::ServerNotFound` if the server info hasn't been determined yet.
    pub async fn get_info(&self) -> Result<serde_json::Value, ServerError> {
        let manager = self.manager.lock().await;
        let info = manager.info().ok_or(ServerError::ServerNotFound)?;
        
        Ok(serde_json::json!({
            "serverUrl": manager.url(),
            "monacoApiVersion": info.monaco_api_version,
            "vscodeCommit": info.vscode_commit,
            "platform": info.platform.to_string(),
            "serviceConfig": {
                "baseUrl": manager.url(),
                "connectionToken": manager.config().connection_token,
            }
        }))
    }
    
    /// Stops the VSCode server process.
    ///
    /// This is an asynchronous operation.
    ///
    /// # Errors
    ///
    /// Propagates errors from `VscodeServerManager::stop`.
    pub async fn stop(&self) -> Result<(), ServerError> {
        let manager = self.manager.lock().await;
        manager.stop().await
    }
    
        /// Restarts the VSCode server process.
    ///
    /// This involves stopping the server, waiting briefly, and then starting it again.
    /// This is an asynchronous operation.
    ///
    /// # Errors
    ///
    /// Propagates errors from `VscodeServerManager::stop` and `VscodeServerManager::start`.
pub async fn restart(&self) -> Result<(), ServerError> {
    self.stop().await?;
     tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    {
        let manager = self.manager.lock().await;
        manager.start().await
    }
 }
}

// Private helper functions

/// Determines the default directory for storing/finding the VSCode server.
///
/// The lookup order is:
/// 1. The value of the `VSCODE_SERVER_DIR` environment variable, if set.
/// 2. A subdirectory named `vscode-server-backend` within the system's cache directory
///    (e.g., `~/.cache/vscode-server-backend` on Linux).
/// 3. A local directory named `vscode-server` in the current working directory (`./vscode-server`)
///    as a fallback if the system cache directory cannot be determined.
fn default_server_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VSCODE_SERVER_DIR") {
        return PathBuf::from(dir);
    }
    
    if let Some(cache_dir) = dirs::cache_dir() {
        return cache_dir.join("vscode-server-backend");
    }
    
    PathBuf::from("./vscode-server")
}

// Re-exports for convenience
pub use download::download_server;