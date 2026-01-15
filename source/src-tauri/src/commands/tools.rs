//! Tools section: Steamless GUI and SLSah launcher

use std::path::PathBuf;

/// Launch Steamless.exe via Wine/Proton (GUI version, not CLI)
/// This allows users to manually select and patch game executables
#[tauri::command]
pub async fn launch_steamless_via_wine(steamless_exe_path: String) -> Result<String, String> {
    let path = PathBuf::from(&steamless_exe_path);
    if !path.exists() {
        return Err(format!(
            "Steamless.exe not found at: {}",
            steamless_exe_path
        ));
    }

    #[cfg(target_os = "macos")]
    {
        return Err(
            "Steamless is not supported on macOS. Please run on Linux/SteamOS or Windows."
                .to_string(),
        );
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new(&steamless_exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch Steamless: {}", e))?;
        return Ok("Steamless launched".to_string());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use std::process::Command;
        let home = dirs::home_dir().ok_or("Could not find home directory")?;

        let proton_paths = vec![
            home.join(".local/share/Steam/steamapps/common/Proton - Experimental/files/bin/wine"),
            home.join(".local/share/Steam/steamapps/common/Proton - Experimental/files/bin/wine64"),
            home.join(".local/share/Steam/steamapps/common/Proton 9.0/files/bin/wine"),
            home.join(".local/share/Steam/steamapps/common/Proton 9.0/files/bin/wine64"),
            home.join(".local/share/Steam/steamapps/common/Proton 8.0/files/bin/wine"),
            home.join(".local/share/Steam/steamapps/common/Proton 8.0/files/bin/wine64"),
            home.join(".steam/steam/steamapps/common/Proton - Experimental/files/bin/wine"),
            home.join(".steam/steam/steamapps/common/Proton - Experimental/files/bin/wine64"),
            PathBuf::from("/usr/bin/wine"),
            PathBuf::from("/usr/bin/wine64"),
        ];

        let wine_path = proton_paths.iter()
            .find(|p| p.exists())
            .ok_or("No Wine or Proton installation found. Please install Proton via Steam or install Wine.")?;

        eprintln!("[Steamless] Using Wine: {:?}", wine_path);

        let prefix = home.join(".local/share/boilerroom/steamless/pfx");
        std::fs::create_dir_all(&prefix)
            .map_err(|e| format!("Failed to create Wine prefix: {}", e))?;

        let mut cmd = Command::new(wine_path);
        cmd.arg(&steamless_exe_path);
        cmd.env("WINEPREFIX", prefix.to_string_lossy().to_string());
        cmd.env("WINEDEBUG", "-all");

        if wine_path.to_string_lossy().contains("Proton") {
            if let Some(proton_root) = wine_path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
            {
                let lib_path = proton_root.join("lib");
                let lib64_path = proton_root.join("lib64");
                let mut ld_path = String::new();
                if lib64_path.exists() {
                    ld_path.push_str(&lib64_path.to_string_lossy());
                    ld_path.push(':');
                }
                ld_path.push_str(&lib_path.to_string_lossy());
                cmd.env("LD_LIBRARY_PATH", ld_path);
            }
        }

        cmd.spawn()
            .map_err(|e| format!("Failed to launch Steamless via Wine: {}", e))?;

        Ok(format!(
            "Steamless launched via Wine ({:?})",
            wine_path.file_name().unwrap_or_default()
        ))
    }
}

/// Check if SLSah is installed
#[tauri::command]
pub async fn check_slsah_installed() -> Result<bool, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let slsah_path = home.join("steam-schema-generator/slsah.sh");
    Ok(slsah_path.exists())
}

/// Install SLSah (SLSsteam Achievement Helper)
#[tauri::command]
pub async fn install_slsah() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return Err("SLSah is a Linux/SteamOS tool and is not supported on Windows.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        return Err("SLSah is a Linux/SteamOS tool and is not supported on macOS.".to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        use std::process::Command;
        let output = Command::new("sh")
            .args([
                "-c",
                "curl -L https://github.com/niwia/SLSah/raw/main/install.sh | sh",
            ])
            .output()
            .map_err(|e| format!("Failed to run installer: {}", e))?;

        if output.status.success() {
            Ok("SLSah installed successfully! You can find the desktop shortcut in your applications menu.".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Installation failed: {}", stderr))
        }
    }
}

/// Launch SLSah
#[tauri::command]
pub async fn launch_slsah() -> Result<String, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        return Err("SLSah is only available on Linux/SteamOS.".to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        use std::process::Command;
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let slsah_path = home.join("steam-schema-generator/slsah.sh");

        if !slsah_path.exists() {
            return Err("SLSah is not installed. Please install it first.".to_string());
        }

        let terminals = vec![
            ("konsole", vec!["-e", slsah_path.to_str().unwrap()]),
            ("gnome-terminal", vec!["--", slsah_path.to_str().unwrap()]),
            ("xfce4-terminal", vec!["-e", slsah_path.to_str().unwrap()]),
            ("xterm", vec!["-e", slsah_path.to_str().unwrap()]),
        ];

        for (terminal, args) in terminals {
            if Command::new("which")
                .arg(terminal)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                Command::new(terminal)
                    .args(&args)
                    .spawn()
                    .map_err(|e| format!("Failed to launch SLSah: {}", e))?;
                return Ok(format!("SLSah launched in {}", terminal));
            }
        }

        Err("No supported terminal emulator found (tried konsole, gnome-terminal, xfce4-terminal, xterm)".to_string())
    }
}

/// Check if dotnet runtime is available on the system (version 9+ required)
/// Returns (available, version_string)
#[tauri::command]
pub async fn check_dotnet_available() -> Result<(bool, String), String> {
    use std::process::Command;
    
    let result = Command::new("dotnet")
        .arg("--info")
        .output();
    
    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse version from "Host: Version: X.Y.Z" section
            let version = stdout
                .lines()
                .find(|line| line.trim().starts_with("Version:"))
                .and_then(|line| line.split(':').nth(1))
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            
            // Check if major version is 9+
            let major: u32 = version.split('.').next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            
            let available = major >= 9;
            Ok((available, version))
        }
        _ => Ok((false, "not installed".to_string()))
    }
}
