use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

// SSH Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub ip: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub private_key_path: String,
    #[serde(default)]
    pub is_local: bool,
}

// Search result from Morrenus API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub game_id: String,
    pub game_name: String,
    pub manifest_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_image: Option<String>,
}

// ============================================================================
// CONNECTION COMMANDS
// ============================================================================

/// Check if the Steam Deck is reachable (ping via TCP connect)
#[tauri::command]
pub async fn check_deck_status(ip: String, port: u16) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);

    // Try TCP connect with timeout (simulates ping + port check)
    match TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(3),
    ) {
        Ok(_) => Ok("online".to_string()),
        Err(_) => Ok("offline".to_string()),
    }
}

/// Test SSH connection with credentials
#[tauri::command]
pub async fn test_ssh(config: SshConfig) -> Result<String, String> {
    use std::net::{IpAddr, SocketAddr};

    // Validate and parse IP address
    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;

    let addr = SocketAddr::new(ip, config.port);

    // Connect TCP first
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("Connection failed: {} ({}:{})", e, config.ip, config.port))?;

    // Create SSH session
    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Try authentication (password auth since user doesn't use SSH keys)
    if !config.password.is_empty() {
        sess.userauth_password(&config.username, &config.password)
            .map_err(|e| format!("SSH authentication failed: {}", e))?;
    } else if !config.private_key_path.is_empty() {
        let key_path = Path::new(&config.private_key_path);
        sess.userauth_pubkey_file(&config.username, None, key_path, None)
            .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else {
        return Err("Password is required".to_string());
    }

    if !sess.authenticated() {
        return Err("Authentication failed".to_string());
    }

    // Run a simple command to verify
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec("echo 'SSH OK'")
        .map_err(|e| format!("Failed to exec command: {}", e))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| format!("Failed to read output: {}", e))?;

    channel.wait_close().ok();

    Ok(output.trim().to_string())
}

// ============================================================================
// SEARCH COMMANDS
// ============================================================================

/// Search bundles from Morrenus API
#[tauri::command]
pub async fn search_bundles(
    query: String,
    app_handle: tauri::AppHandle,
) -> Result<Vec<SearchResult>, String> {
    println!("[DEBUG] search_bundles called with query: {}", query);

    // Get API key from store
    let api_key = get_api_key_internal(&app_handle)?;

    if api_key.is_empty() {
        println!("[DEBUG] API key is empty!");
        return Err("API key not configured. Please set it in Settings.".to_string());
    }

    println!("[DEBUG] API key present (len={})", api_key.len());

    let client = reqwest::Client::new();
    let url = format!(
        "https://manifest.morrenus.xyz/api/v1/search?q={}",
        urlencoding::encode(&query)
    );

    println!("[DEBUG] Requesting: {}", url);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| {
            println!("[DEBUG] Request failed: {}", e);
            format!("Request failed: {}", e)
        })?;

    println!("[DEBUG] Response status: {}", response.status());

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        println!("[DEBUG] Error body: {}", body);

        // Specific messages for auth errors
        if status.as_u16() == 401 {
            return Err(
                "API key expired or invalid. Please get a new key from Morrenus.".to_string(),
            );
        }
        if status.as_u16() == 403 {
            return Err("API key forbidden. Please check your key permissions.".to_string());
        }

        return Err(format!("API error ({}): {}", status, body));
    }

    #[derive(Deserialize)]
    struct ApiResponse {
        results: Vec<ApiResult>,
    }

    #[derive(Deserialize)]
    struct ApiResult {
        game_id: String,
        game_name: String,
        manifest_available: bool,
    }

    let api_response: ApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let results: Vec<SearchResult> = api_response
        .results
        .into_iter()
        .map(|r| SearchResult {
            game_id: r.game_id,
            game_name: r.game_name,
            manifest_available: r.manifest_available,
            manifest_size: None,
            header_image: None,
        })
        .collect();

    Ok(results)
}

// ============================================================================
// STEAMGRIDDB COMMANDS
// ============================================================================

/// Fetch game artwork from SteamGridDB API
#[tauri::command]
pub async fn fetch_steamgriddb_artwork(
    api_key: String,
    steam_app_id: String,
) -> Result<Option<String>, String> {
    if api_key.is_empty() {
        return Ok(None); // No API key, skip artwork
    }

    let client = reqwest::Client::new();

    // First, search for the game by Steam App ID
    let search_url = format!(
        "https://www.steamgriddb.com/api/v2/games/steam/{}",
        steam_app_id
    );

    let search_response = client
        .get(&search_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("SteamGridDB request failed: {}", e))?;

    if !search_response.status().is_success() {
        eprintln!("[SteamGridDB] Game not found for steam_id {}", steam_app_id);
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct GameResponse {
        data: Option<GameData>,
    }

    #[derive(Deserialize)]
    struct GameData {
        id: u64,
    }

    let game_response: GameResponse = search_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse game response: {}", e))?;

    let game_id = match game_response.data {
        Some(data) => data.id,
        None => return Ok(None),
    };

    // Now fetch grids for this game
    let grids_url = format!(
        "https://www.steamgriddb.com/api/v2/grids/game/{}?dimensions=460x215,920x430&types=static",
        game_id
    );

    let grids_response = client
        .get(&grids_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("SteamGridDB grids request failed: {}", e))?;

    if !grids_response.status().is_success() {
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct GridsResponse {
        data: Vec<GridData>,
    }

    #[derive(Deserialize)]
    struct GridData {
        url: String,
    }

    let grids: GridsResponse = grids_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse grids response: {}", e))?;

    // Return first available grid URL
    if let Some(grid) = grids.data.first() {
        eprintln!(
            "[SteamGridDB] Found artwork for {}: {}",
            steam_app_id, &grid.url
        );
        Ok(Some(grid.url.clone()))
    } else {
        Ok(None)
    }
}

// ============================================================================
// DOWNLOAD COMMANDS
// ============================================================================

/// Download bundle from Morrenus API to temp directory
#[tauri::command]
pub async fn download_bundle(
    app_id: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let api_key = get_api_key_internal(&app_handle)?;

    if api_key.is_empty() {
        return Err("API key not configured".to_string());
    }

    let client = reqwest::Client::new();
    let url = format!("https://manifest.morrenus.xyz/api/v1/manifest/{}", app_id);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, body));
    }

    // Save to temp file
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("tontondeck_{}.zip", app_id));

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

// ============================================================================
// DEPOTDOWNLOADERMOD COMMANDS
// ============================================================================

/// Game manifest data extracted from ZIP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameManifestData {
    pub app_id: String,
    pub game_name: String,
    pub install_dir: String,
    pub depots: Vec<DepotInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotInfo {
    pub depot_id: String,
    pub name: String,
    pub manifest_id: String,
    pub manifest_path: String,
    pub key: String,
    pub size: u64,
}

use std::collections::HashMap;
use std::sync::OnceLock;

static DEPOT_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

fn get_depot_map() -> &'static HashMap<String, String> {
    DEPOT_NAMES.get_or_init(|| {
        let content = include_str!("depots.ini");
        let mut map = HashMap::new();
        for line in content.lines() {
            if let Some((id, name)) = line.split_once('=') {
                map.insert(id.trim().to_string(), name.trim().to_string());
            }
        }
        eprintln!(
            "[commands.rs] Parsed {} depot names from embedded depots.ini",
            map.len()
        );
        map
    })
}

fn get_known_depot_name(depot_id: &str) -> Option<String> {
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
        "tontondeck_extract_{}",
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
                    });
                }
            }
        }

        GameManifestData {
            app_id,
            game_name,
            install_dir,
            depots,
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
        // Format 1 (Main App): addappid(2273430) -- Game Name
        // Format 2 (Depot): addappid(2273431, 1, "key") -- Depot Name

        // Parse setManifestid() for sizes
        // Format: setManifestid(depot_id, "manifest_id", size)
        // Regex for Main App ID (can be single arg OR three args with key/comment)
        // Format 1: addappid(2273430) -- Game Name
        // Format 2: addappid(2273430, 1, "key") -- Game Name (Main App)
        let app_decl_re = regex::Regex::new(
            r#"(?m)^addappid\s*\(\s*(\d+)\s*(?:,\s*\d+\s*,\s*"[^"]*")?\s*\)\s*--\s*(.*)$"#,
        )
        .map_err(|e| format!("Regex error: {}", e))?;

        // Regex for Depots (three arguments)
        let depot_decl_re = regex::Regex::new(
            r#"(?m)^addappid\s*\(\s*(\d+)\s*,\s*\d+\s*,\s*"([^"]*)"\s*\)\s*--\s*(.*)$"#,
        )
        .map_err(|e| format!("Regex error: {}", e))?;

        // Parse setManifestid() for sizes
        // Format: setManifestid(depot_id, "manifest_id", size)
        let manifest_re =
            regex::Regex::new(r#"(?m)setManifestid\s*\(\s*(\d+)\s*,\s*"([^"]*)"\s*,\s*(\d+)\s*\)"#)
                .map_err(|e| format!("Manifest regex error: {}", e))?;

        let mut app_id = String::new();
        let mut game_name = String::new();
        let mut depots = Vec::new();

        // 1. Try to find Main App ID declaration first
        // We look for the FIRST text occurrence of addappid
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

            // If we didn't find Main App ID yet (legacy format?), try to infer from first depot
            // If we didn't find Main App ID yet (legacy format?), try to infer from first depot
            if app_id.is_empty() {
                app_id = depot_id.to_string();
                game_name = comment.to_string();
                eprintln!(
                    "[extract_manifest_zip] inferred AppID from first depot: {}",
                    app_id
                );
            } else {
                // It's a depot
                // Ensure this is not the Main App ID (which sometimes appears in addappid if 3-args used)
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
                    eprintln!(
                        "[extract_manifest_zip] Adding depot {} with name '{}'",
                        depot_id, final_name
                    );

                    depots.push(DepotInfo {
                        depot_id: depot_id.to_string(),
                        name: final_name,
                        manifest_id: String::new(), // Will be filled from setManifestid
                        manifest_path: String::new(),
                        key: key.to_string(),
                        size: 0, // Will be filled from setManifestid
                    });
                }
            }
        }

        // Parse setManifestid() entries to get manifest IDs and sizes
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

            // Update the corresponding depot
            for depot in &mut depots {
                if depot.depot_id == depot_id {
                    depot.manifest_id = manifest_id.to_string();
                    depot.size = size;
                }
            }
        }

        // 3. Offline Enrichment (Heuristics)
        // A. Apply Known Redist Names
        for depot in &mut depots {
            if let Some(known_name) = get_known_depot_name(&depot.depot_id) {
                eprintln!(
                    "[extract_manifest_zip] Recognized generic depot {} as '{}'",
                    depot.depot_id, known_name
                );
                depot.name = known_name;
            }
        }

        // B. "Largest Depot" Rule
        // If the largest depot has a generic name, rename it to "{GameName} Content"
        // But only if we have a valid Game Name
        if !game_name.is_empty() {
            let mut max_size = 0;
            let mut max_idx = None;

            for (i, depot) in depots.iter().enumerate() {
                // Ignore if already classified as Redist/OS-specific or explicitly named in LUA
                // If distinct from "Depot {ID}" check.
                // We check if it starts with "Depot " -> implies generic.
                if depot.size > max_size {
                    max_size = depot.size;
                    max_idx = Some(i);
                }
            }

            if let Some(idx) = max_idx {
                let depot = &mut depots[idx];
                // Check if name is generic (starts with "Depot ")
                if depot.name.starts_with("Depot ") {
                    let new_name = format!("{} Content", game_name);
                    eprintln!("[extract_manifest_zip] Heuristic: Renaming largest depot ({}) from '{}' to '{}'", depot.depot_id, depot.name, new_name);
                    depot.name = new_name;
                }
            }
        }

        // Find manifest files to get manifest IDs
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
            // Format: depot_id_manifest_id.manifest
            let parts: Vec<&str> = fname.trim_end_matches(".manifest").split('_').collect();
            if parts.len() >= 2 {
                let depot_id = parts[0];
                let manifest_id = parts[1];

                // Update depot with manifest info
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
    let keys_file = temp_dir.join("tontondeck_depot_keys.txt");
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
    let zip_path = temp_dir.join(format!("tontondeck_{}.zip", app_id));
    if zip_path.exists() {
        std::fs::remove_file(&zip_path).ok();
    }

    // Clean up extracted manifests directory
    // Look for any tontondeck_extract_* directories
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("tontondeck_extract_") {
                        std::fs::remove_dir_all(&path).ok();
                    }
                }
            }
        }
    }

    // Clean up depot keys file
    let keys_file = temp_dir.join("tontondeck_depot_keys.txt");
    if keys_file.exists() {
        std::fs::remove_file(&keys_file).ok();
    }

    Ok(())
}

// ============================================================================
// STEAMLESS DRM REMOVAL COMMANDS
// ============================================================================

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

// ============================================================================
// DEPLOY COMMANDS
// ============================================================================

/// Upload file to Steam Deck via SFTP
use crate::install_manager::InstallManager;
use tauri::State;

/// Start pipelined installation (Download -> Process -> Upload)
#[tauri::command]
pub async fn start_pipelined_install(
    install_manager: State<'_, InstallManager>,
    app_id: String,
    game_name: String,
    depot_ids: Vec<String>,
    manifest_ids: Vec<String>,
    manifest_files: Vec<String>,
    depot_keys: Vec<(String, String)>, // (depot_id, key) pairs
    depot_downloader_path: String,
    steamless_path: String,
    ssh_config: SshConfig,
    target_directory: String,
) -> Result<(), String> {
    // Validate input lengths match
    if depot_ids.len() != manifest_ids.len() || depot_ids.len() != manifest_files.len() {
        return Err("Input arrays lengths mismatch".to_string());
    }

    // Generate depot keys file in temp dir (like Accela)
    let temp_dir = std::env::temp_dir();
    let keys_file = temp_dir.join("tontondeck_depot_keys.txt");
    let mut keys_content = String::new();
    for (depot_id, key) in &depot_keys {
        keys_content.push_str(&format!("{};{}\n", depot_id, key));
    }
    std::fs::write(&keys_file, &keys_content)
        .map_err(|e| format!("Failed to write depot keys file: {}", e))?;

    eprintln!(
        "[start_pipelined_install] Generated keys file at {:?} with {} keys",
        keys_file,
        depot_keys.len()
    );

    let mut depots = Vec::new();
    for i in 0..depot_ids.len() {
        depots.push(crate::install_manager::DepotDownloadArg {
            depot_id: depot_ids[i].clone(),
            manifest_id: manifest_ids[i].clone(),
            manifest_file: manifest_files[i].clone(),
        });
    }

    install_manager.start_pipeline(
        app_id,
        game_name,
        depots,
        keys_file,
        depot_downloader_path,
        steamless_path,
        ssh_config,
        target_directory,
    )
}

/// Cancel ongoing installation
#[tauri::command]
pub async fn cancel_installation(install_manager: State<'_, InstallManager>) -> Result<(), String> {
    install_manager.cancel();
    Ok(())
}

/// Pause ongoing installation
#[tauri::command]
pub async fn pause_installation(install_manager: State<'_, InstallManager>) -> Result<(), String> {
    install_manager.pause();
    Ok(())
}

/// Resume paused installation
#[tauri::command]
pub async fn resume_installation(install_manager: State<'_, InstallManager>) -> Result<(), String> {
    install_manager.resume();
    Ok(())
}

#[tauri::command]
pub async fn upload_to_deck(
    config: SshConfig,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    let addr = format!("{}:{}", config.ip, config.port);

    // Connect
    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(10),
    )
    .map_err(|e| format!("TCP connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Authenticate
    if !config.private_key_path.is_empty() {
        sess.userauth_pubkey_file(
            &config.username,
            None,
            Path::new(&config.private_key_path),
            None,
        )
        .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else {
        sess.userauth_password(&config.username, &config.password)
            .map_err(|e| format!("SSH password auth failed: {}", e))?;
    }

    // Open SFTP session
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Failed to open SFTP: {}", e))?;

    // Read local file
    let local_data =
        std::fs::read(&local_path).map_err(|e| format!("Failed to read local file: {}", e))?;

    // Create remote directories if needed
    let remote_path_obj = Path::new(&remote_path);
    if let Some(parent) = remote_path_obj.parent() {
        let mkdir_cmd = format!("mkdir -p {}", parent.display());
        let mut channel = sess
            .channel_session()
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        channel.exec(&mkdir_cmd).ok();
        channel.wait_close().ok();
    }

    // Write to remote
    let mut remote_file = sftp
        .create(Path::new(&remote_path))
        .map_err(|e| format!("Failed to create remote file: {}", e))?;

    // Write in chunks
    const CHUNK_SIZE: usize = 65536;
    for chunk in local_data.chunks(CHUNK_SIZE) {
        remote_file
            .write_all(chunk)
            .map_err(|e| format!("Failed to write chunk: {}", e))?;
    }

    Ok(())
}

/// Extract ZIP file on remote Steam Deck
#[tauri::command]
pub async fn extract_remote(
    config: SshConfig,
    zip_path: String,
    dest_dir: String,
) -> Result<(), String> {
    let addr = format!("{}:{}", config.ip, config.port);

    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(10),
    )
    .map_err(|e| format!("TCP connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Authenticate
    if !config.private_key_path.is_empty() {
        sess.userauth_pubkey_file(
            &config.username,
            None,
            Path::new(&config.private_key_path),
            None,
        )
        .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else {
        sess.userauth_password(&config.username, &config.password)
            .map_err(|e| format!("SSH password auth failed: {}", e))?;
    }

    // Create destination directory and extract
    let cmd = format!(
        "mkdir -p {} && unzip -o {} -d {} || bsdtar -xf {} -C {}",
        dest_dir, zip_path, dest_dir, zip_path, dest_dir
    );

    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec(&cmd)
        .map_err(|e| format!("Failed to execute extract: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output).ok();

    let exit_status = channel
        .exit_status()
        .map_err(|e| format!("Failed to get exit status: {}", e))?;

    channel.wait_close().ok();

    if exit_status != 0 {
        return Err(format!(
            "Extract failed with status {}: {}",
            exit_status, output
        ));
    }

    Ok(())
}

/// Update SLSsteam config.yaml with new AppID
#[tauri::command]
pub async fn update_slssteam_config(config: SshConfig, app_id: String) -> Result<(), String> {
    let addr = format!("{}:{}", config.ip, config.port);

    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(10),
    )
    .map_err(|e| format!("TCP connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Authenticate
    if !config.private_key_path.is_empty() {
        sess.userauth_pubkey_file(
            &config.username,
            None,
            Path::new(&config.private_key_path),
            None,
        )
        .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else {
        sess.userauth_password(&config.username, &config.password)
            .map_err(|e| format!("SSH password auth failed: {}", e))?;
    }

    let sftp = sess
        .sftp()
        .map_err(|e| format!("Failed to open SFTP: {}", e))?;

    let config_path = "/home/deck/.config/SLSsteam/config.yaml";

    // Read existing config
    let mut config_content = String::new();
    if let Ok(mut file) = sftp.open(Path::new(config_path)) {
        file.read_to_string(&mut config_content).ok();
    }

    // Create backup with timestamp
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_path = format!("{}.bak-{}", config_path, timestamp);

    // Backup command
    let backup_cmd = format!("cp {} {} 2>/dev/null || true", config_path, backup_path);
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel.exec(&backup_cmd).ok();
    channel.wait_close().ok();

    // Parse and update YAML
    let new_content = add_app_to_config(&config_content, &app_id)?;

    // Ensure directory exists
    let mkdir_cmd = "mkdir -p /home/deck/.config/SLSsteam";
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel.exec(mkdir_cmd).ok();
    channel.wait_close().ok();

    // Write updated config
    let mut remote_file = sftp
        .create(Path::new(config_path))
        .map_err(|e| format!("Failed to create config file: {}", e))?;

    remote_file
        .write_all(new_content.as_bytes())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

/// Helper function to add AppID to config YAML
/// Uses text-based insertion to preserve comments
fn add_app_to_config(content: &str, app_id: &str) -> Result<String, String> {
    // Check if app_id already exists
    if content.contains(&format!("- {}", app_id)) {
        return Ok(content.to_string());
    }
    
    // Find AdditionalApps section
    if let Some(idx) = content.find("AdditionalApps:") {
        // Find the end of AdditionalApps line
        let after_key = &content[idx..];
        if let Some(newline_idx) = after_key.find('\n') {
            let insert_pos = idx + newline_idx + 1;
            
            // Insert the new entry (no indentation needed for YAML list items)
            let new_entry = format!("- {}\n", app_id);
            
            let mut result = String::with_capacity(content.len() + new_entry.len());
            result.push_str(&content[..insert_pos]);
            result.push_str(&new_entry);
            result.push_str(&content[insert_pos..]);
            return Ok(result);
        }
    }
    
    // No AdditionalApps section - append it
    let mut result = content.to_string();
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push_str("\nAdditionalApps:\n");
    result.push_str(&format!("- {}\n", app_id));
    Ok(result)
}

// ============================================================================
// LIBRARY MANAGEMENT COMMANDS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledGame {
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_image: Option<String>,
}

/// List installed games on Steam Deck by scanning the common folder
#[tauri::command]
pub async fn list_installed_games(config: SshConfig) -> Result<Vec<InstalledGame>, String> {
    use std::net::{IpAddr, SocketAddr};

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let mut games = Vec::new();

    // Get all Steam library paths
    let libraries = get_steam_library_paths(&sess)?;

    for library in libraries {
        let steamapps_path = format!("{}/steamapps", library);
        let common_path = format!("{}/common", steamapps_path);

        // First, build a map of installdir -> appid by parsing all appmanifest_*.acf files
        let acf_cmd = format!(
            "for f in {}/appmanifest_*.acf; do \
                if [ -f \"$f\" ]; then \
                    appid=$(grep -m1 '\"appid\"' \"$f\" | grep -oE '[0-9]+'); \
                    installdir=$(grep -m1 '\"installdir\"' \"$f\" | sed 's/.*\"installdir\"[[:space:]]*\"\\([^\"]*\\)\".*/\\1/'); \
                    echo \"$installdir|$appid\"; \
                fi; \
            done 2>/dev/null",
            steamapps_path
        );

        let acf_output = ssh_exec(&sess, &acf_cmd)?;

        // Parse ACF output into a map: installdir -> appid
        let mut installdir_to_appid: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for line in acf_output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let installdir = parts[0].trim().to_string();
                let appid = parts[1].trim().to_string();
                if !installdir.is_empty() && !appid.is_empty() {
                    installdir_to_appid.insert(installdir, appid);
                }
            }
        }
        eprintln!(
            "[list_installed_games] Found {} ACF entries in {}",
            installdir_to_appid.len(),
            steamapps_path
        );

        // List all folders in steamapps/common
        let list_cmd = format!("ls -1 '{}' 2>/dev/null || echo ''", common_path);
        let output = ssh_exec(&sess, &list_cmd)?;

        for line in output.lines() {
            let name = line.trim();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }

            let game_path = format!("{}/{}", common_path, name);

            // Check if .DepotDownloader folder exists (ACCELA/TontonDeck marker)
            let marker_cmd = format!(
                "test -d '{}/.DepotDownloader' && echo 'YES' || echo 'NO'",
                game_path
            );
            let marker_out = ssh_exec(&sess, &marker_cmd)?;

            if marker_out.trim() != "YES" {
                continue; // Not installed by DepotDownloader, skip
            }

            // Get directory size
            let size_cmd = format!("du -sb '{}' 2>/dev/null | cut -f1 || echo '0'", game_path);
            let size_out = ssh_exec(&sess, &size_cmd)?;
            let size_bytes: u64 = size_out.trim().parse().unwrap_or(0);

            // Look up AppID from our ACF map using folder name as installdir
            let app_id = installdir_to_appid
                .get(name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            eprintln!(
                "[list_installed_games] Game: {}, AppID from ACF: {}",
                name, app_id
            );

            games.push(InstalledGame {
                app_id,
                name: name.to_string(),
                path: game_path,
                size_bytes,
                header_image: None,
            });
        }
    }

    Ok(games)
}

/// List installed games locally (for running on Steam Deck/Linux itself)
#[tauri::command]
pub async fn list_installed_games_local() -> Result<Vec<InstalledGame>, String> {
    use std::fs;
    use walkdir::WalkDir;

    eprintln!("[list_installed_games_local] Starting local scan...");

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    eprintln!("[list_installed_games_local] Home: {:?}", home);

    let mut games = Vec::new();

    // Common Steam library paths on Linux/SteamOS
    let library_paths = vec![
        home.join(".local/share/Steam/steamapps"),
        home.join(".steam/steam/steamapps"),
    ];

    for steamapps in library_paths {
        if !steamapps.exists() {
            eprintln!(
                "[list_installed_games_local] Path doesn't exist: {:?}",
                steamapps
            );
            continue;
        }

        eprintln!("[list_installed_games_local] Scanning: {:?}", steamapps);

        // Build map of installdir -> appid from appmanifest_*.acf files
        let mut installdir_to_appid: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        if let Ok(entries) = fs::read_dir(&steamapps) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("appmanifest_") && name.ends_with(".acf") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let mut appid = String::new();
                            let mut installdir = String::new();

                            for line in content.lines() {
                                if line.contains("\"appid\"") {
                                    if let Some(id) = line.split('"').nth(3) {
                                        appid = id.to_string();
                                    }
                                }
                                if line.contains("\"installdir\"") {
                                    if let Some(dir) = line.split('"').nth(3) {
                                        installdir = dir.to_string();
                                    }
                                }
                            }

                            if !appid.is_empty() && !installdir.is_empty() {
                                installdir_to_appid.insert(installdir, appid);
                            }
                        }
                    }
                }
            }
        }

        eprintln!(
            "[list_installed_games_local] Found {} ACF entries",
            installdir_to_appid.len()
        );

        // Scan common folder for games with .DepotDownloader marker
        let common_path = steamapps.join("common");
        if !common_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&common_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                // Check for .DepotDownloader marker (TonTonDeck/ACCELA installed)
                let marker_path = path.join(".DepotDownloader");
                if !marker_path.exists() {
                    continue;
                }

                // Calculate directory size
                let mut size_bytes: u64 = 0;
                for entry in WalkDir::new(&path).into_iter().flatten() {
                    if entry.file_type().is_file() {
                        if let Ok(meta) = entry.metadata() {
                            size_bytes += meta.len();
                        }
                    }
                }

                // Look up AppID from ACF map
                let app_id = installdir_to_appid
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                eprintln!(
                    "[list_installed_games_local] Found: {} (AppID: {})",
                    name, app_id
                );

                games.push(InstalledGame {
                    app_id,
                    name,
                    path: path.to_string_lossy().to_string(),
                    size_bytes,
                    header_image: None,
                });
            }
        }
    }

    eprintln!(
        "[list_installed_games_local] Total games found: {}",
        games.len()
    );
    Ok(games)
}

/// Installed depot info for update detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledDepot {
    pub depot_id: String,
    pub manifest_id: String,
}

/// Check if a game is installed and return its installed manifest IDs
#[tauri::command]
pub async fn check_game_installed(
    config: SshConfig,
    app_id: String,
) -> Result<Vec<InstalledDepot>, String> {
    use std::net::{IpAddr, SocketAddr};

    if config.is_local {
        // LOCAL CHECK
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let mut paths = Vec::new();

        let primary_path = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Steam")
        } else {
            home.join(".steam/steam")
        };

        if primary_path.exists() {
            paths.push(primary_path);
        }

        let mut installed_depots: Vec<InstalledDepot> = Vec::new();

        for lib in paths {
            let common_path = lib.join("steamapps/common");
            if let Ok(entries) = std::fs::read_dir(common_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let dd_path = path.join(".DepotDownloader");
                        if dd_path.exists() {
                            if let Ok(dd_entries) = std::fs::read_dir(dd_path) {
                                for dd_entry in dd_entries.flatten() {
                                    let name = dd_entry.file_name().to_string_lossy().to_string();
                                    if name.ends_with(".manifest") {
                                        let base = name.trim_end_matches(".manifest");
                                        let parts: Vec<&str> = base.split('_').collect();
                                        if parts.len() >= 2 {
                                            installed_depots.push(InstalledDepot {
                                                depot_id: parts[0].to_string(),
                                                manifest_id: parts[1].to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        return Ok(installed_depots);
    }

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH authentication failed: {}", e))?;

    if !sess.authenticated() {
        return Err("Authentication failed".to_string());
    }

    // Get Steam library paths
    let libraries = get_steam_library_paths(&sess)?;

    let mut installed_depots: Vec<InstalledDepot> = Vec::new();

    for lib in libraries {
        let common_path = format!("{}/steamapps/common", lib);

        // Find game folders and check .DepotDownloader for manifest info
        let find_cmd = format!(
            "find '{}' -maxdepth 2 -type d -name '.DepotDownloader' 2>/dev/null | while read dir; do ls -1 \"$dir\" 2>/dev/null; done",
            common_path
        );
        let output = ssh_exec(&sess, &find_cmd)?;

        for line in output.lines() {
            let name = line.trim();
            // Parse manifest files: depot_id_manifest_id.manifest
            if name.ends_with(".manifest") {
                let base = name.trim_end_matches(".manifest");
                let parts: Vec<&str> = base.split('_').collect();
                if parts.len() >= 2 {
                    installed_depots.push(InstalledDepot {
                        depot_id: parts[0].to_string(),
                        manifest_id: parts[1].to_string(),
                    });
                }
            }
            // Parse .depot files: appid.depot with manifest ID inside
            else if name.ends_with(".depot") {
                // These might contain app_id, not depot info
                // Skip for now
            }
        }
    }

    eprintln!(
        "[check_game_installed] Found {} installed depots for app {}",
        installed_depots.len(),
        app_id
    );

    Ok(installed_depots)
}

/// Helper function to parse Steam library paths from libraryfolders.vdf
fn get_steam_library_paths(sess: &ssh2::Session) -> Result<Vec<String>, String> {
    use std::collections::HashSet;

    let mut libraries_set: HashSet<String> = HashSet::new();

    // Primary Steam path (resolve symlink to get real path)
    let real_path_out = ssh_exec(
        sess,
        "readlink -f ~/.steam/steam 2>/dev/null || echo '/home/deck/.steam/steam'",
    )?;
    let primary_path = real_path_out.trim().to_string();
    libraries_set.insert(primary_path.clone());

    // Parse libraryfolders.vdf for additional libraries
    let vdf_content = ssh_exec(
        sess,
        "cat ~/.steam/steam/steamapps/libraryfolders.vdf 2>/dev/null || echo ''",
    )?;

    // Parse paths from VDF (simple regex-like parsing)
    for line in vdf_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"path\"") {
            // Extract path value: "path"		"/run/media/..."
            if let Some(start) = trimmed.rfind('"') {
                let before_last = &trimmed[..start];
                if let Some(path_start) = before_last.rfind('"') {
                    let path = &before_last[path_start + 1..];
                    if !path.is_empty() {
                        libraries_set.insert(path.to_string());
                    }
                }
            }
        }
    }

    Ok(libraries_set.into_iter().collect())
}

/// Get available Steam libraries on Steam Deck
#[tauri::command]
pub async fn get_steam_libraries(config: SshConfig) -> Result<Vec<String>, String> {
    use std::net::{IpAddr, SocketAddr};

    // LOCAL MODE
    if config.is_local {
        // Find local Steam paths
        // This is heuristic. On Mac it's different than Linux.
        // User asked for "Local Mode" to manage local library.
        // For Mac: ~/Library/Application Support/Steam
        // For Linux: ~/.steam/steam

        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let mut paths = Vec::new();

        let primary_steam_path = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Steam")
        } else {
            home.join(".steam/steam")
        };

        if primary_steam_path.exists() {
            paths.push(primary_steam_path.to_string_lossy().to_string());

            // Check libraryfolders.vdf locally
            let vdf_path = primary_steam_path.join("steamapps/libraryfolders.vdf");
            if let Ok(content) = std::fs::read_to_string(vdf_path) {
                // Simple parse
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("\"path\"") {
                        if let Some(start) = trimmed.rfind('"') {
                            let before_last = &trimmed[..start];
                            if let Some(path_start) = before_last.rfind('"') {
                                let path = &before_last[path_start + 1..];
                                if !path.is_empty() {
                                    paths.push(path.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Dedup
        paths.sort();
        paths.dedup();

        return Ok(paths);
    }

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    get_steam_library_paths(&sess)
}

/// Uninstall a game from Steam Deck
#[tauri::command]
pub async fn uninstall_game(
    config: SshConfig,
    game_path: String,
    app_id: String,
) -> Result<String, String> {
    // LOCAL MODE
    if config.is_local {
        use std::path::PathBuf;

        // 1. Remove game folder
        let game_dir = PathBuf::from(&game_path);
        if game_dir.exists() {
            std::fs::remove_dir_all(&game_dir)
                .map_err(|e| format!("Failed to remove game folder: {}", e))?;
        }

        // 2. Remove ACF file
        // game_path looks like: /home/user/.steam/steam/steamapps/common/GameName
        // ACF is in steamapps/ directory (parent of common/)
        if let Some(common_dir) = game_dir.parent() {
            if let Some(steamapps_dir) = common_dir.parent() {
                let acf_path = steamapps_dir.join(format!("appmanifest_{}.acf", app_id));
                if acf_path.exists() {
                    std::fs::remove_file(&acf_path).ok();
                }
            }
        }

        // 3. Remove AppID from SLSsteam config
        if let Some(home) = dirs::home_dir() {
            let config_path = home.join(".config/SLSsteam/config.yaml");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(new_content) = remove_app_from_config(&content, &app_id) {
                        let _ = std::fs::write(&config_path, new_content);
                    }
                }
            }
        }

        return Ok(format!("Uninstalled game at {} (local mode)", game_path));
    }

    // REMOTE MODE via SSH
    let addr = format!("{}:{}", config.ip, config.port);

    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(10),
    )
    .map_err(|e| format!("TCP connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH password auth failed: {}", e))?;

    // 1. Remove game folder
    let rm_cmd = format!("rm -rf '{}'", game_path);
    ssh_exec(&sess, &rm_cmd)?;

    // 2. Remove ACF file (appmanifest_{app_id}.acf)
    // ACF is in steamapps/ which is parent of common/
    // game_path looks like: /home/deck/.local/share/Steam/steamapps/common/GameName
    let acf_cmd = format!(
        "rm -f \"$(dirname '{}')/../appmanifest_{}.acf\"",
        game_path, app_id
    );
    ssh_exec(&sess, &acf_cmd)?;

    // 3. Remove AppID from SLSsteam config.yaml
    let config_path = "/home/deck/.config/SLSsteam/config.yaml";
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Failed to open SFTP: {}", e))?;

    // Read existing config
    let config_content = match sftp.open(Path::new(config_path)) {
        Ok(mut f) => {
            let mut buf = String::new();
            let _ = f.read_to_string(&mut buf);
            buf
        }
        Err(_) => String::new(),
    };

    // Remove app_id from AdditionalApps
    let new_content = remove_app_from_config(&config_content, &app_id)?;

    // Write back
    let mut remote_f = sftp
        .create(Path::new(config_path))
        .map_err(|e| format!("Failed to create config file: {}", e))?;
    remote_f
        .write_all(new_content.as_bytes())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(format!(
        "Uninstalled game at {} (removed ACF and config)",
        game_path
    ))
}

/// Helper function to remove AppID from config YAML
fn remove_app_from_config(content: &str, app_id: &str) -> Result<String, String> {
    if content.is_empty() {
        return Ok(String::new());
    }

    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    if let Some(mapping) = doc.as_mapping_mut() {
        if let Some(additional_apps) =
            mapping.get_mut(&serde_yaml::Value::String("AdditionalApps".to_string()))
        {
            if let Some(apps_list) = additional_apps.as_sequence_mut() {
                let app_id_num: i64 = app_id.parse().unwrap_or(0);
                apps_list.retain(|v| match v {
                    serde_yaml::Value::Number(n) => n.as_i64() != Some(app_id_num),
                    serde_yaml::Value::String(s) => s != app_id,
                    _ => true,
                });
            }
        }
    }

    serde_yaml::to_string(&doc).map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Check if a game has updates available
#[tauri::command]
pub async fn check_game_update(
    app_id: String,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    // Get API key from store
    let api_key = get_api_key(app_handle.clone()).await?;

    // Fetch latest manifest info from Morrenus API
    let url = format!(
        "https://morrenus.martylek.com/api/bundles/search?query={}&key={}",
        app_id, api_key
    );

    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        return Err("API returned error".to_string());
    }

    // For now, we don't have local manifest tracking
    // This would require storing installed manifest_id locally/remotely
    // Return false (no update) as placeholder
    // TODO: Implement proper manifest comparison
    Ok(false)
}

// ============================================================================
// SLSSTEAM INSTALLATION COMMANDS
// ============================================================================

/// Status of SLSsteam installation components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlssteamStatus {
    pub is_readonly: bool,
    pub slssteam_so_exists: bool,
    pub config_exists: bool,
    pub config_play_not_owned: bool,
    pub config_safe_mode_on: bool,
    pub steam_jupiter_patched: bool,
    pub desktop_entry_patched: bool,
    pub additional_apps_count: usize,
}

/// Verify SLSsteam installation status on Steam Deck
#[tauri::command]
pub async fn verify_slssteam(config: SshConfig) -> Result<SlssteamStatus, String> {
    use std::net::{IpAddr, SocketAddr};

    if config.is_local {
        // LOCAL MODE: Check local filesystem for SLSsteam installation
        eprintln!("[SLSsteam Verify] Mode: LOCAL");

        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        eprintln!("[SLSsteam Verify] Home directory: {:?}", home);

        // Detect OS/distro
        let os_info = if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            if content.contains("SteamOS") {
                "SteamOS (Steam Deck)"
            } else if content.to_lowercase().contains("bazzite") {
                "Bazzite (Immutable)"
            } else if content.contains("Arch") {
                "Arch Linux"
            } else {
                "Linux (other)"
            }
        } else {
            if cfg!(target_os = "macos") {
                "macOS"
            } else if cfg!(target_os = "windows") {
                "Windows"
            } else {
                "Unknown"
            }
        };
        eprintln!("[SLSsteam Verify] Detected OS: {}", os_info);

        // Check SLSsteam.so
        let slssteam_so_path = home.join(".local/share/SLSsteam/SLSsteam.so");
        let slssteam_so_exists = slssteam_so_path.exists();
        eprintln!("[SLSsteam Verify] SLSsteam.so path: {:?}", slssteam_so_path);
        eprintln!(
            "[SLSsteam Verify] SLSsteam.so exists: {}",
            slssteam_so_exists
        );

        // Check config
        let config_path = home.join(".config/SLSsteam/config.yaml");
        let config_exists = config_path.exists();
        eprintln!("[SLSsteam Verify] Config path: {:?}", config_path);
        eprintln!("[SLSsteam Verify] Config exists: {}", config_exists);

        // Parse config for settings
        let (config_play_not_owned, config_safe_mode_on, additional_apps_count) = if config_exists {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                let play_not_owned = content.contains("PlayNotOwnedGames: true") || content.contains("PlayNotOwnedGames: yes");
                let safe_mode_on = content.contains("SafeMode: true") || content.contains("SafeMode: yes");
                let apps_count = content.matches("- ").count();
                eprintln!(
                    "[SLSsteam Verify] Config PlayNotOwnedGames: {}",
                    play_not_owned
                );
                eprintln!("[SLSsteam Verify] Config SafeMode: {}", safe_mode_on);
                eprintln!(
                    "[SLSsteam Verify] Config AdditionalApps count: {}",
                    apps_count
                );
                (play_not_owned, safe_mode_on, apps_count)
            } else {
                eprintln!("[SLSsteam Verify] Config read error");
                (false, false, 0)
            }
        } else {
            eprintln!("[SLSsteam Verify] Config not found");
            (false, false, 0)
        };

        // Check if steam.desktop is patched
        let desktop_path = home.join(".local/share/applications/steam.desktop");
        let desktop_entry_patched = if desktop_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&desktop_path) {
                let patched = content.contains("LD_AUDIT");
                eprintln!(
                    "[SLSsteam Verify] steam.desktop exists: true, patched: {}",
                    patched
                );
                patched
            } else {
                eprintln!("[SLSsteam Verify] steam.desktop exists but unreadable");
                false
            }
        } else {
            eprintln!(
                "[SLSsteam Verify] steam.desktop not found at {:?}",
                desktop_path
            );
            false
        };

        // Check if steam-jupiter is patched (SteamOS only)
        let jupiter_path = Path::new("/usr/bin/steam-jupiter");
        let steam_jupiter_patched = if jupiter_path.exists() {
            eprintln!("[SLSsteam Verify] steam-jupiter found at /usr/bin/steam-jupiter");
            if let Ok(content) = std::fs::read_to_string(jupiter_path) {
                let patched = content.contains("LD_AUDIT");
                eprintln!("[SLSsteam Verify] steam-jupiter patched: {}", patched);
                patched
            } else {
                eprintln!("[SLSsteam Verify] steam-jupiter unreadable (may need sudo)");
                false
            }
        } else {
            eprintln!("[SLSsteam Verify] steam-jupiter not found (not SteamOS Gaming Mode)");
            false
        };

        // Check SteamOS readonly status
        let readonly_cmd = Path::new("/usr/bin/steamos-readonly");
        let is_readonly = if readonly_cmd.exists() {
            eprintln!("[SLSsteam Verify] steamos-readonly command found");
            if let Ok(output) = std::process::Command::new("steamos-readonly")
                .arg("status")
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!(
                    "[SLSsteam Verify] steamos-readonly status stdout: {}",
                    stdout.trim()
                );
                if !stderr.is_empty() {
                    eprintln!(
                        "[SLSsteam Verify] steamos-readonly status stderr: {}",
                        stderr.trim()
                    );
                }
                stdout.contains("enabled")
            } else {
                eprintln!("[SLSsteam Verify] steamos-readonly command failed");
                false
            }
        } else {
            eprintln!("[SLSsteam Verify] steamos-readonly not found (not SteamOS)");
            false
        };
        eprintln!("[SLSsteam Verify] Readonly mode: {}", is_readonly);

        eprintln!("[SLSsteam Verify] === SUMMARY ===");
        eprintln!("[SLSsteam Verify] OS: {}, Readonly: {}, SLSsteam.so: {}, Config: {}, Desktop patched: {}, Jupiter patched: {}",
            os_info, is_readonly, slssteam_so_exists, config_exists, desktop_entry_patched, steam_jupiter_patched);

        return Ok(SlssteamStatus {
            is_readonly,
            slssteam_so_exists,
            config_exists,
            config_play_not_owned,
            config_safe_mode_on,
            steam_jupiter_patched,
            desktop_entry_patched,
            additional_apps_count,
        });
    }

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    eprintln!("[SLSsteam Verify] Mode: REMOTE");
    eprintln!(
        "[SLSsteam Verify] Target: {}@{}:{}",
        config.username, config.ip, config.port
    );

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    eprintln!("[SLSsteam Verify] Connecting via SSH...");
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;
    eprintln!("[SLSsteam Verify] SSH connected successfully");

    // Check OS/distro via SSH
    let os_out = ssh_exec(
        &sess,
        "cat /etc/os-release 2>/dev/null | head -5 || echo 'Unknown'",
    )?;
    eprintln!("[SLSsteam Verify] Remote OS info:\n{}", os_out.trim());

    // Check readonly status
    eprintln!("[SLSsteam Verify] Checking readonly status...");
    let readonly_out = ssh_exec(
        &sess,
        "steamos-readonly status 2>/dev/null || echo 'not-steamos'",
    )?;
    eprintln!("[SLSsteam Verify] Readonly output: {}", readonly_out.trim());
    let is_readonly = readonly_out.to_lowercase().contains("enabled");
    eprintln!("[SLSsteam Verify] Readonly mode: {}", is_readonly);

    // Check if SLSsteam.so exists
    eprintln!("[SLSsteam Verify] Checking SLSsteam.so...");
    let so_out = ssh_exec(
        &sess,
        "test -f ~/.local/share/SLSsteam/SLSsteam.so && echo 'EXISTS' || echo 'MISSING'",
    )?;
    let slssteam_so_exists = so_out.contains("EXISTS");
    eprintln!(
        "[SLSsteam Verify] SLSsteam.so exists: {}",
        slssteam_so_exists
    );

    // Check config.yaml
    eprintln!("[SLSsteam Verify] Checking config.yaml...");
    let config_out = ssh_exec(
        &sess,
        "cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''",
    )?;
    let config_exists = !config_out.is_empty() && !config_out.trim().is_empty();
    eprintln!("[SLSsteam Verify] Config exists: {}", config_exists);

    let config_play_not_owned = config_out
        .to_lowercase()
        .contains("playnotownedgames: true")
        || config_out.to_lowercase().contains("playnotownedgames:true")
        || config_out.to_lowercase().contains("playnotownedgames: yes")
        || config_out.to_lowercase().contains("playnotownedgames:yes");
    let config_safe_mode_on = config_out.to_lowercase().contains("safemode: true")
        || config_out.to_lowercase().contains("safemode:true")
        || config_out.to_lowercase().contains("safemode: yes")
        || config_out.to_lowercase().contains("safemode:yes");
    eprintln!(
        "[SLSsteam Verify] Config PlayNotOwnedGames: {}",
        config_play_not_owned
    );
    eprintln!("[SLSsteam Verify] Config SafeMode: {}", config_safe_mode_on);

    // Count additional apps
    let additional_apps_count = config_out
        .lines()
        .filter(|l| l.trim().starts_with("- ") && l.trim().len() > 2)
        .count();
    eprintln!(
        "[SLSsteam Verify] Config AdditionalApps count: {}",
        additional_apps_count
    );

    // Check steam-jupiter for LD_AUDIT
    eprintln!("[SLSsteam Verify] Checking steam-jupiter...");
    let jupiter_exists_out = ssh_exec(
        &sess,
        "test -f /usr/bin/steam-jupiter && echo 'EXISTS' || echo 'MISSING'",
    )?;
    eprintln!(
        "[SLSsteam Verify] steam-jupiter exists: {}",
        jupiter_exists_out.contains("EXISTS")
    );

    let jupiter_out = ssh_exec(
        &sess,
        "grep -c 'LD_AUDIT' /usr/bin/steam-jupiter 2>/dev/null || echo '0'",
    )?;
    let steam_jupiter_patched = jupiter_out.trim().parse::<i32>().unwrap_or(0) > 0;
    eprintln!(
        "[SLSsteam Verify] steam-jupiter patched: {}",
        steam_jupiter_patched
    );

    // Check desktop entry
    eprintln!("[SLSsteam Verify] Checking steam.desktop...");
    let desktop_exists_out = ssh_exec(
        &sess,
        "test -f ~/.local/share/applications/steam.desktop && echo 'EXISTS' || echo 'MISSING'",
    )?;
    eprintln!(
        "[SLSsteam Verify] steam.desktop exists: {}",
        desktop_exists_out.contains("EXISTS")
    );

    let desktop_out = ssh_exec(
        &sess,
        "grep -c 'LD_AUDIT' ~/.local/share/applications/steam.desktop 2>/dev/null || echo '0'",
    )?;
    let desktop_entry_patched = desktop_out.trim().parse::<i32>().unwrap_or(0) > 0;
    eprintln!(
        "[SLSsteam Verify] steam.desktop patched: {}",
        desktop_entry_patched
    );

    eprintln!("[SLSsteam Verify] === SUMMARY ===");
    eprintln!("[SLSsteam Verify] Readonly: {}, SLSsteam.so: {}, Config: {}, Desktop patched: {}, Jupiter patched: {}",
        is_readonly, slssteam_so_exists, config_exists, desktop_entry_patched, steam_jupiter_patched);

    Ok(SlssteamStatus {
        is_readonly,
        slssteam_so_exists,
        config_exists,
        config_play_not_owned,
        config_safe_mode_on,
        steam_jupiter_patched,
        desktop_entry_patched,
        additional_apps_count,
    })
}

/// Status of SLSsteam installation components (local version - simpler)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlssteamLocalStatus {
    pub slssteam_so_exists: bool,
    pub config_exists: bool,
    pub config_play_not_owned: bool,
    pub additional_apps_count: usize,
}

/// Verify SLSsteam installation status on local machine (for running on Steam Deck itself)
#[tauri::command]
pub async fn verify_slssteam_local() -> Result<SlssteamLocalStatus, String> {
    eprintln!("[SLSsteam Local Verify] Starting local verification...");

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    eprintln!("[SLSsteam Local Verify] Home directory: {:?}", home);

    // Check SLSsteam.so
    let slssteam_so_path = home.join(".local/share/SLSsteam/SLSsteam.so");
    let slssteam_so_exists = slssteam_so_path.exists();
    eprintln!(
        "[SLSsteam Local Verify] SLSsteam.so path: {:?}, exists: {}",
        slssteam_so_path, slssteam_so_exists
    );

    // Check config.yaml
    let config_path = home.join(".config/SLSsteam/config.yaml");
    let config_exists = config_path.exists();
    eprintln!(
        "[SLSsteam Local Verify] Config path: {:?}, exists: {}",
        config_path, config_exists
    );

    // Parse config for settings
    let (config_play_not_owned, additional_apps_count) = if config_exists {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let play_not_owned = content.to_lowercase().contains("playnotownedgames: true")
                || content.to_lowercase().contains("playnotownedgames:true")
                || content.to_lowercase().contains("playnotownedgames: yes")
                || content.to_lowercase().contains("playnotownedgames:yes");
            let apps_count = content
                .lines()
                .filter(|l| l.trim().starts_with("- ") && l.trim().len() > 2)
                .count();
            eprintln!(
                "[SLSsteam Local Verify] PlayNotOwnedGames: {}, Apps: {}",
                play_not_owned, apps_count
            );
            (play_not_owned, apps_count)
        } else {
            eprintln!("[SLSsteam Local Verify] Config read error");
            (false, 0)
        }
    } else {
        (false, 0)
    };

    eprintln!(
        "[SLSsteam Local Verify] === RESULT: so={}, config={}, play_not_owned={}, apps={} ===",
        slssteam_so_exists, config_exists, config_play_not_owned, additional_apps_count
    );

    Ok(SlssteamLocalStatus {
        slssteam_so_exists,
        config_exists,
        config_play_not_owned,
        additional_apps_count,
    })
}

/// Detect if running on Steam Deck / SteamOS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamDeckDetection {
    pub is_steam_deck: bool,
    pub is_steamos: bool,
    pub os_name: String,
}

#[tauri::command]
pub async fn detect_steam_deck() -> Result<SteamDeckDetection, String> {
    eprintln!("[detect_steam_deck] Checking platform...");

    // Check /etc/os-release for SteamOS
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();

    let is_steamos = os_release.contains("SteamOS");
    let is_bazzite = os_release.to_lowercase().contains("bazzite");

    // Check for Steam Deck specific indicators
    let has_jupiter = std::path::Path::new("/usr/bin/steam-jupiter").exists();
    let has_deck_user = std::path::Path::new("/home/deck").exists();

    // Determine OS name
    let os_name = if is_steamos {
        "SteamOS".to_string()
    } else if is_bazzite {
        "Bazzite".to_string()
    } else if cfg!(target_os = "linux") {
        // Try to get distro name
        os_release
            .lines()
            .find(|l| l.starts_with("PRETTY_NAME="))
            .map(|l| {
                l.trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string()
            })
            .unwrap_or_else(|| "Linux".to_string())
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else {
        "Unknown".to_string()
    };

    let is_steam_deck = is_steamos || is_bazzite || (has_jupiter && has_deck_user);

    eprintln!(
        "[detect_steam_deck] Result: is_steam_deck={}, is_steamos={}, os={}",
        is_steam_deck, is_steamos, os_name
    );

    Ok(SteamDeckDetection {
        is_steam_deck,
        is_steamos,
        os_name,
    })
}

/// Check if sshpass is available (needed for rsync password authentication)
#[tauri::command]
pub async fn check_sshpass_available() -> Result<bool, String> {
    use std::process::Command;

    let result = Command::new("which").arg("sshpass").output();

    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Check if SteamOS is in read-only mode
#[tauri::command]
pub async fn check_readonly_status(config: SshConfig) -> Result<bool, String> {
    use std::net::{IpAddr, SocketAddr};

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    // Check readonly status
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec("steamos-readonly status 2>/dev/null || echo 'unknown'")
        .map_err(|e| format!("Failed to exec command: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output).ok();
    channel.wait_close().ok();

    // Returns true if readonly is enabled (bad for installation)
    let is_readonly = output.to_lowercase().contains("enabled");
    Ok(is_readonly)
}

/// Install SLSsteam on Steam Deck via SSH
#[tauri::command]
pub async fn install_slssteam(
    config: SshConfig,
    slssteam_path: String,
    root_password: String,
) -> Result<String, String> {
    use std::fs;
    use std::net::{IpAddr, SocketAddr};

    // Validate inputs
    if config.is_local {
        // ========================================
        // LOCAL MODE INSTALLATION
        // ========================================
        use std::process::Command;

        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let mut log = String::new();

        // Step 0: Detect SteamOS/immutable distro and try to disable readonly
        log.push_str("Checking for SteamOS/immutable distro...\n");

        if Path::new("/usr/bin/steamos-readonly").exists() {
            log.push_str("Detected SteamOS. Attempting to disable readonly...\n");
            // Note: This requires sudo which user must have configured with NOPASSWD or will be prompted
            let _ = Command::new("sudo")
                .args(["steamos-readonly", "disable"])
                .status(); // Ignore errors - may not have sudo access
            log.push_str("Readonly disable attempted.\n");
        } else if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            if content.to_lowercase().contains("bazzite") {
                log.push_str("Detected Bazzite. Immutable handling may be needed.\n");
            }
        }

        // Step 1: Verify SLSsteam.so source exists
        if !Path::new(&slssteam_path).exists() {
            return Err(format!("SLSsteam.so not found at: {}", slssteam_path));
        }

        // Step 2: Create directories
        log.push_str("Creating directories...\n");
        let slssteam_dir = home.join(".local/share/SLSsteam");
        let config_dir = home.join(".config/SLSsteam");
        let apps_dir = home.join(".local/share/applications");

        std::fs::create_dir_all(&slssteam_dir)
            .map_err(|e| format!("Failed to create SLSsteam dir: {}", e))?;
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
        std::fs::create_dir_all(&apps_dir)
            .map_err(|e| format!("Failed to create applications dir: {}", e))?;

        // Step 3: Copy SLSsteam.so
        log.push_str("Copying SLSsteam.so...\n");
        let dest_so = slssteam_dir.join("SLSsteam.so");
        std::fs::copy(&slssteam_path, &dest_so)
            .map_err(|e| format!("Failed to copy SLSsteam.so: {}", e))?;

        // Set permissions (chmod 755)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest_so)
                .map_err(|e| format!("Failed to get permissions: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_so, perms)
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }
        log.push_str("SLSsteam.so copied and permissions set.\n");

        // Step 4: Write default config.yaml
        log.push_str("Writing default config.yaml...\n");
        let config_file = config_dir.join("config.yaml");
        let default_config = r#"#Example AppIds Config for those not familiar with YAML:
#AppIds:
#  - 440
#  - 730
#Take care of not messing up your spaces! Otherwise it won't work

#Example of DlcData:
#DlcData:
#  AppId:
#    FirstDlcAppId: "Dlc Name"
#    SecondDlcAppId: "Dlc Name"

#Example of DenuvoGames:
#DenuvoGames:
#  SteamId:
#    -  AppId1
#    -  AppId2

#Example of FakeAppIds:
#FakeAppIds:
#  AppId1: FakeAppId1
#  AppId2: FakeAppId2

#Disables Family Share license locking for self and others
DisableFamilyShareLock: yes

#Switches to whitelist instead of the default blacklist
UseWhitelist: no

#Automatically filter Apps in CheckAppOwnership. Filters everything but Games and Applications. Should not affect DLC checks
#Overrides black-/whitelist. Gets overriden by AdditionalApps
AutoFilterList: yes

#List of AppIds to ex-/include
AppIds:

#Enables playing of not owned games. Respects black-/whitelist AppIds
PlayNotOwnedGames: yes

#Additional AppIds to inject (Overrides your black-/whitelist & also overrides OwnerIds for apps you got shared!) Best to use this only on games NOT in your library.
AdditionalApps:
# Game Name
- 480
#Extra Data for Dlcs belonging to a specific AppId. Only needed
#when the App you're playing is hit by Steams 64 DLC limit
DlcData:

#Used to retrieve ProductInfo from Steam servers for some games
AppTokens:

#Fake Steam being offline for specified AppIds. Same format as AppIds
FakeOffline:

#Change AppIds of games to enable networking features
#Use 0 as a key to set for all unowned Apps
FakeAppIds:

#Custom ingame statuses. Set AppId to 0 to disable
IdleStatus:
  AppId: 0
  Title: ""

UnownedStatus:
  AppId: 0
  Title: ""

#Blocks games from unlocking on wrong accounts
DenuvoGames:

#Automatically disable SLSsteam when steamclient.so does not match a predefined file hash that is known to work
#You should enable this if you're planing to use SLSsteam with Steam Deck's gamemode
SafeMode: yes

#Toggles notifications via notify-send
Notifications: yes

#Warn user via notification when steamclient.so hash differs from known safe hash
#Mostly useful for development so I don't accidentally miss an update
WarnHashMissmatch: no

#Notify when SLSsteam is done initializing
NotifyInit: yes

#Enable sending commands to SLSsteam via /tmp/SLSsteam.API
API: yes

#Log levels:
#Once = 0
#Debug = 1
#Info = 2
#NotifyShort = 3
#NotifyLong = 4
#Warn = 5
#None = 6
LogLevel: 2

#Logs all calls to Steamworks (this makes the logfile huge! Only useful for debugging/analyzing
ExtendedLogging: no
"#;
        std::fs::write(&config_file, default_config)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        log.push_str("Config.yaml written.\n");

        // Step 5: Patch steam.desktop
        log.push_str("Patching steam.desktop...\n");
        let ld_audit_path = dest_so.to_string_lossy();

        if Path::new("/usr/share/applications/steam.desktop").exists() {
            let original = std::fs::read_to_string("/usr/share/applications/steam.desktop")
                .map_err(|e| format!("Failed to read steam.desktop: {}", e))?;

            // Replace Exec=/ with Exec=env LD_AUDIT="..." /
            let patched = original.replace(
                "Exec=/",
                &format!("Exec=env LD_AUDIT=\"{}\" /", ld_audit_path),
            );

            let user_desktop = apps_dir.join("steam.desktop");
            std::fs::write(&user_desktop, patched)
                .map_err(|e| format!("Failed to write patched desktop: {}", e))?;
            log.push_str("steam.desktop patched.\n");
        } else {
            log.push_str("steam.desktop not found, skipping.\n");
        }

        // Step 6: Try to patch steam-jupiter (SteamOS Gaming Mode - requires sudo)
        if Path::new("/usr/bin/steam-jupiter").exists() {
            log.push_str("Attempting to patch steam-jupiter (requires sudo)...\n");

            // Backup first
            let backup_result = Command::new("sudo")
                .args([
                    "cp",
                    "/usr/bin/steam-jupiter",
                    &config_dir.join("steam-jupiter.bak").to_string_lossy(),
                ])
                .status();

            if backup_result.is_ok() {
                // Patch: replace exec with exec env LD_AUDIT=...
                let patch_cmd = format!(
                    "sudo sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT=\"{}\" /usr/lib/steam/steam|' /usr/bin/steam-jupiter",
                    ld_audit_path
                );
                let patch_result = Command::new("sh").args(["-c", &patch_cmd]).status();

                if patch_result.is_ok() {
                    log.push_str("steam-jupiter patched successfully.\n");
                } else {
                    log.push_str("steam-jupiter patch failed (may need sudo password).\n");
                }
            } else {
                log.push_str("Could not backup steam-jupiter (may need sudo password).\n");
            }
        } else {
            log.push_str("steam-jupiter not found (not SteamOS Gaming Mode).\n");
        }

        log.push_str("Local SLSsteam installation complete!\n");
        return Ok(log);
    }

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }
    if !Path::new(&slssteam_path).exists() {
        return Err(format!("SLSsteam.so not found at: {}", slssteam_path));
    }

    // Read SLSsteam.so bytes
    let slssteam_bytes =
        fs::read(&slssteam_path).map_err(|e| format!("Failed to read SLSsteam.so: {}", e))?;

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let mut log = String::new();

    // Step 1: Create SLSsteam directories
    log.push_str("Creating SLSsteam directories...\n");
    ssh_exec(&sess, "mkdir -p ~/.local/share/SLSsteam ~/.config/SLSsteam")?;

    // Step 1.5: Write default config.yaml
    log.push_str("Writing default config.yaml...\n");
    let default_config = r#"#Example AppIds Config for those not familiar with YAML:
#AppIds:
#  - 440
#  - 730
#Take care of not messing up your spaces! Otherwise it won't work

#Example of DlcData:
#DlcData:
#  AppId:
#    FirstDlcAppId: "Dlc Name"
#    SecondDlcAppId: "Dlc Name"

#Example of DenuvoGames:
#DenuvoGames:
#  SteamId:
#    -  AppId1
#    -  AppId2

#Example of FakeAppIds:
#FakeAppIds:
#  AppId1: FakeAppId1
#  AppId2: FakeAppId2

#Disables Family Share license locking for self and others
DisableFamilyShareLock: yes

#Switches to whitelist instead of the default blacklist
UseWhitelist: no

#Automatically filter Apps in CheckAppOwnership. Filters everything but Games and Applications. Should not affect DLC checks
#Overrides black-/whitelist. Gets overriden by AdditionalApps
AutoFilterList: yes

#List of AppIds to ex-/include
AppIds:

#Enables playing of not owned games. Respects black-/whitelist AppIds
PlayNotOwnedGames: yes

#Additional AppIds to inject (Overrides your black-/whitelist & also overrides OwnerIds for apps you got shared!) Best to use this only on games NOT in your library.
AdditionalApps:
# Game Name
- 480
#Extra Data for Dlcs belonging to a specific AppId. Only needed
#when the App you're playing is hit by Steams 64 DLC limit
DlcData:

#Used to retrieve ProductInfo from Steam servers for some games
AppTokens:

#Fake Steam being offline for specified AppIds. Same format as AppIds
FakeOffline:

#Change AppIds of games to enable networking features
#Use 0 as a key to set for all unowned Apps
FakeAppIds:

#Custom ingame statuses. Set AppId to 0 to disable
IdleStatus:
  AppId: 0
  Title: ""

UnownedStatus:
  AppId: 0
  Title: ""

#Blocks games from unlocking on wrong accounts
DenuvoGames:

#Automatically disable SLSsteam when steamclient.so does not match a predefined file hash that is known to work
#You should enable this if you're planing to use SLSsteam with Steam Deck's gamemode
SafeMode: yes

#Toggles notifications via notify-send
Notifications: yes

#Warn user via notification when steamclient.so hash differs from known safe hash
#Mostly useful for development so I don't accidentally miss an update
WarnHashMissmatch: no

#Notify when SLSsteam is done initializing
NotifyInit: yes

#Enable sending commands to SLSsteam via /tmp/SLSsteam.API
API: yes

#Log levels:
#Once = 0
#Debug = 1
#Info = 2
#NotifyShort = 3
#NotifyLong = 4
#Warn = 5
#None = 6
LogLevel: 2

#Logs all calls to Steamworks (this makes the logfile huge! Only useful for debugging/analyzing
ExtendedLogging: no
"#;

    let config_remote_path = "/home/deck/.config/SLSsteam/config.yaml";
    // Check if config exists first? Or overwrite? User said "example config to put while installing", implying overwrite/init.
    // Let's safe-write: only if not exists? No, user usually wants to reset or init. Overwrite is safer for reliable state.

    let sftp_config = sess.sftp().map_err(|e| format!("SFTP error: {}", e))?;
    let mut config_file = sftp_config
        .create(Path::new(config_remote_path))
        .map_err(|e| format!("Failed to create config file: {}", e))?;
    config_file
        .write_all(default_config.as_bytes())
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    drop(config_file);
    drop(sftp_config); // Drop sftp to avoid borrowing issues with sess later

    log.push_str("Default config.yaml written.\n");

    // Step 2: Upload SLSsteam.so via SFTP
    log.push_str("Uploading SLSsteam.so...\n");
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

    let remote_path = "/home/deck/.local/share/SLSsteam/SLSsteam.so";
    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("Failed to create remote file: {}", e))?;

    remote_file
        .write_all(&slssteam_bytes)
        .map_err(|e| format!("Failed to write SLSsteam.so: {}", e))?;
    drop(remote_file);

    // Step 3: Set permissions
    ssh_exec(&sess, "chmod 755 ~/.local/share/SLSsteam/SLSsteam.so")?;
    log.push_str("SLSsteam.so uploaded and permissions set.\n");

    // Step 4: Verify file exists
    let verify = ssh_exec(
        &sess,
        "test -f ~/.local/share/SLSsteam/SLSsteam.so && echo 'OK' || echo 'FAIL'",
    )?;
    if !verify.contains("OK") {
        return Err("SLSsteam.so upload verification failed".to_string());
    }
    log.push_str("Upload verified.\n");

    // Step 5: Create user applications directory
    ssh_exec(&sess, "mkdir -p ~/.local/share/applications")?;

    // Step 6: Copy and modify steam.desktop
    log.push_str("Modifying steam.desktop...\n");
    let desktop_cmd = r#"
        if [ -f /usr/share/applications/steam.desktop ]; then
            cp /usr/share/applications/steam.desktop ~/.local/share/applications/
            sed -i 's|^Exec=/|Exec=env LD_AUDIT="/home/deck/.local/share/SLSsteam/SLSsteam.so" /|' ~/.local/share/applications/steam.desktop
            echo 'DESKTOP_OK'
        else
            echo 'DESKTOP_SKIP'
        fi
    "#;
    let desktop_result = ssh_exec(&sess, desktop_cmd)?;
    log.push_str(&format!("Desktop result: {}\n", desktop_result.trim()));

    // Step 7: Modify steam-jupiter (requires sudo)
    log.push_str("Modifying steam-jupiter (requires sudo)...\n");

    // First backup
    let sudo_backup = format!(
        "echo '{}' | sudo -S cp /usr/bin/steam-jupiter ~/.config/SLSsteam/steam-jupiter.bak 2>&1",
        root_password
    );
    ssh_exec(&sess, &sudo_backup)?;
    log.push_str("Backup created.\n");

    // Patch steam-jupiter: replace exec with exec env LD_AUDIT=...
    let patch_cmd = format!(
        r#"echo '{}' | sudo -S sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT="/home/deck/.local/share/SLSsteam/SLSsteam.so" /usr/lib/steam/steam|' /usr/bin/steam-jupiter 2>&1"#,
        root_password
    );
    let patch_result = ssh_exec(&sess, &patch_cmd)?;
    log.push_str(&format!("Patch result: {}\n", patch_result.trim()));

    // Note: config.yaml is already written in Step 1.5 above

    log.push_str("\nSLSsteam installation complete!\n");
    log.push_str("Please restart Steam on your Steam Deck for changes to take effect.");

    Ok(log)
}

/// Helper function to execute SSH command and return output
pub fn ssh_exec(sess: &ssh2::Session, cmd: &str) -> Result<String, String> {
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec(cmd)
        .map_err(|e| format!("Failed to exec: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output).ok();
    channel.wait_close().ok();

    Ok(output)
}

// ============================================================================
// SETTINGS COMMANDS
// ============================================================================

/// Save API key to secure store
#[tauri::command]
pub async fn save_api_key(key: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.set("api_key", serde_json::json!(key));
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

/// Get API key from secure store
#[tauri::command]
pub async fn get_api_key(app_handle: tauri::AppHandle) -> Result<String, String> {
    get_api_key_internal(&app_handle)
}

/// Internal helper to get API key
fn get_api_key_internal(app_handle: &tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;

    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let key = store
        .get("api_key")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    Ok(key)
}

// ============================================================================
// TESTS
// ============================================================================

// ============================================================================
// SLSSTEAM AUTO-FETCH FROM GITHUB
// ============================================================================

/// Get the path to the cached SLSsteam.so file
fn get_slssteam_cache_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let cache_dir = home.join(".cache/tontondeck/slssteam");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;
    Ok(cache_dir)
}

/// Get currently cached SLSsteam version (from version.txt file)
#[tauri::command]
pub async fn get_cached_slssteam_version() -> Result<Option<String>, String> {
    let cache_dir = get_slssteam_cache_dir()?;
    let version_file = cache_dir.join("version.txt");

    if version_file.exists() {
        let version = std::fs::read_to_string(&version_file)
            .map_err(|e| format!("Failed to read version: {}", e))?;
        Ok(Some(version.trim().to_string()))
    } else {
        Ok(None)
    }
}

/// Check if SLSsteam.so is cached
#[tauri::command]
pub async fn get_cached_slssteam_path() -> Result<Option<String>, String> {
    let cache_dir = get_slssteam_cache_dir()?;
    let so_path = cache_dir.join("SLSsteam.so");

    if so_path.exists() {
        Ok(Some(so_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

/// GitHub Release Asset structure
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// GitHub Release structure
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

/// Fetch latest SLSsteam from GitHub releases
#[tauri::command]
pub async fn fetch_latest_slssteam() -> Result<String, String> {
    // 1. Get latest release info from GitHub API
    let client = reqwest::Client::builder()
        .user_agent("TontonDeck/1.0")
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let release: GitHubRelease = client
        .get("https://api.github.com/repos/AceSLS/SLSsteam/releases/latest")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // 2. Find SLSsteam-Any.7z asset
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "SLSsteam-Any.7z")
        .ok_or("SLSsteam-Any.7z not found in release assets")?;

    // 3. Download the 7z file
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    // 4. Extract bin/SLSsteam.so from 7z
    let cache_dir = get_slssteam_cache_dir()?;
    let temp_7z = cache_dir.join("SLSsteam-Any.7z");

    std::fs::write(&temp_7z, &bytes).map_err(|e| format!("Failed to write 7z file: {}", e))?;

    // Extract using sevenz-rust
    let extract_dir = cache_dir.join("extract");
    let _ = std::fs::remove_dir_all(&extract_dir); // Clean previous extraction
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extract dir: {}", e))?;

    sevenz_rust::decompress_file(&temp_7z, &extract_dir)
        .map_err(|e| format!("7z extraction failed: {}", e))?;

    // 5. Find and move SLSsteam.so to cache root
    let so_source = extract_dir.join("bin/SLSsteam.so");
    if !so_source.exists() {
        return Err("bin/SLSsteam.so not found in archive".to_string());
    }

    let so_dest = cache_dir.join("SLSsteam.so");
    std::fs::copy(&so_source, &so_dest)
        .map_err(|e| format!("Failed to copy SLSsteam.so: {}", e))?;

    // 6. Save version (tag_name from release, e.g., "20251206112936")
    let version_file = cache_dir.join("version.txt");
    std::fs::write(&version_file, &release.tag_name)
        .map_err(|e| format!("Failed to write version: {}", e))?;

    // 7. Cleanup
    let _ = std::fs::remove_file(&temp_7z);
    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(format!(
        "SLSsteam {} downloaded successfully",
        release.tag_name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_app_empty_config() {
        let result = add_app_to_config("", "12345").unwrap();
        assert!(result.contains("AdditionalApps"));
        assert!(result.contains("12345"));
    }

    #[test]
    fn test_add_app_existing_config() {
        let existing = "SomeKey: value\nAdditionalApps:\n  - 11111\n";
        let result = add_app_to_config(existing, "22222").unwrap();
        assert!(result.contains("11111"));
        assert!(result.contains("22222"));
    }

    #[test]
    fn test_add_app_no_duplicate() {
        let existing = "AdditionalApps:\n  - 12345\n";
        let result = add_app_to_config(existing, "12345").unwrap();
        // Should only appear once
        let count = result.matches("12345").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_add_app_creates_list() {
        let existing = "SomeOtherKey: true\n";
        let result = add_app_to_config(existing, "99999").unwrap();
        assert!(result.contains("AdditionalApps"));
        assert!(result.contains("99999"));
    }
}
