//! Steamless DRM removal commands

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Game executable info for Steamless processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameExecutable {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub priority: i32,
}

/// Find game executables in a directory, sorted by priority (like ACCELA)
#[tauri::command]
pub async fn find_game_executables(
    game_directory: String,
    game_name: String,
) -> Result<Vec<GameExecutable>, String> {
    use std::fs;
    use walkdir::WalkDir;

    let mut executables: Vec<GameExecutable> = Vec::new();

    // Skip patterns (like ACCELA)
    let skip_patterns = [
        "unins",
        "setup",
        "config",
        "launcher",
        "updater",
        "patch",
        "redist",
        "vcredist",
        "dxsetup",
        "physx",
        "crash",
        "handler",
        "unity",
        ".original.",
    ];

    // Walk directory recursively
    for entry in WalkDir::new(&game_directory)
        .max_depth(3) // Limit depth
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process .exe files
        if path.extension().and_then(|s| s.to_str()) != Some("exe") {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let filename_lower = filename.to_lowercase();

        // Skip unwanted files
        if skip_patterns.iter().any(|p| filename_lower.contains(p)) {
            continue;
        }

        // Get file size
        let size = match fs::metadata(path) {
            Ok(m) => m.len(),
            Err(_) => continue,
        };

        // Skip very small files (< 100KB)
        if size < 100 * 1024 {
            continue;
        }

        // Calculate priority (like ACCELA)
        let priority = calculate_exe_priority(&filename, &game_name, size);

        executables.push(GameExecutable {
            path: path.to_string_lossy().to_string(),
            name: filename,
            size,
            priority,
        });
    }

    // Sort by priority (highest first)
    executables.sort_by(|a, b| b.priority.cmp(&a.priority));

    Ok(executables)
}

/// Calculate priority score for an executable (like ACCELA)
fn calculate_exe_priority(filename: &str, game_name: &str, file_size: u64) -> i32 {
    let filename_lower = filename.to_lowercase();
    let game_name_lower = game_name.to_lowercase();
    let game_name_clean: String = game_name_lower
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    let mut priority = 0i32;

    // High priority: match with game name
    if filename_lower.starts_with(&game_name_clean) {
        priority += 100;
    } else if filename_lower.contains(&game_name_clean) {
        priority += 80;
    }

    // Medium priority: common main exe names
    if ["game.exe", "main.exe", "play.exe", "start.exe"].contains(&filename_lower.as_str()) {
        priority += 50;
    }

    // Bonus for larger files
    if file_size > 50 * 1024 * 1024 {
        // > 50MB
        priority += 30;
    } else if file_size > 10 * 1024 * 1024 {
        // > 10MB
        priority += 20;
    } else if file_size > 5 * 1024 * 1024 {
        // > 5MB
        priority += 10;
    }

    // Penalty for common non-game executables
    if ["editor", "tool", "config", "settings"]
        .iter()
        .any(|w| filename_lower.contains(w))
    {
        priority -= 20;
    }

    // High penalty for crash handlers
    if ["crash", "handler", "debug"]
        .iter()
        .any(|w| filename_lower.contains(w))
    {
        priority -= 50;
    }

    priority.max(0)
}

/// Run Steamless CLI on a game executable to remove DRM
#[tauri::command]
pub async fn run_steamless(steamless_path: String, exe_path: String) -> Result<String, String> {
    use std::process::Command;

    // Check if Steamless.CLI.exe exists
    if !Path::new(&steamless_path).exists() {
        return Err(format!(
            "Steamless.CLI.exe not found at: {}",
            steamless_path
        ));
    }

    // Check if target exe exists
    if !Path::new(&exe_path).exists() {
        return Err(format!("Game executable not found at: {}", exe_path));
    }

    // On macOS/Linux, use mono to run the .exe
    #[cfg(not(target_os = "windows"))]
    let output = {
        Command::new("mono")
            .arg(&steamless_path)
            .arg("-f")
            .arg(&exe_path)
            .arg("--quiet")
            .arg("--realign")
            .arg("--recalcchecksum")
            .output()
            .map_err(|e| {
                format!(
                    "Failed to run Steamless via mono: {}. Is mono installed?",
                    e
                )
            })?
    };

    // On Windows, run directly
    #[cfg(target_os = "windows")]
    let output = {
        Command::new(&steamless_path)
            .arg("-f")
            .arg(&exe_path)
            .arg("--quiet")
            .arg("--realign")
            .arg("--recalcchecksum")
            .output()
            .map_err(|e| format!("Failed to run Steamless: {}", e))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!("Steamless failed: {}\n{}", stdout, stderr));
    }

    // Check if .original file was created (means DRM was removed)
    let original_path = format!("{}.original", exe_path);
    if Path::new(&original_path).exists() {
        Ok(format!("DRM removed successfully from: {}", exe_path))
    } else {
        Ok(format!("Steamless ran but no DRM found in: {}", exe_path))
    }
}

/// Result of Steamless processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamlessResult {
    pub success: bool,
    pub message: String,
    pub processed_file: Option<String>,
}

/// Apply Steamless to a game directory using Wine/Proton
/// This is the full pipeline: find Wine, check .NET, run on largest exe
#[tauri::command]
pub async fn apply_steamless_to_game(
    game_path: String,
    steamless_cli_path: String,
) -> Result<SteamlessResult, String> {
    use crate::steamless::process_game_with_steamless;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    // Verify paths exist
    let game_dir = PathBuf::from(&game_path);
    let steamless_path = PathBuf::from(&steamless_cli_path);

    if !game_dir.exists() {
        return Err(format!("Game directory not found: {}", game_path));
    }

    if !steamless_path.exists() {
        return Err(format!(
            "Steamless.CLI.exe not found at: {}",
            steamless_cli_path
        ));
    }

    // Collect progress messages
    let messages: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let messages_clone = messages.clone();

    // Run the full steamless pipeline (blocking because Wine interaction)
    let result = tokio::task::spawn_blocking(move || {
        process_game_with_steamless(&game_dir, &steamless_path, |msg| {
            eprintln!("[Steamless] {}", msg);
            if let Ok(mut msgs) = messages_clone.lock() {
                msgs.push(msg.to_string());
            }
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    let final_messages = messages.lock().unwrap().join("\n");

    match result {
        Ok(drm_removed) => Ok(SteamlessResult {
            success: true,
            message: if drm_removed {
                format!("DRM removed successfully!\n{}", final_messages)
            } else {
                format!("Steamless completed, no DRM detected.\n{}", final_messages)
            },
            processed_file: None,
        }),
        Err(e) => Err(format!("Steamless failed: {}\n{}", e, final_messages)),
    }
}

