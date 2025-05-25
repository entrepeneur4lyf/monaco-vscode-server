// platform.rs - Platform detection and support

use std::fmt;
use serde::{Deserialize, Serialize};

/// Supported platforms for VSCode server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    LinuxX64,
    LinuxArm64,
    LinuxArmhf,
    DarwinX64,
    DarwinArm64,
    Win32X64,
}

impl Platform {
    /// Detects the current platform
    pub fn current() -> Result<Self, String> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return Ok(Platform::LinuxX64);
        
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return Ok(Platform::LinuxArm64);
        
        #[cfg(all(target_os = "linux", target_arch = "arm"))]
        return Ok(Platform::LinuxArmhf);
        
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return Ok(Platform::DarwinX64);
        
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return Ok(Platform::DarwinArm64);
        
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return Ok(Platform::Win32X64);
        
        #[cfg(not(any(
            all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm")),
            all(target_os = "macos", any(target_arch = "x86_64", target_arch = "aarch64")),
            all(target_os = "windows", target_arch = "x86_64")
        )))]
        return Err(format!("Unsupported platform: {} {}", 
            std::env::consts::OS, 
            std::env::consts::ARCH
        ));
    }
    
    /// Gets the VSCode server flavor string for this platform
    pub fn server_flavor(&self) -> &'static str {
        match self {
            Platform::LinuxX64 => "server-linux-x64",
            Platform::LinuxArm64 => "server-linux-arm64",
            Platform::LinuxArmhf => "server-linux-armhf",
            Platform::DarwinX64 => "server-darwin-x64",
            Platform::DarwinArm64 => "server-darwin-arm64",
            Platform::Win32X64 => "server-win32-x64",
        }
    }
    
    /// Gets the URL suffix for downloading
    pub fn url_suffix(&self) -> &'static str {
        match self {
            Platform::Win32X64 => "archive",
            _ => "stable",
        }
    }
    
    /// Checks if this platform uses zip archives
    pub fn uses_zip(&self) -> bool {
        matches!(self, Platform::Win32X64)
    }
    
    /// From target triple string
    pub fn from_target_triple(target: &str) -> Result<Self, String> {
        match target {
            "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => Ok(Platform::LinuxX64),
            "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => Ok(Platform::LinuxArm64),
            "armv7-unknown-linux-gnueabihf" => Ok(Platform::LinuxArmhf),
            "x86_64-apple-darwin" => Ok(Platform::DarwinX64),
            "aarch64-apple-darwin" => Ok(Platform::DarwinArm64),
            "x86_64-pc-windows-msvc" | "x86_64-pc-windows-gnu" => Ok(Platform::Win32X64),
            _ => Err(format!("Unsupported target: {}", target)),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.server_flavor())
    }
}