# monaco-vscode-server
[![crates.io](https://img.shields.io/crates/v/monaco-vscode-server.svg)](https://crates.io/crates/monaco-vscode-server) [![crates.io downloads](https://img.shields.io/crates/d/monaco-vscode-server.svg)](https://crates.io/crates/monaco-vscode-server)

A Rust crate for managing VSCode server backend for applications using [monaco-vscode-api](https://github.com/CodinGame/monaco-vscode-api).

Perfect for Tauri applications that need a VSCode-compatible backend for the Monaco editor.

## Features

- 🔄 **Automatic version matching** - Always downloads the correct VSCode server version for your monaco-vscode-api
- 🚀 **Simple API** - Easy integration with Tauri or any Rust application
- 🌍 **Cross-platform** - Works on Windows, macOS, and Linux
- ⚡ **Async/await** - Built on Tokio for non-blocking operations

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
monaco-vscode-server = "0.1.0" # Or the latest version
```

## Quick Start

### Basic Usage

```rust
use monaco_vscode_server::{VscodeServerManager, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create manager with default config
    let mut manager = VscodeServerManager::new().await?;
    
    // Ensure server is downloaded
    manager.ensure_server().await?;
    
    // Start the server
    manager.start().await?;
    
    println!("VSCode server running at: {}", manager.url());
    
    // Your application code here...
    
    // Server stops automatically when manager is dropped
    Ok(())
}
```

### Tauri Integration

In your Tauri app's `main.rs`:

```rust
use tauri::{Manager, State};
use monaco_vscode_server::{TauriVscodeServer, TauriConfig};

// State type for the server
struct ServerState(Arc<TauriVscodeServer>);

#[tauri::command]
async fn get_server_info(state: State<'_, ServerState>) -> Result<serde_json::Value, String> {
    state.0.get_info().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn restart_server(state: State<'_, ServerState>) -> Result<(), String> {
    state.0.restart().await.map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            // Initialize VSCode server
            let server = runtime.block_on(async {
                let server = TauriVscodeServer::new(TauriConfig::default()).await?;
                server.initialize().await?;
                Ok::<_, Box<dyn std::error::Error>>(server)
            })?;
            
            app.manage(ServerState(Arc::new(server)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_server_info,
            restart_server
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

In your frontend (with monaco-vscode-api):

```javascript
// Get server configuration from Tauri
const serverInfo = await invoke('get_server_info');

// Initialize monaco-vscode-api with the server URL
import { initialize } from '@codingame/monaco-vscode-api';

await initialize({
    serviceConfig: {
        baseUrl: serverInfo.serverUrl,
        connectionToken: serverInfo.serviceConfig.connectionToken
    }
});
```

## Configuration

### Custom Server Configuration

```rust
use monaco_vscode_server::{VscodeServerManager, ServerConfig};

let config = ServerConfig {
    port: 8001,
    host: "127.0.0.1".to_string(),
    server_dir: PathBuf::from("./my-vscode-server"),
    disable_telemetry: true,
    connection_token: None,
    args: vec![
        "--accept-server-license-terms".to_string(),
    ],
};

let manager = VscodeServerManager::with_config(config).await?;
```

## API Reference

### `VscodeServerManager`

Main manager for the VSCode server.

- `new()` - Create with default configuration
- `with_config(config)` - Create with custom configuration
- `ensure_server()` - Download server if needed
- `start()` - Start the server
- `stop()` - Stop the server
- `is_running()` - Check if server is running
- `url()` - Get the server URL
- `info()` - Get server version information

### `TauriVscodeServer`

Helper specifically for Tauri applications.

- `new(config)` - Create new instance
- `initialize()` - Download and optionally start server
- `get_info()` - Get info for frontend
- `stop()` - Stop the server
- `restart()` - Restart the server

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | ✅ |
| macOS | x86_64 | ✅ |
| macOS | ARM64 | ✅ |
| Linux | x86_64 | ✅ |
| Linux | ARM64 | ✅ |
| Linux | ARMv7 | ✅ |

## Server Storage

By default, servers are stored in:
- Linux/macOS: `~/.cache/monaco-vscode-server/`
- Windows: `%LOCALAPPDATA%\monaco-vscode-server\`

Override the server directory using the `server_dir` field in `ServerConfig` or by setting the `VSCODE_SERVER_DIR` environment variable.

## Version Matching

The crate automatically:
1. Queries the latest monaco-vscode-api release
2. Reads its `package.json` to find the VSCode commit
3. Downloads the exact matching server version

This ensures compatibility between monaco-vscode-api and the VSCode server.

## Troubleshooting

### Server fails to start

Check that the port isn't already in use. The default port is 8001.
If you need to change it:
```rust
let config = ServerConfig {
    port: 8002, // Try a different port
    ..Default::default()
};
```

### Download fails

The crate respects system proxy settings. For custom proxy:
```bash
export HTTPS_PROXY=http://proxy.example.com:8080
```

### Permission errors

Ensure the server directory is writable. Use a custom directory:
```rust
let config = ServerConfig {
    server_dir: PathBuf::from("/tmp/vscode-server"),
    ..Default::default()
};
```

## License

MIT - See LICENSE file for details.

**Note**: The VSCode server itself is subject to Microsoft's license terms.