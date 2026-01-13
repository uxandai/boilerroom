//! DepotDownloaderMod functions and depot name mapping

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use std::io::Read; // Unused
// use std::path::{Path, PathBuf}; // Unused
use std::sync::OnceLock;

/// Game manifest data extracted from ZIP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameManifestData {
    pub app_id: String,
    pub game_name: String,
    pub install_dir: String,
    pub depots: Vec<DepotInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_token: Option<String>, // From addtoken(appid, "token") in LUA - optional
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotInfo {
    pub depot_id: String,
    pub name: String,
    pub manifest_id: String,
    pub manifest_path: String,
    pub key: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oslist: Option<String>, // Parsed from LUA comment: "windows", "linux", "macos"
}

static DEPOT_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

pub fn get_depot_map() -> &'static HashMap<String, String> {
    DEPOT_NAMES.get_or_init(|| {
        let content = include_str!("../depots.ini");
        let mut map = HashMap::new();
        for line in content.lines() {
            if let Some((id, name)) = line.split_once('=') {
                map.insert(id.trim().to_string(), name.trim().to_string());
            }
        }
        eprintln!(
            "[depot.rs] Parsed {} depot names from embedded depots.ini",
            map.len()
        );
        map
    })
}

pub fn get_known_depot_name(depot_id: &str) -> Option<String> {
    get_depot_map().get(depot_id).cloned()
}

/// Extract manifest ZIP and return game data
#[tauri::command]
pub async fn extract_manifest_zip(zip_path: String) -> Result<GameManifestData, String> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(&zip_path).map_err(|e| format!("Failed to open ZIP: {}", e))?;

    let reader = BufReader::new(file);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("Failed to read ZIP: {}", e))?;

    // Create temp extraction directory
    let temp_dir = std::env::temp_dir().join(format!(
        "boilerroom_extract_{}",
        chrono::Utc::now().timestamp()
    ));
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;

        let outpath = temp_dir.join(file.name());

        if file.is_dir() {
            std::fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }

    // Look for info.json or parse manifest files
    let info_path = temp_dir.join("info.json");
    let game_data = if info_path.exists() {
        let info_content = std::fs::read_to_string(&info_path)
            .map_err(|e| format!("Failed to read info.json: {}", e))?;

        #[derive(Deserialize)]
        struct InfoJson {
            appid: Option<String>,
            app_id: Option<String>,
            name: Option<String>,
            game_name: Option<String>,
            installdir: Option<String>,
            depots: Option<std::collections::HashMap<String, serde_json::Value>>,
        }

        let info: InfoJson = serde_json::from_str(&info_content)
            .map_err(|e| format!("Failed to parse info.json: {}", e))?;

        let app_id = info.appid.or(info.app_id).unwrap_or_default();
        let game_name = info
            .name
            .or(info.game_name)
            .unwrap_or_else(|| format!("App_{}", app_id));
        let install_dir = info.installdir.unwrap_or_else(|| game_name.clone());

        // Parse depots
        let mut depots = Vec::new();
        if let Some(depot_map) = info.depots {
            for (depot_id, depot_val) in depot_map {
                let key = depot_val
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let manifest_id = depot_val
                    .get("manifest")
                    .or(depot_val.get("manifest_id"))
                    .and_then(|v| {
                        v.as_str()
                            .map(|s| s.to_string())
                            .or(v.as_u64().map(|n| n.to_string()))
                    })
                    .unwrap_or_default();

                let size = depot_val.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

                // Look for manifest file
                let manifest_path = temp_dir
                    .join("manifests")
                    .join(format!("{}_{}.manifest", depot_id, manifest_id));

                if !manifest_id.is_empty() {
                    depots.push(DepotInfo {
                        depot_id: depot_id.clone(),
                        name: format!("Depot {}", depot_id), // info.json doesn't usually have depot names
                        manifest_id,
                        manifest_path: manifest_path.to_string_lossy().to_string(),
                        key,
                        size,
                        oslist: None, // info.json doesn't have OS info
                    });
                }
            }
        }

        GameManifestData {
            app_id,
            game_name,
            install_dir,
            depots,
            app_token: None, // info.json format doesn't have tokens
        }
    } else {
        // No info.json - try to parse LUA files (Morrenus API format)
        eprintln!("[extract_manifest_zip] No info.json, looking for LUA files...");

        // Find LUA files
        let lua_files: Vec<_> = std::fs::read_dir(&temp_dir)
            .map_err(|e| format!("Failed to read temp dir: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "lua")
                    .unwrap_or(false)
            })
            .collect();

        eprintln!("[extract_manifest_zip] Found {} LUA files", lua_files.len());

        if lua_files.is_empty() {
            // Check subdirectories
            let mut all_files: Vec<String> = Vec::new();
            fn collect_files(dir: &std::path::Path, files: &mut Vec<String>) {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            collect_files(&path, files);
                        } else {
                            files.push(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
            collect_files(&temp_dir, &mut all_files);
            eprintln!("[extract_manifest_zip] All files in ZIP: {:?}", all_files);
            return Err(format!("No LUA or info.json found. Files: {:?}", all_files));
        }

        let lua_path = lua_files[0].path();
        eprintln!("[extract_manifest_zip] Parsing LUA file: {:?}", lua_path);

        let lua_content = std::fs::read_to_string(&lua_path)
            .map_err(|e| format!("Failed to read LUA file: {}", e))?;

        eprintln!(
            "[extract_manifest_zip] LUA content length: {} bytes",
            lua_content.len()
        );
        eprintln!(
            "[extract_manifest_zip] LUA first 500 chars: {}",
            &lua_content.chars().take(500).collect::<String>()
        );

        // Parse addappid() calls from LUA
        let app_decl_re = regex::Regex::new(
            r#"(?m)^addappid\s*\(\s*(\d+)\s*(?:,\s*\d+\s*,\s*"[^"]*")?\s*\)\s*--\s*(.*)$"#,
        )
        .map_err(|e| format!("Regex error: {}", e))?;

        let depot_decl_re = regex::Regex::new(
            r#"(?m)^addappid\s*\(\s*(\d+)\s*,\s*\d+\s*,\s*"([^"]*)"\s*\)\s*--\s*(.*)$"#,
        )
        .map_err(|e| format!("Regex error: {}", e))?;

        let manifest_re =
            regex::Regex::new(r#"(?m)setManifestid\s*\(\s*(\d+)\s*,\s*"([^"]*)"\s*,\s*(\d+)\s*\)"#)
                .map_err(|e| format!("Manifest regex error: {}", e))?;

        let token_re = regex::Regex::new(r#"(?m)addtoken\s*\(\s*(\d+)\s*,\s*"([^"]*)"\s*\)"#)
            .map_err(|e| format!("Token regex error: {}", e))?;

        let mut app_id = String::new();
        let mut game_name = String::new();
        let mut depots = Vec::new();
        let mut app_token: Option<String> = None;

        // 1. Try to find Main App ID declaration first
        if let Some(cap) = app_decl_re.captures(&lua_content) {
            app_id = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            game_name = cap
                .get(2)
                .map(|m| m.as_str().trim())
                .unwrap_or("")
                .to_string();
            eprintln!(
                "[extract_manifest_zip] Found Main AppID: {} Name: {}",
                app_id, game_name
            );
        }

        // 2. Parse depots
        for cap in depot_decl_re.captures_iter(&lua_content) {
            let depot_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let key = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let comment = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("");

            eprintln!(
                "[extract_manifest_zip] Found depot addappid: id={}, key_len={}, comment={}",
                depot_id,
                key.len(),
                comment
            );

            if app_id.is_empty() {
                app_id = depot_id.to_string();
                game_name = comment.to_string();
                eprintln!(
                    "[extract_manifest_zip] inferred AppID from first depot: {}",
                    app_id
                );
            } else {
                if depot_id == app_id {
                    eprintln!(
                        "[extract_manifest_zip] Skipping depot_id {} as it matches Main AppID",
                        depot_id
                    );
                    continue;
                }

                if !key.is_empty() {
                    let final_name = if comment.is_empty() {
                        format!("Depot {}", depot_id)
                    } else {
                        comment.to_string()
                    };

                    let oslist = {
                        let comment_lower = comment.to_lowercase();
                        if comment_lower.contains("windows") || comment_lower.contains("- win") {
                            Some("windows".to_string())
                        } else if comment_lower.contains("linux") {
                            Some("linux".to_string())
                        } else if comment_lower.contains("mac")
                            || comment_lower.contains("osx")
                            || comment_lower.contains("darwin")
                        {
                            Some("macos".to_string())
                        } else {
                            None
                        }
                    };

                    eprintln!(
                        "[extract_manifest_zip] Adding depot {} with name '{}', os={:?}",
                        depot_id, final_name, oslist
                    );

                    depots.push(DepotInfo {
                        depot_id: depot_id.to_string(),
                        name: final_name,
                        manifest_id: String::new(),
                        manifest_path: String::new(),
                        key: key.to_string(),
                        size: 0,
                        oslist,
                    });
                }
            }
        }

        // Parse setManifestid() entries
        for cap in manifest_re.captures_iter(&lua_content) {
            let depot_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let manifest_id = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let size: u64 = cap
                .get(3)
                .map(|m| m.as_str().parse().unwrap_or(0))
                .unwrap_or(0);

            eprintln!(
                "[extract_manifest_zip] Found setManifestid: depot={}, manifest={}, size={}",
                depot_id, manifest_id, size
            );

            for depot in &mut depots {
                if depot.depot_id == depot_id {
                    depot.manifest_id = manifest_id.to_string();
                    depot.size = size;
                }
            }
        }

        // Parse addtoken()
        for cap in token_re.captures_iter(&lua_content) {
            let token_app_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let token_value = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            if !token_value.is_empty() {
                eprintln!(
                    "[extract_manifest_zip] Found addtoken: app_id={}, token_len={}",
                    token_app_id,
                    token_value.len()
                );
                if app_token.is_none() || token_app_id == app_id {
                    app_token = Some(token_value.to_string());
                }
            }
        }

        // Apply known redist names
        for depot in &mut depots {
            if let Some(known_name) = get_known_depot_name(&depot.depot_id) {
                eprintln!(
                    "[extract_manifest_zip] Recognized generic depot {} as '{}'",
                    depot.depot_id, known_name
                );
                depot.name = known_name;
            }
        }

        // "Largest Depot" rule
        if !game_name.is_empty() {
            let mut max_size = 0;
            let mut max_idx = None;

            for (i, depot) in depots.iter().enumerate() {
                if depot.size > max_size {
                    max_size = depot.size;
                    max_idx = Some(i);
                }
            }

            if let Some(idx) = max_idx {
                let depot = &mut depots[idx];
                if depot.name.starts_with("Depot ") {
                    let new_name = format!("{} Content", game_name);
                    eprintln!("[extract_manifest_zip] Heuristic: Renaming largest depot ({}) from '{}' to '{}'", depot.depot_id, depot.name, new_name);
                    depot.name = new_name;
                }
            }
        }

        // Find manifest files
        let manifest_files: Vec<_> = std::fs::read_dir(&temp_dir)
            .map_err(|e| format!("Failed to read temp dir for manifests: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "manifest")
                    .unwrap_or(false)
            })
            .collect();

        eprintln!(
            "[extract_manifest_zip] Found {} manifest files",
            manifest_files.len()
        );

        for mf in manifest_files {
            let fname = mf.file_name().to_string_lossy().to_string();
            let parts: Vec<&str> = fname.trim_end_matches(".manifest").split('_').collect();
            if parts.len() >= 2 {
                let depot_id = parts[0];
                let manifest_id = parts[1];

                for depot in &mut depots {
                    if depot.depot_id == depot_id {
                        depot.manifest_id = manifest_id.to_string();
                        depot.manifest_path = mf.path().to_string_lossy().to_string();
                        eprintln!(
                            "[extract_manifest_zip] Matched manifest {} to depot {}",
                            manifest_id, depot_id
                        );
                    }
                }
            }
        }

        eprintln!(
            "[extract_manifest_zip] Final: app_id={}, game={}, depots={}",
            app_id,
            game_name,
            depots.len()
        );

        if app_id.is_empty() {
            return Err("Failed to parse LUA file - no addappid found".to_string());
        }

        GameManifestData {
            app_id,
            game_name: if game_name.is_empty() {
                "Unknown Game".to_string()
            } else {
                game_name.clone()
            },
            install_dir: game_name,
            depots,
            app_token,
        }
    };

    Ok(game_data)
}

/// Run DepotDownloaderMod for a specific depot
#[tauri::command]
pub async fn run_depot_downloader(
    depot_downloader_path: String,
    app_id: String,
    depot_id: String,
    manifest_id: String,
    manifest_file: String,
    depot_key: String,
    output_dir: String,
) -> Result<String, String> {
    use std::process::Command;

    // Create keys file with depot key
    let temp_dir = std::env::temp_dir();
    let keys_file = temp_dir.join("boilerroom_depot_keys.txt");
    std::fs::write(&keys_file, format!("{};{}\n", depot_id, depot_key))
        .map_err(|e| format!("Failed to write keys file: {}", e))?;

    // Build command
    let mut cmd = Command::new(&depot_downloader_path);
    cmd.args([
        "-app",
        &app_id,
        "-depot",
        &depot_id,
        "-manifest",
        &manifest_id,
        "-manifestfile",
        &manifest_file,
        "-depotkeys",
        keys_file.to_string_lossy().as_ref(),
        "-max-downloads",
        "25",
        "-dir",
        &output_dir,
        "-validate",
    ]);

    // Execute and capture output
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run DepotDownloaderMod: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Cleanup keys file
    std::fs::remove_file(&keys_file).ok();

    if !output.status.success() {
        return Err(format!("DepotDownloaderMod failed: {}\n{}", stdout, stderr));
    }

    Ok(stdout.to_string())
}

/// Get the configured DepotDownloaderMod path
#[tauri::command]
pub async fn get_depot_downloader_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;

    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let path = store
        .get("depot_downloader_path")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    Ok(path)
}

/// Clean up temporary files after successful installation
#[tauri::command]
pub async fn cleanup_temp_files(app_id: String) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();

    // Clean up downloaded ZIP
    let zip_path = temp_dir.join(format!("boilerroom_{}.zip", app_id));
    if zip_path.exists() {
        std::fs::remove_file(&zip_path).ok();
    }

    // Clean up extracted manifests directory
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("boilerroom_extract_") {
                        std::fs::remove_dir_all(&path).ok();
                    }
                }
            }
        }
    }

    // Clean up depot keys file
    let keys_file = temp_dir.join("boilerroom_depot_keys.txt");
    if keys_file.exists() {
        std::fs::remove_file(&keys_file).ok();
    }

    Ok(())
}
