//! Steamless integration with Proton/Wine discovery
//! Ported from ACCELA's steamless_task.py
//!
//! NOTE: This module is kept for future reference but not currently used.
//! The GUI version of Steamless is now launched via Wine from Settings.

#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Result of Wine/Proton discovery
#[derive(Debug, Clone)]
pub struct WineInstallation {
    pub name: String,
    pub wine_path: PathBuf,
    pub is_proton: bool,
}

/// Find available Proton/Wine installations
pub fn find_wine_installations() -> Vec<WineInstallation> {
    let mut installations = Vec::new();

    // Get home directory
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return installations,
    };

    // Steam paths to search for Proton
    let steam_paths = vec![
        // Native Steam
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".local/share/Steam/compatibilitytools.d"),
        // Flatpak Steam
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/common"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
        // Symlink path (common on some distros)
        home.join(".steam/steam/steamapps/common"),
    ];

    // Search for Proton installations
    for steam_path in &steam_paths {
        if !steam_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(steam_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Look for Proton directories
                if name.to_lowercase().contains("proton") && path.is_dir() {
                    // Check for wine binary in common locations
                    let wine_paths = vec![path.join("files/bin/wine"), path.join("dist/bin/wine")];

                    for wine_path in wine_paths {
                        if wine_path.exists() && wine_path.is_file() {
                            eprintln!("[Steamless] Found Proton: {} at {:?}", name, wine_path);
                            installations.push(WineInstallation {
                                name: name.clone(),
                                wine_path,
                                is_proton: true,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    // Also check for system Wine
    let system_wine_paths = vec![
        PathBuf::from("/usr/bin/wine"),
        PathBuf::from("/usr/local/bin/wine"),
    ];

    for wine_path in system_wine_paths {
        if wine_path.exists() {
            eprintln!("[Steamless] Found system Wine at {:?}", wine_path);
            installations.push(WineInstallation {
                name: "System Wine".to_string(),
                wine_path,
                is_proton: false,
            });
            break;
        }
    }

    // Sort: prefer Proton Experimental, then newer versions, then system Wine
    installations.sort_by(|a, b| {
        let a_exp = a.name.to_lowercase().contains("experimental");
        let b_exp = b.name.to_lowercase().contains("experimental");

        if a_exp && !b_exp {
            std::cmp::Ordering::Less
        } else if !a_exp && b_exp {
            std::cmp::Ordering::Greater
        } else if a.is_proton && !b.is_proton {
            std::cmp::Ordering::Less
        } else if !a.is_proton && b.is_proton {
            std::cmp::Ordering::Greater
        } else {
            // Compare version numbers (higher is better)
            b.name.cmp(&a.name)
        }
    });

    installations
}

/// Get or create the Steamless Wine prefix directory
pub fn get_steamless_prefix() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let prefix = home.join(".local/share/tontondeck/steamless/pfx");

    if let Err(e) = fs::create_dir_all(&prefix) {
        eprintln!("[Steamless] Failed to create prefix directory: {}", e);
    }

    prefix
}

/// Check if .NET Framework 4.8 is installed in the Wine prefix
pub fn check_dotnet_installed(wine: &WineInstallation, prefix: &Path) -> bool {
    // Check for marker file first (quick check)
    let marker = prefix.join(".dotnet48_installed");
    if marker.exists() {
        eprintln!("[Steamless] .NET 4.8 marker found");
        return true;
    }

    // Check for .NET DLL files
    let dotnet_paths = vec![
        prefix.join("drive_c/windows/Microsoft.NET/Framework/v4.0.30319/clr.dll"),
        prefix.join("drive_c/windows/Microsoft.NET/Framework64/v4.0.30319/clr.dll"),
    ];

    for dll_path in &dotnet_paths {
        if dll_path.exists() {
            if let Ok(meta) = fs::metadata(dll_path) {
                if meta.len() > 500_000 {
                    eprintln!(
                        "[Steamless] Found .NET DLL: {:?} ({} bytes)",
                        dll_path,
                        meta.len()
                    );
                    // Create marker for future quick checks
                    let _ = fs::write(&marker, "OK\n");
                    return true;
                }
            }
        }
    }

    // Try registry query as last resort
    let mut env = std::collections::HashMap::new();
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert(
        "WINEPREFIX".to_string(),
        prefix.to_string_lossy().to_string(),
    );

    if wine.is_proton {
        // Set LD_LIBRARY_PATH for Proton
        if let Some(bin_dir) = wine.wine_path.parent() {
            if let Some(proton_root) = bin_dir.parent().and_then(|p| p.parent()) {
                let lib_path = proton_root.join("lib");
                let lib64_path = proton_root.join("lib64");
                let mut ld_path = String::new();
                if lib64_path.exists() {
                    ld_path.push_str(&lib64_path.to_string_lossy());
                    ld_path.push(':');
                }
                ld_path.push_str(&lib_path.to_string_lossy());
                env.insert("LD_LIBRARY_PATH".to_string(), ld_path);
            }
        }
    }

    let result = Command::new(&wine.wine_path)
        .args([
            "reg",
            "query",
            "HKLM\\Software\\Microsoft\\NET Framework Setup\\NDP\\v4\\Full",
            "/v",
            "Release",
        ])
        .envs(&env)
        .output();

    if let Ok(output) = result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // .NET 4.8 has Release >= 528040
        if stdout.contains("528040") || stdout.contains("52804") {
            eprintln!("[Steamless] .NET 4.8 detected via registry");
            let _ = fs::write(&marker, "OK\n");
            return true;
        }
    }

    false
}

/// Find winetricks executable
pub fn find_winetricks() -> Option<PathBuf> {
    // Check common paths
    let paths = vec![
        PathBuf::from("/usr/bin/winetricks"),
        PathBuf::from("/usr/local/bin/winetricks"),
    ];

    for path in paths {
        if path.exists() {
            return Some(path);
        }
    }

    // Try which command
    if let Ok(output) = Command::new("which").arg("winetricks").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

/// Install .NET Framework 4.8 using winetricks
/// Returns true if successful, false otherwise
pub fn install_dotnet<F>(
    wine: &WineInstallation,
    prefix: &Path,
    progress_callback: F,
) -> Result<(), String>
where
    F: Fn(&str),
{
    let winetricks = find_winetricks().ok_or_else(|| {
        "winetricks not found. Please install it: sudo apt install winetricks (or equivalent)"
            .to_string()
    })?;

    progress_callback(
        "Installing .NET Framework 4.8 (this may take 10-20 minutes on first run)...",
    );
    eprintln!("[Steamless] Using winetricks at {:?}", winetricks);

    let mut env = std::collections::HashMap::new();
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert(
        "WINEPREFIX".to_string(),
        prefix.to_string_lossy().to_string(),
    );
    env.insert(
        "WINE".to_string(),
        wine.wine_path.to_string_lossy().to_string(),
    );

    if wine.is_proton {
        env.insert("WINEARCH".to_string(), "win32".to_string());

        // Set LD_LIBRARY_PATH for Proton
        if let Some(bin_dir) = wine.wine_path.parent() {
            if let Some(proton_root) = bin_dir.parent().and_then(|p| p.parent()) {
                let lib_path = proton_root.join("lib");
                let lib64_path = proton_root.join("lib64");
                let mut ld_path = String::new();
                if lib64_path.exists() {
                    ld_path.push_str(&lib64_path.to_string_lossy());
                    ld_path.push(':');
                }
                ld_path.push_str(&lib_path.to_string_lossy());
                env.insert("LD_LIBRARY_PATH".to_string(), ld_path);
            }
        }
    }

    // Run winetricks to install dotnet48
    let mut cmd = Command::new(&winetricks);
    cmd.args(["--unattended", "dotnet48"]);
    cmd.envs(&env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    eprintln!("[Steamless] Running: winetricks --unattended dotnet48");

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start winetricks: {}", e))?;

    // Read output for progress
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("[winetricks] {}", line);
            if line.contains("Executing")
                || line.contains("Installing")
                || line.contains("Downloading")
            {
                progress_callback(&format!(".NET install: {}", line));
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("winetricks failed: {}", e))?;

    if status.success() {
        // Create marker file
        let marker = prefix.join(".dotnet48_installed");
        let _ = fs::write(&marker, "OK\n");
        progress_callback(".NET Framework 4.8 installed successfully!");
        Ok(())
    } else {
        Err(format!(
            "winetricks failed with exit code: {:?}",
            status.code()
        ))
    }
}

/// Run Steamless.CLI.exe on an executable using Wine/Proton
/// Returns true if DRM was successfully removed
pub fn run_steamless<F>(
    wine: &WineInstallation,
    prefix: &Path,
    steamless_cli_path: &Path,
    exe_path: &Path,
    progress_callback: F,
) -> Result<bool, String>
where
    F: Fn(&str),
{
    progress_callback(&format!(
        "Running Steamless on {}...",
        exe_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    // Convert Linux path to Windows path for Wine
    let windows_exe_path = format!("Z:{}", exe_path.to_string_lossy().replace("/", "\\"));

    let mut env = std::collections::HashMap::new();
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert(
        "WINEPREFIX".to_string(),
        prefix.to_string_lossy().to_string(),
    );

    if wine.is_proton {
        env.insert("WINEARCH".to_string(), "win32".to_string());

        // Set LD_LIBRARY_PATH for Proton
        if let Some(bin_dir) = wine.wine_path.parent() {
            if let Some(proton_root) = bin_dir.parent().and_then(|p| p.parent()) {
                let lib_path = proton_root.join("lib");
                let lib64_path = proton_root.join("lib64");
                let mut ld_path = String::new();
                if lib64_path.exists() {
                    ld_path.push_str(&lib64_path.to_string_lossy());
                    ld_path.push(':');
                }
                ld_path.push_str(&lib_path.to_string_lossy());
                env.insert("LD_LIBRARY_PATH".to_string(), ld_path);
            }
        }
    }

    // Build command: wine Steamless.CLI.exe -f <path> --quiet --realign --recalcchecksum
    let mut cmd = Command::new(&wine.wine_path);
    cmd.arg(steamless_cli_path);
    cmd.args([
        "-f",
        &windows_exe_path,
        "--quiet",
        "--realign",
        "--recalcchecksum",
    ]);
    cmd.envs(&env);
    cmd.current_dir(steamless_cli_path.parent().unwrap_or(Path::new(".")));
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    eprintln!(
        "[Steamless] Running: wine {:?} -f {} --quiet --realign --recalcchecksum",
        steamless_cli_path, windows_exe_path
    );

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start Steamless: {}", e))?;

    // Read output
    let mut has_drm = false;
    let mut unpacked_created = false;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let line_lower = line.to_lowercase();
            eprintln!("[Steamless] {}", line);

            // Skip Wine messages
            if line.starts_with("wine:") {
                continue;
            }

            progress_callback(&line);

            // Check for DRM detection
            if line_lower.contains("steam stub")
                || line_lower.contains("steamstub")
                || line_lower.contains("packed with")
            {
                has_drm = true;
            }

            // Check for successful unpack
            if line_lower.contains("unpacked file saved")
                || line_lower.contains("successfully unpacked")
            {
                unpacked_created = true;
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Steamless process failed: {}", e))?;

    eprintln!(
        "[Steamless] Exit code: {:?}, has_drm: {}, unpacked_created: {}",
        status.code(),
        has_drm,
        unpacked_created
    );

    // Check for .unpacked.exe file
    let unpacked_path = PathBuf::from(format!("{}.unpacked.exe", exe_path.to_string_lossy()));

    if unpacked_path.exists() {
        progress_callback("Steam DRM removed successfully!");

        // Backup original and replace with unpacked
        let backup_path = PathBuf::from(format!("{}.original.exe", exe_path.to_string_lossy()));

        // Remove old backup if exists
        if backup_path.exists() {
            let _ = fs::remove_file(&backup_path);
        }

        // Rename original to backup
        if let Err(e) = fs::rename(exe_path, &backup_path) {
            eprintln!("[Steamless] Failed to backup original: {}", e);
            return Err(format!("Failed to backup original exe: {}", e));
        }

        // Rename unpacked to original name
        if let Err(e) = fs::rename(&unpacked_path, exe_path) {
            // Try to restore original
            let _ = fs::rename(&backup_path, exe_path);
            return Err(format!("Failed to rename unpacked exe: {}", e));
        }

        progress_callback(&format!(
            "Patched: {} (original backed up as .original.exe)",
            exe_path.file_name().unwrap_or_default().to_string_lossy()
        ));
        return Ok(true);
    }

    // Exit code 1 typically means no DRM found
    if status.code() == Some(1) {
        progress_callback("No Steam DRM detected in executable");
        return Ok(false);
    }

    if !status.success() {
        return Err(format!(
            "Steamless failed with exit code: {:?}",
            status.code()
        ));
    }

    progress_callback("Steamless completed (no changes needed)");
    Ok(false)
}

/// Full Steamless processing pipeline
/// Discovers Wine, checks/installs .NET, runs Steamless on largest exe
#[allow(unused_variables)]
pub fn process_game_with_steamless<F>(
    game_directory: &Path,
    steamless_cli_path: &Path,
    progress_callback: F,
) -> Result<bool, String>
where
    F: Fn(&str) + Clone,
{
    // Skip on macOS
    #[cfg(target_os = "macos")]
    {
        progress_callback("Steamless not supported on macOS");
        return Ok(false);
    }

    // On Windows, run natively without Wine
    #[cfg(target_os = "windows")]
    {
        return run_steamless_windows(game_directory, steamless_cli_path, progress_callback);
    }

    // Linux/SteamOS - use Wine/Proton
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        progress_callback("Searching for Proton/Wine installation...");

        let installations = find_wine_installations();
        if installations.is_empty() {
            return Err("No Proton or Wine installation found. Please install Steam with Proton, or install Wine.".to_string());
        }

        let wine = &installations[0];
        progress_callback(&format!("Using: {}", wine.name));
        eprintln!(
            "[Steamless] Selected Wine: {} at {:?}",
            wine.name, wine.wine_path
        );

        // Get or create prefix
        let prefix = get_steamless_prefix();
        eprintln!("[Steamless] Using prefix: {:?}", prefix);

        // Check and install .NET if needed
        if !check_dotnet_installed(wine, &prefix) {
            progress_callback(".NET Framework 4.8 not found, installing...");
            install_dotnet(wine, &prefix, progress_callback.clone())?;
        } else {
            progress_callback(".NET Framework 4.8 is installed");
        }

        // Find largest exe in game directory
        progress_callback("Finding main game executable...");
        let exe_path = find_largest_exe(game_directory)?;

        eprintln!("[Steamless] Found largest exe: {:?}", exe_path);
        progress_callback(&format!(
            "Processing: {}",
            exe_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        // Run Steamless
        run_steamless(
            wine,
            &prefix,
            steamless_cli_path,
            &exe_path,
            progress_callback,
        )
    }
}

/// Find the largest .exe file in a directory (likely the main game executable)
fn find_largest_exe(game_directory: &Path) -> Result<PathBuf, String> {
    use walkdir::WalkDir;

    let mut largest: Option<(PathBuf, u64)> = None;

    for entry in WalkDir::new(game_directory)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip non-exe files
        if path.extension().and_then(|e| e.to_str()) != Some("exe") {
            continue;
        }

        // Skip known non-game executables
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        if filename.contains("unins")
            || filename.contains("setup")
            || filename.contains("redist")
            || filename.contains("vcredist")
            || filename.contains("dxsetup")
            || filename.ends_with(".original.exe")
        {
            continue;
        }

        if let Ok(meta) = fs::metadata(path) {
            let size = meta.len();
            if largest.is_none() || size > largest.as_ref().unwrap().1 {
                largest = Some((path.to_path_buf(), size));
            }
        }
    }

    largest
        .map(|(p, _)| p)
        .ok_or_else(|| "No suitable .exe file found in game directory".to_string())
}

#[cfg(target_os = "windows")]
fn run_steamless_windows<F>(
    game_directory: &Path,
    steamless_cli_path: &Path,
    progress_callback: F,
) -> Result<bool, String>
where
    F: Fn(&str),
{
    progress_callback("Finding main game executable...");
    let exe_path = find_largest_exe(game_directory)?;

    let mut cmd = Command::new(steamless_cli_path);
    cmd.args([
        "-f",
        &exe_path.to_string_lossy(),
        "--quiet",
        "--realign",
        "--recalcchecksum",
    ]);
    cmd.current_dir(steamless_cli_path.parent().unwrap_or(Path::new(".")));

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run Steamless: {}", e))?;

    let unpacked_path = PathBuf::from(format!("{}.unpacked.exe", exe_path.to_string_lossy()));

    if unpacked_path.exists() {
        let backup_path = PathBuf::from(format!("{}.original.exe", exe_path.to_string_lossy()));
        let _ = fs::remove_file(&backup_path);
        fs::rename(&exe_path, &backup_path).map_err(|e| format!("Backup failed: {}", e))?;
        fs::rename(&unpacked_path, &exe_path).map_err(|e| format!("Replace failed: {}", e))?;
        progress_callback("Steam DRM removed successfully!");
        return Ok(true);
    }

    if status.code() == Some(1) {
        progress_callback("No Steam DRM detected");
        return Ok(false);
    }

    if !status.success() {
        return Err(format!(
            "Steamless failed with exit code: {:?}",
            status.code()
        ));
    }

    Ok(false)
}
