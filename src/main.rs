use clap::{Parser, Subcommand};
use codingame_monaco_vscode_server::{ServerConfig, VscodeServerManager};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "A CLI tool to manage the VSCode Server for monaco-vscode-api.", long_about = "This utility allows you to download, start, and manage the VSCode Server backend required by the monaco-vscode-api library. It simplifies the process of setting up the server environment.")]
/// Main CLI entry point for managing the VSCode Server.
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
/// Defines the available subcommands for the CLI.
enum Commands {
    /// Ensures the VSCode server is downloaded (if needed) and then starts it.
    /// This command will keep running until Ctrl+C is pressed.
    Start(StartArgs),
    /// Ensures the VSCode server is downloaded to the specified directory.
    /// If the server (matching the version required by the embedded monaco-vscode-api) is already present, this command does nothing.
    Download(DownloadArgs),
}

#[derive(clap::Args)] // Changed Parser to Args for subcommand structs
/// Arguments for the `start` subcommand.
struct StartArgs {
    /// Specifies the port number on which the VSCode server will listen.
    /// If not provided, a default port will be used (see ServerConfig::default()).
    /// Can also be set using the VSCODE_PORT environment variable.
    #[arg(long, short, env = "VSCODE_PORT")]
    port: Option<u16>,

    /// Specifies the host address the VSCode server will bind to.
    /// Defaults to "127.0.0.1".
    /// Can also be set using the VSCODE_HOST environment variable.
    #[arg(long, short = 'H', env = "VSCODE_HOST")]
    host: Option<String>,

    /// Specifies the directory where the VSCode server will be installed or looked for.
    /// If not provided, a default directory will be used (see ServerConfig::default()).
    /// Can also be set using the VSCODE_SERVER_DIR environment variable.
    #[arg(long, env = "VSCODE_SERVER_DIR")]
    server_dir: Option<PathBuf>,

    /// Allows passing additional, custom arguments directly to the VSCode server executable.
    /// For example, to enable verbose logging: --extra-args "--log=trace"
    #[arg(long)]
    extra_args: Vec<String>,
}

#[derive(clap::Args)] // Changed Parser to Args for subcommand structs
/// Arguments for the `download` subcommand.
struct DownloadArgs {
    /// Specifies the directory where the VSCode server will be downloaded and installed.
    /// If not provided, a default directory will be used (see ServerConfig::default()).
    /// Can also be set using the VSCODE_SERVER_DIR environment variable.
    #[arg(long, env = "VSCODE_SERVER_DIR")]
    server_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start(args) => {
            let mut config = ServerConfig::default();
            if let Some(port) = args.port {
                config.port = port;
            }
            if let Some(host) = args.host {
                config.host = host;
            }
            if let Some(server_dir) = args.server_dir {
                config.server_dir = server_dir;
            }
            config.args.extend(args.extra_args);

            println!("Starting server with config: {:?}", config);
            let mut manager = VscodeServerManager::with_config(config).await?;
            
            if let Err(e) = manager.ensure_server().await {
                eprintln!("Error ensuring server is available: {}", e);
                eprintln!("If this is a download or extraction error, please check your network connection and permissions.");
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            println!("VSCode server files are ready.");

            if let Err(e) = manager.start().await {
                eprintln!("Error starting server: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            println!("VSCode server started successfully on {}", manager.url());
            println!("Press Ctrl+C to stop the server.");

            // Keep the main thread alive until Ctrl+C or server stops for another reason
            tokio::signal::ctrl_c().await?;
            println!("Ctrl+C received, stopping server...");
            manager.stop().await?;
            println!("Server stopped.");
        }
        Commands::Download(args) => {
            let mut config = ServerConfig::default();
            if let Some(server_dir) = args.server_dir {
                config.server_dir = server_dir;
            }

            println!("Ensuring server is downloaded to: {:?}", config.server_dir);
            let mut manager = VscodeServerManager::with_config(config).await?;
            if let Err(e) = manager.ensure_server().await {
                eprintln!("Error ensuring server is available: {}", e);
                 eprintln!("If this is a download or extraction error, please check your network connection and permissions.");
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            println!("VSCode server download/extraction complete.");
        }
    }

    Ok(())
}
