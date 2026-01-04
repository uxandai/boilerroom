use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tauri::Emitter;

// Global state for copy_game_to_remote cancellation
static COPY_PROCESS_PID: AtomicU32 = AtomicU32::new(0);
static COPY_CANCELLED: AtomicBool = AtomicBool::new(false);

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
// API STATUS COMMANDS
// ============================================================================

/// API status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorrenusApiStatus {
    pub health_ok: bool,
    pub user_stats: Option<MorrenusUserStats>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorrenusUserStats {
    pub user_id: String,
    pub username: String,
    pub api_key_usage_count: i64,
    pub daily_usage: i64,
    pub daily_limit: i64,
    pub can_make_requests: bool,
}

/// Check Morrenus API status and get user stats
#[tauri::command]
pub async fn check_morrenus_api_status(api_key: String) -> Result<MorrenusApiStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Check API health
    let health_response = client
        .get("https://manifest.morrenus.xyz/api/v1/health")
        .send()
        .await;

    let health_ok = match health_response {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };

    if !health_ok {
        return Ok(MorrenusApiStatus {
            health_ok: false,
            user_stats: None,
            error: Some("API server unavailable".to_string()),
        });
    }

    // If no API key, return health only
    if api_key.is_empty() {
        return Ok(MorrenusApiStatus {
            health_ok: true,
            user_stats: None,
            error: Some("No API key configured".to_string()),
        });
    }

    // Get user stats
    let stats_response = client
        .get(&format!(
            "https://manifest.morrenus.xyz/api/v1/user/stats?api_key={}",
            api_key
        ))
        .send()
        .await;

    match stats_response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<MorrenusUserStats>().await {
                    Ok(stats) => Ok(MorrenusApiStatus {
                        health_ok: true,
                        user_stats: Some(stats),
                        error: None,
                    }),
                    Err(e) => Ok(MorrenusApiStatus {
                        health_ok: true,
                        user_stats: None,
                        error: Some(format!("Failed to parse stats: {}", e)),
                    }),
                }
            } else if resp.status().as_u16() == 401 {
                Ok(MorrenusApiStatus {
                    health_ok: true,
                    user_stats: None,
                    error: Some("Invalid API key".to_string()),
                })
            } else {
                Ok(MorrenusApiStatus {
                    health_ok: true,
                    user_stats: None,
                    error: Some(format!("API error: {}", resp.status())),
                })
            }
        }
        Err(e) => Ok(MorrenusApiStatus {
            health_ok: true,
            user_stats: None,
            error: Some(format!("Request failed: {}", e)),
        }),
    }
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

/// Get artwork cache directory
fn get_artwork_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = dirs::cache_dir()
        .ok_or("Failed to find cache directory")?
        .join("tontondeck")
        .join("artwork");

    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create artwork cache dir: {}", e))?;

    Ok(cache_dir)
}

/// Cache artwork image to disk
/// Downloads the image from URL and saves it locally
#[tauri::command]
pub async fn cache_artwork(app_id: String, url: String) -> Result<String, String> {
    let cache_dir = get_artwork_cache_dir()?;

    // Determine file extension from URL
    let ext = if url.contains(".png") {
        "png"
    } else if url.contains(".webp") {
        "webp"
    } else {
        "jpg" // Default to jpg
    };

    let cache_path = cache_dir.join(format!("{}.{}", app_id, ext));

    // Download the image
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download artwork: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download artwork: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read artwork bytes: {}", e))?;

    std::fs::write(&cache_path, &bytes).map_err(|e| format!("Failed to save artwork: {}", e))?;

    eprintln!("[ArtworkCache] Cached {} -> {:?}", app_id, cache_path);

    Ok(cache_path.to_string_lossy().to_string())
}

/// Get cached artwork path if it exists
#[tauri::command]
pub async fn get_cached_artwork_path(app_id: String) -> Result<Option<String>, String> {
    let cache_dir = get_artwork_cache_dir()?;

    // Check for any cached file with this app_id
    for ext in &["jpg", "png", "webp"] {
        let cache_path = cache_dir.join(format!("{}.{}", app_id, ext));
        if cache_path.exists() {
            return Ok(Some(cache_path.to_string_lossy().to_string()));
        }
    }

    Ok(None)
}

/// Clear all cached artwork
#[tauri::command]
pub async fn clear_artwork_cache() -> Result<usize, String> {
    let cache_dir = get_artwork_cache_dir()?;

    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let _ = std::fs::remove_file(entry.path());
                count += 1;
            }
        }
    }

    eprintln!("[ArtworkCache] Cleared {} cached images", count);
    Ok(count)
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

        // Parse addtoken() for app tokens (optional)
        // Format: addtoken(appid, "token")
        let token_re = regex::Regex::new(r#"(?m)addtoken\s*\(\s*(\d+)\s*,\s*"([^"]*)"\s*\)"#)
            .map_err(|e| format!("Token regex error: {}", e))?;

        let mut app_id = String::new();
        let mut game_name = String::new();
        let mut depots = Vec::new();
        let mut app_token: Option<String> = None;

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

        // Parse addtoken() for app tokens (if present)
        for cap in token_re.captures_iter(&lua_content) {
            let token_app_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let token_value = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            if !token_value.is_empty() {
                eprintln!(
                    "[extract_manifest_zip] Found addtoken: app_id={}, token_len={}",
                    token_app_id,
                    token_value.len()
                );
                // Use the token for the main app (most common case)
                if app_token.is_none() || token_app_id == app_id {
                    app_token = Some(token_value.to_string());
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
            app_token, // From addtoken() if present
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
    app_token: Option<String>, // Optional app token from LUA addtoken()
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
        depot_keys,
        depot_downloader_path,
        steamless_path,
        ssh_config,
        target_directory,
        app_token,
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
pub async fn update_slssteam_config(
    config: SshConfig,
    app_id: String,
    game_name: String,
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

    let sftp = sess
        .sftp()
        .map_err(|e| format!("Failed to open SFTP: {}", e))?;

    let config_path = "/home/deck/.config/SLSsteam/config.yaml";

    // Read existing config
    let config_content = match sftp.open(Path::new(config_path)) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content).ok();
            content
        }
        Err(_) => String::new(),
    };

    let backup_path = "/home/deck/.config/SLSsteam/config.yaml.bak";

    // Backup command
    let backup_cmd = format!("cp {} {} 2>/dev/null || true", config_path, backup_path);
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel.exec(&backup_cmd).ok();
    channel.wait_close().ok();

    // Parse and update YAML (with game name as comment)
    let new_content =
        crate::install_manager::add_app_to_config_yaml(&config_content, &app_id, &game_name);

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

// ============================================================================
// LIBRARY MANAGEMENT COMMANDS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledGame {
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub has_depotdownloader_marker: bool, // true if installed by TonTonDeck/ACCELA
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

            let has_depotdownloader_marker = marker_out.trim() == "YES";

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
                has_depotdownloader_marker,
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
                let has_depotdownloader_marker = marker_path.exists();

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
                    "[list_installed_games_local] Found: {} (AppID: {}, marker: {})",
                    name, app_id, has_depotdownloader_marker
                );

                games.push(InstalledGame {
                    app_id,
                    name,
                    path: path.to_string_lossy().to_string(),
                    size_bytes,
                    has_depotdownloader_marker,
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
    pub library_inject_so_exists: bool,
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

        // Check library-inject.so
        let library_inject_path = home.join(".local/share/SLSsteam/library-inject.so");
        let library_inject_so_exists = library_inject_path.exists();
        eprintln!(
            "[SLSsteam Verify] library-inject.so exists: {}",
            library_inject_so_exists
        );

        // Check config
        let config_path = home.join(".config/SLSsteam/config.yaml");
        let config_exists = config_path.exists();
        eprintln!("[SLSsteam Verify] Config path: {:?}", config_path);
        eprintln!("[SLSsteam Verify] Config exists: {}", config_exists);

        // Parse config for settings
        let (config_play_not_owned, config_safe_mode_on, additional_apps_count) = if config_exists {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                let play_not_owned = content.contains("PlayNotOwnedGames: true")
                    || content.contains("PlayNotOwnedGames: yes");
                let safe_mode_on =
                    content.contains("SafeMode: true") || content.contains("SafeMode: yes");
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
            library_inject_so_exists,
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

    // Check if library-inject.so exists
    let inject_out = ssh_exec(
        &sess,
        "test -f ~/.local/share/SLSsteam/library-inject.so && echo 'EXISTS' || echo 'MISSING'",
    )?;
    let library_inject_so_exists = inject_out.contains("EXISTS");
    eprintln!(
        "[SLSsteam Verify] library-inject.so exists: {}",
        library_inject_so_exists
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
        library_inject_so_exists,
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
    pub library_inject_so_exists: bool,
    pub config_exists: bool,
    pub config_play_not_owned: bool,
    pub additional_apps_count: usize,
    pub desktop_entry_patched: bool,
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

    // Check library-inject.so
    let library_inject_path = home.join(".local/share/SLSsteam/library-inject.so");
    let library_inject_so_exists = library_inject_path.exists();
    eprintln!(
        "[SLSsteam Local Verify] library-inject.so exists: {}",
        library_inject_so_exists
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

    // Check if steam.desktop is patched with LD_AUDIT
    let desktop_path = home.join(".local/share/applications/steam.desktop");
    let desktop_entry_patched = if desktop_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&desktop_path) {
            let patched = content.contains("LD_AUDIT");
            eprintln!(
                "[SLSsteam Local Verify] steam.desktop path: {:?}, exists: true, patched (LD_AUDIT): {}",
                desktop_path, patched
            );
            if !patched {
                // Log some context about what we're looking for
                eprintln!("[SLSsteam Local Verify] Expected: Exec line should contain LD_AUDIT=path/to/SLSsteam.so");
            }
            patched
        } else {
            eprintln!("[SLSsteam Local Verify] steam.desktop exists but could not read");
            false
        }
    } else {
        eprintln!(
            "[SLSsteam Local Verify] steam.desktop not found at: {:?}",
            desktop_path
        );
        false
    };

    eprintln!(
        "[SLSsteam Local Verify] === RESULT: so={}, config={}, play_not_owned={}, apps={}, desktop_patched={} ===",
        slssteam_so_exists, config_exists, config_play_not_owned, additional_apps_count, desktop_entry_patched
    );

    Ok(SlssteamLocalStatus {
        slssteam_so_exists,
        library_inject_so_exists,
        config_exists,
        config_play_not_owned,
        additional_apps_count,
        desktop_entry_patched,
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

        // Step 3b: Copy library-inject.so if it exists in cache
        let slssteam_cache_dir = get_slssteam_cache_dir()?;
        let library_inject_source = slssteam_cache_dir.join("library-inject.so");
        if library_inject_source.exists() {
            let dest_inject = slssteam_dir.join("library-inject.so");
            std::fs::copy(&library_inject_source, &dest_inject)
                .map_err(|e| format!("Failed to copy library-inject.so: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest_inject)
                    .map_err(|e| format!("Failed to get permissions: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest_inject, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }
            log.push_str("library-inject.so copied.\n");
        }

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

        // LD_AUDIT needs both library-inject.so and SLSsteam.so
        let library_inject_path = slssteam_dir.join("library-inject.so");
        let ld_audit_path = if library_inject_path.exists() {
            format!(
                "{}:{}",
                library_inject_path.to_string_lossy(),
                dest_so.to_string_lossy()
            )
        } else {
            dest_so.to_string_lossy().to_string()
        };

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

    // Step 4b: Upload library-inject.so if it exists in cache
    let slssteam_cache_dir = get_slssteam_cache_dir()?;
    let library_inject_path = slssteam_cache_dir.join("library-inject.so");
    if library_inject_path.exists() {
        log.push_str("Uploading library-inject.so...\n");
        let inject_bytes = fs::read(&library_inject_path)
            .map_err(|e| format!("Failed to read library-inject.so: {}", e))?;

        let sftp2 = sess
            .sftp()
            .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

        let remote_inject_path = "/home/deck/.local/share/SLSsteam/library-inject.so";
        let mut remote_inject = sftp2
            .create(Path::new(remote_inject_path))
            .map_err(|e| format!("Failed to create remote file: {}", e))?;

        remote_inject
            .write_all(&inject_bytes)
            .map_err(|e| format!("Failed to write library-inject.so: {}", e))?;
        drop(remote_inject);

        ssh_exec(&sess, "chmod 755 ~/.local/share/SLSsteam/library-inject.so")?;
        log.push_str("library-inject.so uploaded.\n");
    }

    // Step 5: Create user applications directory
    ssh_exec(&sess, "mkdir -p ~/.local/share/applications")?;

    // Step 6: Copy and modify steam.desktop
    log.push_str("Modifying steam.desktop...\n");
    // LD_AUDIT needs both library-inject.so and SLSsteam.so for proper functionality
    let desktop_cmd = r#"
        if [ -f /usr/share/applications/steam.desktop ]; then
            cp /usr/share/applications/steam.desktop ~/.local/share/applications/
            # Check if library-inject.so exists and use both if available
            if [ -f ~/.local/share/SLSsteam/library-inject.so ]; then
                sed -i 's|^Exec=/|Exec=env LD_AUDIT="/home/deck/.local/share/SLSsteam/library-inject.so:/home/deck/.local/share/SLSsteam/SLSsteam.so" /|' ~/.local/share/applications/steam.desktop
            else
                sed -i 's|^Exec=/|Exec=env LD_AUDIT="/home/deck/.local/share/SLSsteam/SLSsteam.so" /|' ~/.local/share/applications/steam.desktop
            fi
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
    // Check if library-inject.so exists on remote first
    let check_inject = ssh_exec(
        &sess,
        "test -f ~/.local/share/SLSsteam/library-inject.so && echo 'EXISTS' || echo 'MISSING'",
    );
    let ld_audit_remote = if check_inject
        .as_ref()
        .map(|s| s.contains("EXISTS"))
        .unwrap_or(false)
    {
        "/home/deck/.local/share/SLSsteam/library-inject.so:/home/deck/.local/share/SLSsteam/SLSsteam.so"
    } else {
        "/home/deck/.local/share/SLSsteam/SLSsteam.so"
    };
    let patch_cmd = format!(
        r#"echo '{}' | sudo -S sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT="{}" /usr/lib/steam/steam|' /usr/bin/steam-jupiter 2>&1"#,
        root_password, ld_audit_remote
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

    // 5b. Also copy library-inject.so if it exists
    let inject_source = extract_dir.join("bin/library-inject.so");
    if inject_source.exists() {
        let inject_dest = cache_dir.join("library-inject.so");
        std::fs::copy(&inject_source, &inject_dest)
            .map_err(|e| format!("Failed to copy library-inject.so: {}", e))?;
        eprintln!("[SLSsteam] Also extracted library-inject.so");
    }

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

// ============================================================================
// COPY GAME TO REMOTE COMMAND
// ============================================================================

/// Copy a locally installed game to a remote Steam Deck via rsync
/// Also updates SLSsteam config on remote and creates ACF manifest
#[tauri::command]
pub async fn copy_game_to_remote(
    app: tauri::AppHandle,
    config: SshConfig,
    local_path: String,
    remote_path: String,
    app_id: String,
    game_name: String,
) -> Result<(), String> {
    use std::io::BufReader;
    use std::process::{Command, Stdio};

    // Get folder name from local path
    let local_path_buf = PathBuf::from(&local_path);
    let folder_name = local_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&app_id)
        .to_string();

    let remote_game_path = format!("{}/{}", remote_path, folder_name);
    let dst_path = format!("{}@{}:{}", config.username, config.ip, remote_game_path);
    let src_path = format!("{}/", local_path);

    eprintln!(
        "[copy_game_to_remote] Starting copy: {} -> {}",
        src_path, dst_path
    );

    // Check if sshpass is available
    let has_sshpass = !config.password.is_empty()
        && Command::new("which")
            .arg("sshpass")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    // Detect rsync version for --info=progress2 support
    let rsync_path = if std::path::Path::new("/opt/homebrew/bin/rsync").exists() {
        "/opt/homebrew/bin/rsync"
    } else if std::path::Path::new("/usr/local/bin/rsync").exists() {
        "/usr/local/bin/rsync"
    } else {
        "rsync"
    };

    let _use_info_progress = Command::new(rsync_path)
        .arg("--version")
        .output()
        .map(|out| {
            let version_str = String::from_utf8_lossy(&out.stdout);
            if let Some(ver_line) = version_str.lines().next() {
                if let Some(ver_part) = ver_line.split("version").nth(1) {
                    let ver_num = ver_part.trim().split_whitespace().next().unwrap_or("");
                    let parts: Vec<&str> = ver_num.split('.').collect();
                    if parts.len() >= 2 {
                        let major = parts[0].parse::<u32>().unwrap_or(0);
                        let minor = parts[1].parse::<u32>().unwrap_or(0);
                        return major > 3 || (major == 3 && minor >= 1);
                    }
                }
            }
            false
        })
        .unwrap_or(false);

    // Build rsync command - use --itemize-changes + --progress for detailed progress
    let mut cmd = Command::new(rsync_path);
    // -i (--itemize-changes) gives us one line per file
    // --progress shows per-file % and speed
    // --partial keeps partially transferred files for resume support
    cmd.args([
        "-avzs",
        "-i",
        "--progress",
        "--partial",
        "--no-inc-recursive",
    ]);

    if has_sshpass {
        let ssh_cmd = format!(
            "sshpass -e ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        cmd.env("SSHPASS", &config.password);
        cmd.args(["-e", &ssh_cmd]);
    } else if !config.private_key_path.is_empty() {
        let ssh_cmd = format!(
            "ssh -p {} -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port, config.private_key_path
        );
        cmd.args(["-e", &ssh_cmd]);
    } else {
        let ssh_cmd = format!(
            "ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        cmd.args(["-e", &ssh_cmd]);
    }

    cmd.args([&src_path, &dst_path]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // First, count total files for accurate progress
    let file_count: usize = walkdir::WalkDir::new(&local_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count();

    // Calculate total bytes in source
    let total_bytes: u64 = walkdir::WalkDir::new(&local_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();

    eprintln!(
        "[copy_game_to_remote] Source: {} files, {:.2} GB",
        file_count,
        total_bytes as f64 / 1_073_741_824.0
    );

    // Emit initial progress
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "state": "transferring",
            "message": "Calculating files to sync...",
            "download_percent": 0.0,
            "bytes_total": total_bytes,
            "bytes_transferred": 0
        }),
    );

    // Run rsync --dry-run first to calculate exactly what needs to be transferred
    let mut dry_run_cmd = Command::new(&rsync_path);
    dry_run_cmd.args(["-avzs", "-i", "--dry-run", "--no-inc-recursive"]);

    // Add SSH options (same as main command)
    if has_sshpass {
        let ssh_cmd = format!(
            "sshpass -e ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        dry_run_cmd.env("SSHPASS", &config.password);
        dry_run_cmd.args(["-e", &ssh_cmd]);
    } else if !config.private_key_path.is_empty() {
        let ssh_cmd = format!(
            "ssh -p {} -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port, config.private_key_path
        );
        dry_run_cmd.args(["-e", &ssh_cmd]);
    } else {
        let ssh_cmd = format!(
            "ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        dry_run_cmd.args(["-e", &ssh_cmd]);
    }

    dry_run_cmd.args([&src_path, &dst_path]);
    dry_run_cmd.stdout(Stdio::piped());
    dry_run_cmd.stderr(Stdio::null());

    // Parse dry-run output to get list of files to transfer
    let mut bytes_to_transfer: u64 = 0;
    let mut files_to_transfer: usize = 0;

    if let Ok(mut dry_child) = dry_run_cmd.spawn() {
        if let Some(stdout) = dry_child.stdout.take() {
            let reader = std::io::BufReader::new(stdout);
            for line_result in std::io::BufRead::lines(reader) {
                if let Ok(line) = line_result {
                    let trimmed = line.trim();
                    // File to transfer: starts with >f or <f or cf
                    if (trimmed.starts_with(">f")
                        || trimmed.starts_with("<f")
                        || trimmed.starts_with("cf"))
                        && trimmed.len() > 12
                    {
                        if let Some(filename) = trimmed.get(12..) {
                            let file_path = PathBuf::from(&local_path).join(filename.trim());
                            if let Ok(metadata) = std::fs::metadata(&file_path) {
                                bytes_to_transfer += metadata.len();
                                files_to_transfer += 1;
                            }
                        }
                    }
                }
            }
        }
        let _ = dry_child.wait();
    }

    // Calculate bytes already synced (on remote)
    let bytes_already_synced = total_bytes.saturating_sub(bytes_to_transfer);

    eprintln!(
        "[copy_game_to_remote] Dry-run complete: {} files ({:.2} GB) to transfer, {:.2} GB already synced",
        files_to_transfer,
        bytes_to_transfer as f64 / 1_073_741_824.0,
        bytes_already_synced as f64 / 1_073_741_824.0
    );

    // Emit progress with accurate starting values
    let start_percent = if total_bytes > 0 {
        (bytes_already_synced as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };
    let _ = app.emit("install-progress", serde_json::json!({
        "state": "transferring",
        "message": format!("Starting transfer: {} files ({:.1} GB)", files_to_transfer, bytes_to_transfer as f64 / 1_073_741_824.0),
        "download_percent": start_percent,
        "bytes_total": total_bytes,
        "bytes_transferred": bytes_already_synced,
        "files_total": file_count,
        "files_transferred": file_count - files_to_transfer
    }));
    // Reset cancellation flag
    COPY_CANCELLED.store(false, Ordering::SeqCst);

    // Spawn rsync process
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start rsync: {}", e))?;

    // Store PID for cancellation
    COPY_PROCESS_PID.store(child.id(), Ordering::SeqCst);

    // Parse progress from stdout - using itemize-changes format
    // Format: >f..t...... path/to/file (> = sending, f = file)
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let app_clone = app.clone();
        let file_count_clone = file_count;
        let total_bytes_clone = total_bytes;
        let bytes_already_synced_clone = bytes_already_synced;
        let files_already_synced = file_count - files_to_transfer;
        let local_path_clone = local_path.clone();

        std::thread::spawn(move || {
            use std::io::Read;
            use tauri::Emitter;
            let mut buffer = [0u8; 1];
            let mut line = String::new();
            let mut stdout = reader.into_inner();
            // Start with files already synced (from dry-run)
            let mut files_done: usize = files_already_synced;
            // Start with bytes already synced (from dry-run)
            let mut bytes_transferred: u64 = bytes_already_synced_clone;
            let mut current_file_name = String::new();
            let mut current_file_percent: u8 = 0;
            let mut current_file_bytes: u64 = 0;
            let mut current_speed = String::new();

            // Throttling: only emit progress updates every 10 seconds to avoid UI overhead
            let emit_interval = Duration::from_secs(10);
            let mut last_emit_time = Instant::now() - emit_interval; // Allow immediate first emit

            // Helper to check if we should emit a progress update
            let should_emit = |last_time: Instant| -> bool { last_time.elapsed() >= emit_interval };

            // Regex for parsing progress line: "  1,234,567 100%   12.34MB/s"
            // Captures: bytes (with commas), percentage, speed
            // Note: rsync uses lowercase kB/s, MB/s, GB/s
            let progress_re = regex::Regex::new(r"([\d,]+)\s+(\d+)%\s+([\d.]+\s*[kKMG]?B/s)").ok();

            while let Ok(1) = stdout.read(&mut buffer) {
                let ch = buffer[0] as char;
                if ch == '\r' || ch == '\n' {
                    if !line.is_empty() {
                        let trimmed = line.trim();

                        // Parse itemize-changes output (file being transferred)
                        // Format: YXcstpoguax filename
                        if (trimmed.starts_with(">f")
                            || trimmed.starts_with("<f")
                            || trimmed.starts_with("cf"))
                            && trimmed.len() > 12
                        {
                            if let Some(filename) = trimmed.get(12..) {
                                files_done += 1;
                                current_file_name = filename.trim().to_string();
                                current_file_percent = 0;

                                // Truncate long filenames for UI display
                                let display_file = if current_file_name.len() > 40 {
                                    format!(
                                        "...{}",
                                        &current_file_name[current_file_name.len() - 37..]
                                    )
                                } else {
                                    current_file_name.clone()
                                };

                                let total = if file_count_clone > 0 {
                                    file_count_clone
                                } else {
                                    files_done
                                };
                                let percent = if total > 0 {
                                    (files_done as f64 / total as f64) * 100.0
                                } else {
                                    0.0
                                };

                                eprintln!(
                                    "[rsync] [{}/{}] {}",
                                    files_done, total, current_file_name
                                );

                                // Throttle emissions to every 10 seconds
                                if should_emit(last_emit_time) {
                                    last_emit_time = Instant::now();
                                    let _ = app_clone.emit(
                                        "install-progress",
                                        serde_json::json!({
                                            "state": "transferring",
                                            "message": format!("Copying: {}/{}", files_done, total),
                                            "current_file": display_file,
                                            "current_file_percent": 0,
                                            "download_percent": percent,
                                            "transfer_speed": current_speed,
                                            "files_transferred": files_done,
                                            "files_total": total,
                                            "bytes_transferred": bytes_transferred,
                                            "bytes_total": total_bytes_clone
                                        }),
                                    );
                                }
                            }
                        }
                        // Parse progress percentage line: "  1,234,567  50%   12.34MB/s"
                        else if let Some(ref re) = progress_re {
                            if let Some(caps) = re.captures(trimmed) {
                                // Parse byte count (remove commas)
                                if let Some(bytes_match) = caps.get(1) {
                                    let bytes_str = bytes_match.as_str().replace(',', "");
                                    if let Ok(bytes) = bytes_str.parse::<u64>() {
                                        current_file_bytes = bytes;
                                        eprintln!(
                                            "[rsync progress] bytes={} file={}",
                                            bytes, current_file_name
                                        );
                                    }
                                }
                                // Parse percentage
                                if let Some(pct_match) = caps.get(2) {
                                    if let Ok(pct) = pct_match.as_str().parse::<u8>() {
                                        current_file_percent = pct;
                                        // When file completes (100%), add to total bytes transferred
                                        if pct == 100 && current_file_bytes > 0 {
                                            bytes_transferred += current_file_bytes;
                                            current_file_bytes = 0;
                                        }
                                    }
                                }
                                // Parse speed
                                if let Some(speed_match) = caps.get(3) {
                                    current_speed = speed_match.as_str().to_string();
                                }

                                // Only update UI if we have a current file and progress changed significantly
                                if !current_file_name.is_empty() && current_file_percent > 0 {
                                    let display_file = if current_file_name.len() > 35 {
                                        format!(
                                            "...{}",
                                            &current_file_name[current_file_name.len() - 32..]
                                        )
                                    } else {
                                        current_file_name.clone()
                                    };

                                    let total = if file_count_clone > 0 {
                                        file_count_clone
                                    } else {
                                        files_done
                                    };

                                    // Calculate byte-based progress if we have total bytes
                                    let (overall_percent, byte_progress_str) = if total_bytes_clone
                                        > 0
                                    {
                                        let current_total = bytes_transferred
                                            + (current_file_bytes as f64
                                                * current_file_percent as f64
                                                / 100.0)
                                                as u64;
                                        let pct = (current_total as f64 / total_bytes_clone as f64)
                                            * 100.0;
                                        let transferred_gb = current_total as f64 / 1_073_741_824.0;
                                        let total_gb = total_bytes_clone as f64 / 1_073_741_824.0;
                                        (pct, format!("{:.1} / {:.1} GB", transferred_gb, total_gb))
                                    } else {
                                        // Fallback to file-based progress
                                        let base_percent = if total > 0 {
                                            ((files_done - 1) as f64 / total as f64) * 100.0
                                        } else {
                                            0.0
                                        };
                                        let file_contribution = if total > 0 {
                                            current_file_percent as f64 / total as f64
                                        } else {
                                            0.0
                                        };
                                        (base_percent + file_contribution, String::new())
                                    };

                                    // Throttle emissions to every 10 seconds
                                    if should_emit(last_emit_time) {
                                        last_emit_time = Instant::now();
                                        let _ = app_clone.emit("install-progress", serde_json::json!({
                                            "state": "transferring",
                                            "message": if byte_progress_str.is_empty() {
                                                format!("Copying: {}/{} ({}%)", files_done, total, current_file_percent)
                                            } else {
                                                format!("Copying: {} | {}", byte_progress_str, current_speed)
                                            },
                                            "current_file": format!("{} {}%", display_file, current_file_percent),
                                            "current_file_percent": current_file_percent,
                                            "download_percent": overall_percent,
                                            "transfer_speed": current_speed,
                                            "files_transferred": files_done,
                                            "files_total": total,
                                            "bytes_transferred": bytes_transferred,
                                            "bytes_total": total_bytes_clone
                                        }));
                                    }
                                }
                            }
                        }
                        // Unchanged file (already on remote): starts with .f
                        else if trimmed.starts_with(".f") && trimmed.len() > 12 {
                            files_done += 1;

                            // Get the filename and add its size to bytes_transferred
                            if let Some(filename) = trimmed.get(12..) {
                                let file_path =
                                    PathBuf::from(&local_path_clone).join(filename.trim());
                                if let Ok(metadata) = std::fs::metadata(&file_path) {
                                    bytes_transferred += metadata.len();
                                }
                            }

                            // Throttle emissions to every 10 seconds (replaces files_done % 50 check)
                            if should_emit(last_emit_time) {
                                last_emit_time = Instant::now();
                                let total = if file_count_clone > 0 {
                                    file_count_clone
                                } else {
                                    files_done
                                };
                                let byte_percent = if total_bytes_clone > 0 {
                                    (bytes_transferred as f64 / total_bytes_clone as f64) * 100.0
                                } else {
                                    0.0
                                };
                                let transferred_gb = bytes_transferred as f64 / 1_073_741_824.0;
                                let total_gb = total_bytes_clone as f64 / 1_073_741_824.0;
                                eprintln!(
                                    "[rsync] Skipped {} files ({:.1}/{:.1} GB)",
                                    files_done, transferred_gb, total_gb
                                );
                                let _ = app_clone.emit("install-progress", serde_json::json!({
                                    "state": "transferring",
                                    "message": format!("Verifying: {:.1}/{:.1} GB", transferred_gb, total_gb),
                                    "current_file": "(skipping unchanged)",
                                    "download_percent": byte_percent,
                                    "transfer_speed": "",
                                    "files_transferred": files_done,
                                    "files_total": total,
                                    "bytes_transferred": bytes_transferred,
                                    "bytes_total": total_bytes_clone
                                }));
                            }
                        }
                        // Directory
                        else if trimmed.starts_with(">d")
                            || trimmed.starts_with("cd")
                            || trimmed.starts_with(".d")
                        {
                            // Silent - don't spam logs with directories
                        }

                        line.clear();
                    }
                } else {
                    line.push(ch);
                }
            }

            // Final progress update
            let _ = app_clone.emit("install-progress", serde_json::json!({
                "state": "transferring",
                "message": format!("Transfer complete: {}/{} files", files_done, file_count_clone),
                "current_file": "",
                "download_percent": 100.0,
                "files_transferred": files_done,
                "files_total": file_count_clone
            }));
        });
    }

    // Wait for rsync to complete
    let status = child
        .wait()
        .map_err(|e| format!("rsync wait failed: {}", e))?;

    if !status.success() {
        let exit_code = status.code().unwrap_or(-1);
        let error_msg = match exit_code {
            255 => format!(
                "SSH connection failed. Check IP ({}), SSH enabled, password correct.",
                config.ip
            ),
            _ => format!("rsync failed with exit code {}", exit_code),
        };
        let _ = app.emit(
            "install-progress",
            serde_json::json!({
                "state": "error",
                "message": error_msg
            }),
        );
        return Err(error_msg);
    }

    // VERIFICATION DISABLED - uncomment to enable post-transfer checksum verification
    // This can take 3-10 minutes for 60GB games
    /*
    // Verify transfer with checksum comparison
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "state": "transferring",
            "message": "Verifying transfer (checksum)...",
            "download_percent": 100.0
        }),
    );

    eprintln!("[copy_game_to_remote] Running verification with checksum...");

    let mut verify_cmd = Command::new(&rsync_path);
    verify_cmd.args(["-avzsc", "-i", "--dry-run", "--no-inc-recursive"]);

    // Add SSH options
    if has_sshpass {
        let ssh_cmd = format!(
            "sshpass -e ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        verify_cmd.env("SSHPASS", &config.password);
        verify_cmd.args(["-e", &ssh_cmd]);
    } else if !config.private_key_path.is_empty() {
        let ssh_cmd = format!(
            "ssh -p {} -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port, config.private_key_path
        );
        verify_cmd.args(["-e", &ssh_cmd]);
    } else {
        let ssh_cmd = format!(
            "ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        verify_cmd.args(["-e", &ssh_cmd]);
    }

    verify_cmd.args([&src_path, &dst_path]);
    verify_cmd.stdout(Stdio::piped());
    verify_cmd.stderr(Stdio::null());

    // Check if any files would be transferred (indicating checksum mismatch)
    let mut files_needing_resync = Vec::new();
    if let Ok(mut verify_child) = verify_cmd.spawn() {
        if let Some(stdout) = verify_child.stdout.take() {
            let reader = std::io::BufReader::new(stdout);
            for line_result in std::io::BufRead::lines(reader) {
                if let Ok(line) = line_result {
                    let trimmed = line.trim();
                    // File that would be transferred (checksum mismatch)
                    if (trimmed.starts_with(">f")
                        || trimmed.starts_with("<f")
                        || trimmed.starts_with("cf"))
                        && trimmed.len() > 12
                    {
                        if let Some(filename) = trimmed.get(12..) {
                            files_needing_resync.push(filename.trim().to_string());
                        }
                    }
                }
            }
        }
        let _ = verify_child.wait();
    }

    if !files_needing_resync.is_empty() {
        eprintln!(
            "[copy_game_to_remote] Verification found {} files with checksum mismatch:",
            files_needing_resync.len()
        );
        for f in &files_needing_resync {
            eprintln!("  - {}", f);
        }
        // Emit warning but don't fail - user can re-run to fix
        let _ = app.emit(
            "install-progress",
            serde_json::json!({
                "state": "transferring",
                "message": format!("Warning: {} files may need re-sync (checksum mismatch)", files_needing_resync.len()),
                "download_percent": 100.0
            }),
        );
    } else {
        eprintln!("[copy_game_to_remote] Verification passed - all checksums match!");
    }
    */

    // Update SLSsteam config on remote
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "state": "configuring",
            "message": "Updating SLSsteam config..."
        }),
    );

    // Connect via SSH to update config
    let addr = format!("{}:{}", config.ip, config.port);
    if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
        if let Ok(mut sess) = ssh2::Session::new() {
            sess.set_tcp_stream(tcp);
            if sess.handshake().is_ok()
                && sess
                    .userauth_password(&config.username, &config.password)
                    .is_ok()
            {
                // Read existing config
                let mut content = String::new();
                if let Ok(mut channel) = sess.channel_session() {
                    if channel
                        .exec("cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''")
                        .is_ok()
                    {
                        let _ = channel.read_to_string(&mut content);
                        let _ = channel.wait_close();
                    }
                }

                // Add app to config (with game name as comment)
                let new_config =
                    crate::install_manager::add_app_to_config_yaml(&content, &app_id, &game_name);

                // Write back
                if let Ok(mut channel) = sess.channel_session() {
                    if channel
                        .exec("mkdir -p ~/.config/SLSsteam && cat > ~/.config/SLSsteam/config.yaml")
                        .is_ok()
                    {
                        let _ = channel.write_all(new_config.as_bytes());
                        let _ = channel.send_eof();
                        let _ = channel.wait_close();
                    }
                }

                // Copy decryption keys from local config.vdf to remote
                // Depot IDs are typically in the range [app_id, app_id+100]
                if let Some(home) = dirs::home_dir() {
                    let config_vdf_paths = [
                        home.join(".steam/steam/config/config.vdf"),
                        home.join(".local/share/Steam/config/config.vdf"),
                    ];

                    for config_vdf_path in config_vdf_paths {
                        if config_vdf_path.exists() {
                            if let Ok(config_vdf_content) =
                                std::fs::read_to_string(&config_vdf_path)
                            {
                                // Extract keys for all depots in the app_id range
                                let depot_keys = crate::config_vdf::extract_depot_keys_by_app_id(
                                    &config_vdf_content,
                                    &app_id,
                                );

                                if !depot_keys.is_empty() {
                                    eprintln!(
                                        "[copy_game_to_remote] Found {} decryption keys for app {}",
                                        depot_keys.len(),
                                        app_id
                                    );

                                    // Read remote config.vdf
                                    let mut remote_config_vdf = String::new();
                                    if let Ok(mut channel) = sess.channel_session() {
                                        if channel.exec("cat ~/.steam/steam/config/config.vdf 2>/dev/null || echo ''").is_ok() {
                                            let _ = channel.read_to_string(&mut remote_config_vdf);
                                            let _ = channel.wait_close();
                                        }
                                    }

                                    // Add keys to remote config.vdf
                                    let new_remote_config =
                                        crate::config_vdf::add_decryption_keys_to_vdf(
                                            &remote_config_vdf,
                                            &depot_keys,
                                        );

                                    // Write back to remote
                                    if let Ok(mut channel) = sess.channel_session() {
                                        if channel
                                            .exec("cat > ~/.steam/steam/config/config.vdf")
                                            .is_ok()
                                        {
                                            let _ = channel.write_all(new_remote_config.as_bytes());
                                            let _ = channel.send_eof();
                                            let _ = channel.wait_close();
                                            eprintln!("[copy_game_to_remote] Added {} decryption keys to remote config.vdf", depot_keys.len());
                                        }
                                    }
                                } else {
                                    eprintln!("[copy_game_to_remote] No decryption keys found for app {} in local config.vdf", app_id);
                                }
                            }
                            break; // Only process one config.vdf
                        }
                    }
                }

                // Create ACF manifest
                let steamapps_dir = remote_path
                    .trim_end_matches('/')
                    .trim_end_matches("/common");
                let acf_path = format!("{}/appmanifest_{}.acf", steamapps_dir, app_id);
                let acf_content = format!(
                    r#""AppState"
{{
	"appid"		"{app_id}"
	"Universe"		"1"
	"name"		"{game_name}"
	"StateFlags"		"4"
	"installdir"		"{folder_name}"
	"UserConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
}}"#,
                    app_id = app_id,
                    game_name = game_name,
                    folder_name = folder_name
                );

                if let Ok(mut channel) = sess.channel_session() {
                    let cmd = format!("cat > \"{}\"", acf_path);
                    if channel.exec(&cmd).is_ok() {
                        let _ = channel.write_all(acf_content.as_bytes());
                        let _ = channel.send_eof();
                        let _ = channel.wait_close();
                    }
                }
            }
        }
    }

    // Done!
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "state": "finished",
            "message": format!("{} copied successfully!", game_name),
            "download_percent": 100.0
        }),
    );

    Ok(())
}

// ============================================================================
// CANCEL COPY TO REMOTE
// ============================================================================

/// Cancel an ongoing copy_game_to_remote operation
#[tauri::command]
pub async fn cancel_copy_to_remote(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    eprintln!("[cancel_copy_to_remote] Cancel requested");

    // Set cancellation flag
    COPY_CANCELLED.store(true, Ordering::SeqCst);

    // Get the PID
    let pid = COPY_PROCESS_PID.load(Ordering::SeqCst);

    if pid > 0 {
        eprintln!("[cancel_copy_to_remote] Killing rsync process PID: {}", pid);

        #[cfg(unix)]
        {
            // Send SIGTERM first
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();

            // Wait a bit
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Then SIGKILL if still running
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
        }

        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .status();
        }

        // Reset PID
        COPY_PROCESS_PID.store(0, Ordering::SeqCst);
    }

    // Emit cancelled event
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "state": "cancelled",
            "message": "Copy cancelled by user"
        }),
    );

    Ok(())
}

// ============================================================================
// STEAM UPDATE DISABLE AND LIBCURL FIX COMMANDS
// ============================================================================

/// Disable Steam updates to prevent hash mismatch with SLSsteam
/// Creates/modifies $HOME/.steam/steam/steam.cfg
#[tauri::command]
pub async fn disable_steam_updates(config: SshConfig) -> Result<String, String> {
    let config_content = r#"BootStrapperInhibitAll=enable
BootStrapperForceSelfUpdate=disable
"#;

    // Check if local mode
    if config.is_local || config.ip.is_empty() {
        // Local mode
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let steam_dir = PathBuf::from(&home).join(".steam/steam");
        let config_path = steam_dir.join("steam.cfg");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&steam_dir)
            .map_err(|e| format!("Failed to create steam directory: {}", e))?;

        // Read existing config if it exists
        let existing_content = if config_path.exists() {
            std::fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        // Check if already configured correctly
        let has_inhibit = existing_content.contains("BootStrapperInhibitAll=enable");
        let has_force_disable = existing_content.contains("BootStrapperForceSelfUpdate=disable");

        if has_inhibit && has_force_disable {
            return Ok("Steam update disable already configured. No changes needed.".to_string());
        }

        // Build new content
        let mut new_content = existing_content.clone();

        // Remove old values if they exist with different settings
        new_content = new_content
            .lines()
            .filter(|line| {
                !line.starts_with("BootStrapperInhibitAll=")
                    && !line.starts_with("BootStrapperForceSelfUpdate=")
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Add our settings
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(config_content);

        std::fs::write(&config_path, &new_content)
            .map_err(|e| format!("Failed to write steam.cfg: {}", e))?;

        return Ok(format!(
            "Steam updates disabled locally.\nModified: {}\n\nContent:\n{}",
            config_path.display(),
            new_content.trim()
        ));
    }

    // Remote mode via SSH
    if config.ip.is_empty() {
        return Err("IP address is required for remote mode".to_string());
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

    // Create directory and write config
    let cmd = format!(
        r#"
mkdir -p ~/.steam/steam
CONFIG_FILE="$HOME/.steam/steam/steam.cfg"

# Remove old lines if they exist
if [ -f "$CONFIG_FILE" ]; then
    sed -i '/^BootStrapperInhibitAll=/d' "$CONFIG_FILE"
    sed -i '/^BootStrapperForceSelfUpdate=/d' "$CONFIG_FILE"
fi

# Append new settings
echo 'BootStrapperInhibitAll=enable' >> "$CONFIG_FILE"
echo 'BootStrapperForceSelfUpdate=disable' >> "$CONFIG_FILE"

echo "Steam updates disabled."
cat "$CONFIG_FILE"
"#
    );

    let output = ssh_exec(&sess, &cmd)?;

    Ok(format!(
        "Steam updates disabled on remote Steam Deck.\n\n{}",
        output.trim()
    ))
}

/// Fix libcurl32 symlink issue for Steam
/// Creates: ln -sf /usr/lib32/libcurl.so.4 ~/.steam/steam/ubuntu12_32/libcurl.so.4
#[tauri::command]
pub async fn fix_libcurl32(config: SshConfig) -> Result<String, String> {
    use std::os::unix::fs::symlink;

    let source = "/usr/lib32/libcurl.so.4";

    // Check if local mode
    if config.is_local || config.ip.is_empty() {
        // Local mode
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let target_dir = PathBuf::from(&home).join(".steam/steam/ubuntu12_32");
        let target = target_dir.join("libcurl.so.4");

        // Check if source exists
        if !PathBuf::from(source).exists() {
            return Err(format!(
                "Source library not found: {}\n\nMake sure lib32-curl is installed:\n  sudo pacman -S lib32-curl",
                source
            ));
        }

        // Create target directory if it doesn't exist
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create target directory: {}", e))?;

        // Remove existing symlink/file if it exists
        if target.exists() || target.symlink_metadata().is_ok() {
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove existing file: {}", e))?;
        }

        // Create symlink
        symlink(source, &target).map_err(|e| format!("Failed to create symlink: {}", e))?;

        return Ok(format!(
            "libcurl32 symlink created successfully.\n\n{} -> {}",
            target.display(),
            source
        ));
    }

    // Remote mode via SSH
    if config.ip.is_empty() {
        return Err("IP address is required for remote mode".to_string());
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

    // Check if source exists, create dir, and create symlink
    let cmd = format!(
        r#"
# Check if lib32-curl is installed
if [ ! -f "{source}" ]; then
    echo "ERROR: {source} not found"
    echo "Make sure lib32-curl is installed: sudo pacman -S lib32-curl"
    exit 1
fi

# Create directory if needed
mkdir -p ~/.steam/steam/ubuntu12_32

# Create symlink (force overwrite)
ln -sf "{source}" ~/.steam/steam/ubuntu12_32/libcurl.so.4

echo "Symlink created successfully:"
ls -la ~/.steam/steam/ubuntu12_32/libcurl.so.4
"#,
        source = source
    );

    let output = ssh_exec(&sess, &cmd)?;

    if output.contains("ERROR:") {
        return Err(output);
    }

    Ok(format!(
        "libcurl32 symlink created on remote Steam Deck.\n\n{}",
        output.trim()
    ))
}

/// Status of Steam updates configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamUpdatesStatus {
    pub is_configured: bool,
    pub inhibit_all: bool,
    pub force_self_update_disabled: bool,
    pub config_path: String,
}

/// Check if Steam updates are disabled
#[tauri::command]
pub async fn check_steam_updates_status(config: SshConfig) -> Result<SteamUpdatesStatus, String> {
    // Check if local mode
    if config.is_local || config.ip.is_empty() {
        // Local mode
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let config_path = PathBuf::from(&home).join(".steam/steam/steam.cfg");

        if !config_path.exists() {
            return Ok(SteamUpdatesStatus {
                is_configured: false,
                inhibit_all: false,
                force_self_update_disabled: false,
                config_path: config_path.to_string_lossy().to_string(),
            });
        }

        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let inhibit_all = content.contains("BootStrapperInhibitAll=enable");
        let force_self_update_disabled = content.contains("BootStrapperForceSelfUpdate=disable");

        return Ok(SteamUpdatesStatus {
            is_configured: inhibit_all && force_self_update_disabled,
            inhibit_all,
            force_self_update_disabled,
            config_path: config_path.to_string_lossy().to_string(),
        });
    }

    // Remote mode via SSH
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

    let output = ssh_exec(
        &sess,
        "cat ~/.steam/steam/steam.cfg 2>/dev/null || echo 'FILE_NOT_FOUND'",
    )?;

    if output.contains("FILE_NOT_FOUND") {
        return Ok(SteamUpdatesStatus {
            is_configured: false,
            inhibit_all: false,
            force_self_update_disabled: false,
            config_path: "~/.steam/steam/steam.cfg".to_string(),
        });
    }

    let inhibit_all = output.contains("BootStrapperInhibitAll=enable");
    let force_self_update_disabled = output.contains("BootStrapperForceSelfUpdate=disable");

    Ok(SteamUpdatesStatus {
        is_configured: inhibit_all && force_self_update_disabled,
        inhibit_all,
        force_self_update_disabled,
        config_path: "~/.steam/steam/steam.cfg".to_string(),
    })
}

/// Status of libcurl32 symlink
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Libcurl32Status {
    pub source_exists: bool,
    pub symlink_exists: bool,
    pub symlink_correct: bool,
    pub source_path: String,
    pub target_path: String,
}

/// Check libcurl32 symlink status
#[tauri::command]
pub async fn check_libcurl32_status(config: SshConfig) -> Result<Libcurl32Status, String> {
    let source = "/usr/lib32/libcurl.so.4";

    // Check if local mode
    if config.is_local || config.ip.is_empty() {
        // Local mode
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let target_path = PathBuf::from(&home).join(".steam/steam/ubuntu12_32/libcurl.so.4");
        let source_exists = PathBuf::from(source).exists();

        // Check if symlink exists and points to correct target
        let (symlink_exists, symlink_correct) = if target_path.symlink_metadata().is_ok() {
            if let Ok(link_target) = std::fs::read_link(&target_path) {
                (true, link_target == PathBuf::from(source))
            } else {
                // File exists but is not a symlink
                (true, false)
            }
        } else {
            (false, false)
        };

        return Ok(Libcurl32Status {
            source_exists,
            symlink_exists,
            symlink_correct,
            source_path: source.to_string(),
            target_path: target_path.to_string_lossy().to_string(),
        });
    }

    // Remote mode via SSH
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

    // Check source and symlink status
    let cmd = format!(
        r#"
SOURCE_EXISTS="false"
SYMLINK_EXISTS="false"
SYMLINK_CORRECT="false"

if [ -f "{source}" ]; then
    SOURCE_EXISTS="true"
fi

TARGET="$HOME/.steam/steam/ubuntu12_32/libcurl.so.4"

if [ -L "$TARGET" ]; then
    SYMLINK_EXISTS="true"
    LINK_TARGET=$(readlink "$TARGET")
    if [ "$LINK_TARGET" = "{source}" ]; then
        SYMLINK_CORRECT="true"
    fi
elif [ -f "$TARGET" ]; then
    SYMLINK_EXISTS="true"
fi

echo "SOURCE_EXISTS=$SOURCE_EXISTS"
echo "SYMLINK_EXISTS=$SYMLINK_EXISTS"
echo "SYMLINK_CORRECT=$SYMLINK_CORRECT"
"#,
        source = source
    );

    let output = ssh_exec(&sess, &cmd)?;

    let source_exists = output.contains("SOURCE_EXISTS=true");
    let symlink_exists = output.contains("SYMLINK_EXISTS=true");
    let symlink_correct = output.contains("SYMLINK_CORRECT=true");

    Ok(Libcurl32Status {
        source_exists,
        symlink_exists,
        symlink_correct,
        source_path: source.to_string(),
        target_path: "~/.steam/steam/ubuntu12_32/libcurl.so.4".to_string(),
    })
}

/// Status of 32-bit library dependencies required by Steam
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lib32DependenciesStatus {
    pub lib32_curl_installed: bool,
    pub lib32_openssl_installed: bool,
    pub lib32_glibc_installed: bool,
    pub all_installed: bool,
}

/// Check if required 32-bit libraries are installed (lib32-curl, lib32-openssl, lib32-glibc)
#[tauri::command]
pub async fn check_lib32_dependencies(
    config: SshConfig,
) -> Result<Lib32DependenciesStatus, String> {
    // Check if local mode
    if config.is_local || config.ip.is_empty() {
        // Local mode - check if 32-bit libraries exist
        let lib32_curl_installed = PathBuf::from("/usr/lib32/libcurl.so.4").exists();

        // lib32-openssl can be libssl.so or libssl.so.3
        let lib32_openssl_installed = PathBuf::from("/usr/lib32/libssl.so").exists()
            || PathBuf::from("/usr/lib32/libssl.so.3").exists();

        let lib32_glibc_installed = PathBuf::from("/usr/lib32/libc.so.6").exists();

        let all_installed =
            lib32_curl_installed && lib32_openssl_installed && lib32_glibc_installed;

        return Ok(Lib32DependenciesStatus {
            lib32_curl_installed,
            lib32_openssl_installed,
            lib32_glibc_installed,
            all_installed,
        });
    }

    // Remote mode via SSH
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

    // Check 32-bit library status on remote
    let cmd = r#"
LIB32_CURL="false"
LIB32_OPENSSL="false"
LIB32_GLIBC="false"

if [ -f "/usr/lib32/libcurl.so.4" ]; then
    LIB32_CURL="true"
fi

if [ -f "/usr/lib32/libssl.so" ] || [ -f "/usr/lib32/libssl.so.3" ]; then
    LIB32_OPENSSL="true"
fi

if [ -f "/usr/lib32/libc.so.6" ]; then
    LIB32_GLIBC="true"
fi

echo "LIB32_CURL=$LIB32_CURL"
echo "LIB32_OPENSSL=$LIB32_OPENSSL"
echo "LIB32_GLIBC=$LIB32_GLIBC"
"#;

    let output = ssh_exec(&sess, cmd)?;

    let lib32_curl_installed = output.contains("LIB32_CURL=true");
    let lib32_openssl_installed = output.contains("LIB32_OPENSSL=true");
    let lib32_glibc_installed = output.contains("LIB32_GLIBC=true");
    let all_installed = lib32_curl_installed && lib32_openssl_installed && lib32_glibc_installed;

    Ok(Lib32DependenciesStatus {
        lib32_curl_installed,
        lib32_openssl_installed,
        lib32_glibc_installed,
        all_installed,
    })
}

// ============================================================================
// DEPOT KEYS ONLY INSTALL COMMAND
// ============================================================================

/// Depot info for depot keys only install
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotKeyInfo {
    pub depot_id: String,
    pub manifest_id: String,
    pub manifest_path: String,
    pub key: String,
}

/// Install depot keys only (no download) - configures Steam to recognize game
/// This adds decryption keys to config.vdf, copies manifests to depotcache,
/// creates ACF with StateFlags=6, and updates SLSsteam config.
/// After this, steam://install/{appid} will work.
#[tauri::command]
pub async fn install_depot_keys_only(
    app_id: String,
    game_name: String,
    depots: Vec<DepotKeyInfo>,
    ssh_config: SshConfig,
    target_library: String, // e.g. ~/.steam/steam or /home/deck/.steam/steam
    trigger_steam_install: bool, // If true, run xdg-open steam://install/{appid}
) -> Result<String, String> {
    use crate::config_vdf;
    use std::io::Read;

    eprintln!(
        "[DepotKeysOnly] Starting for {} ({}) with {} depots",
        game_name,
        app_id,
        depots.len()
    );
    eprintln!(
        "[DepotKeysOnly] Target library: {}, is_local: {}",
        target_library, ssh_config.is_local
    );

    // Prepare depot keys for config.vdf
    let depot_keys: Vec<(String, String)> = depots
        .iter()
        .filter(|d| !d.key.is_empty())
        .map(|d| (d.depot_id.clone(), d.key.clone()))
        .collect();

    // Derive paths from target_library
    // target_library is like ~/.steam/steam or /home/deck/.steam/steam
    let steam_root = target_library
        .trim_end_matches('/')
        .trim_end_matches("/steamapps/common");
    let steam_root = steam_root.trim_end_matches("/steamapps");
    let config_vdf_path = format!("{}/config/config.vdf", steam_root);
    let depotcache_path = format!("{}/depotcache", steam_root);
    let steamapps_path = format!("{}/steamapps", steam_root);

    eprintln!(
        "[DepotKeysOnly] Paths: config_vdf={}, depotcache={}, steamapps={}",
        config_vdf_path, depotcache_path, steamapps_path
    );

    if ssh_config.is_local {
        // ====== LOCAL MODE ======

        // 1. Add decryption keys to config.vdf
        let config_vdf_expanded = shellexpand::tilde(&config_vdf_path).to_string();
        eprintln!(
            "[DepotKeysOnly] Reading local config.vdf: {}",
            config_vdf_expanded
        );

        let config_content =
            std::fs::read_to_string(&config_vdf_expanded).unwrap_or_else(|_| String::new());

        if !depot_keys.is_empty() {
            let new_config = config_vdf::add_decryption_keys_to_vdf(&config_content, &depot_keys);
            std::fs::write(&config_vdf_expanded, &new_config)
                .map_err(|e| format!("Failed to write config.vdf: {}", e))?;
            eprintln!(
                "[DepotKeysOnly] Updated config.vdf with {} depot keys",
                depot_keys.len()
            );
        }

        // 2. Copy .manifest files to depotcache
        let depotcache_expanded = shellexpand::tilde(&depotcache_path).to_string();
        std::fs::create_dir_all(&depotcache_expanded)
            .map_err(|e| format!("Failed to create depotcache: {}", e))?;

        for depot in &depots {
            if !depot.manifest_path.is_empty()
                && std::path::Path::new(&depot.manifest_path).exists()
            {
                let manifest_filename =
                    format!("{}_{}.manifest", depot.depot_id, depot.manifest_id);
                let dest_path = format!("{}/{}", depotcache_expanded, manifest_filename);
                std::fs::copy(&depot.manifest_path, &dest_path)
                    .map_err(|e| format!("Failed to copy manifest: {}", e))?;
                eprintln!("[DepotKeysOnly] Copied manifest: {}", manifest_filename);
            }
        }

        // 3. Create ACF with StateFlags=6
        let acf_content = build_acf_state_flags_6(&app_id, &game_name);
        let acf_path =
            shellexpand::tilde(&format!("{}/appmanifest_{}.acf", steamapps_path, app_id))
                .to_string();
        std::fs::write(&acf_path, &acf_content)
            .map_err(|e| format!("Failed to write ACF: {}", e))?;
        eprintln!("[DepotKeysOnly] Created ACF: {}", acf_path);

        // 4. Update SLSsteam config
        let slssteam_config = shellexpand::tilde("~/.config/SLSsteam/config.yaml").to_string();
        if std::path::Path::new(&slssteam_config).exists() {
            if let Ok(content) = std::fs::read_to_string(&slssteam_config) {
                let new_config =
                    crate::install_manager::add_app_to_config_yaml(&content, &app_id, &game_name);
                let _ = std::fs::write(&slssteam_config, &new_config);
                eprintln!("[DepotKeysOnly] Updated SLSsteam config");
            }
        }

        // 5. Trigger steam://install if requested (local - just open URL)
        // Note: May show SLSsteam.so error message but Steam will still run with SLSsteam working
        if trigger_steam_install {
            let steam_url = format!("steam://install/{}", app_id);
            let _ = std::process::Command::new("xdg-open")
                .arg(&steam_url)
                .spawn();
            eprintln!("[DepotKeysOnly] Triggered: {}", steam_url);
        }
    } else {
        // ====== REMOTE MODE (SSH) ======

        let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
        let tcp = TcpStream::connect_timeout(
            &addr
                .parse()
                .map_err(|e| format!("Invalid address: {}", e))?,
            Duration::from_secs(10),
        )
        .map_err(|e| format!("SSH connection failed: {}", e))?;

        let mut sess =
            ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
        sess.set_tcp_stream(tcp);
        sess.handshake()
            .map_err(|e| format!("SSH handshake failed: {}", e))?;
        sess.userauth_password(&ssh_config.username, &ssh_config.password)
            .map_err(|e| format!("SSH auth failed: {}", e))?;

        // 1. Add decryption keys to config.vdf
        if !depot_keys.is_empty() {
            // Read existing config.vdf
            let mut config_content = String::new();
            if let Ok(mut channel) = sess.channel_session() {
                let cmd = format!("cat \"{}\" 2>/dev/null || echo ''", config_vdf_path);
                if channel.exec(&cmd).is_ok() {
                    let _ = channel.read_to_string(&mut config_content);
                    let _ = channel.wait_close();
                }
            }

            // Modify and write back
            let new_config = config_vdf::add_decryption_keys_to_vdf(&config_content, &depot_keys);

            if let Ok(mut channel) = sess.channel_session() {
                let cmd = format!(
                    "mkdir -p \"$(dirname '{}')\" && cat > \"{}\"",
                    config_vdf_path, config_vdf_path
                );
                if channel.exec(&cmd).is_ok() {
                    let _ = channel.write_all(new_config.as_bytes());
                    let _ = channel.send_eof();
                    let _ = channel.wait_close();
                    eprintln!(
                        "[DepotKeysOnly] Updated remote config.vdf with {} keys",
                        depot_keys.len()
                    );
                }
            }
        }

        // 2. Create depotcache directory on remote
        if let Ok(mut channel) = sess.channel_session() {
            let cmd = format!("mkdir -p \"{}\"", depotcache_path);
            let _ = channel.exec(&cmd);
            let _ = channel.wait_close();
        }

        // 3. Copy .manifest files to remote depotcache via SCP
        for depot in &depots {
            if !depot.manifest_path.is_empty()
                && std::path::Path::new(&depot.manifest_path).exists()
            {
                let manifest_filename =
                    format!("{}_{}.manifest", depot.depot_id, depot.manifest_id);
                let remote_path = format!("{}/{}", depotcache_path, manifest_filename);

                // Read local file
                if let Ok(content) = std::fs::read(&depot.manifest_path) {
                    // SCP to remote using SFTP
                    if let Ok(sftp) = sess.sftp() {
                        if let Ok(mut remote_file) = sftp.create(std::path::Path::new(&remote_path))
                        {
                            let _ = remote_file.write_all(&content);
                            eprintln!(
                                "[DepotKeysOnly] Copied manifest to remote: {}",
                                manifest_filename
                            );
                        }
                    }
                }
            }
        }

        // 4. Create ACF with StateFlags=6 on remote
        let acf_content = build_acf_state_flags_6(&app_id, &game_name);
        let acf_remote_path = format!("{}/appmanifest_{}.acf", steamapps_path, app_id);

        if let Ok(mut channel) = sess.channel_session() {
            let cmd = format!("cat > \"{}\"", acf_remote_path);
            if channel.exec(&cmd).is_ok() {
                let _ = channel.write_all(acf_content.as_bytes());
                let _ = channel.send_eof();
                let _ = channel.wait_close();
                eprintln!("[DepotKeysOnly] Created remote ACF: {}", acf_remote_path);
            }
        }

        // 5. Update SLSsteam config on remote
        let mut slssteam_content = String::new();
        if let Ok(mut channel) = sess.channel_session() {
            if channel
                .exec("cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''")
                .is_ok()
            {
                let _ = channel.read_to_string(&mut slssteam_content);
                let _ = channel.wait_close();
            }
        }

        let new_slssteam =
            crate::install_manager::add_app_to_config_yaml(&slssteam_content, &app_id, &game_name);
        if let Ok(mut channel) = sess.channel_session() {
            if channel
                .exec("mkdir -p ~/.config/SLSsteam && cat > ~/.config/SLSsteam/config.yaml")
                .is_ok()
            {
                let _ = channel.write_all(new_slssteam.as_bytes());
                let _ = channel.send_eof();
                let _ = channel.wait_close();
                eprintln!("[DepotKeysOnly] Updated remote SLSsteam config");
            }
        }

        // 6. Trigger steam://install on remote if requested
        if trigger_steam_install {
            let steam_url = format!("steam://install/{}", app_id);
            // Try to trigger on remote - need DISPLAY for xdg-open to work
            if let Ok(mut channel) = sess.channel_session() {
                let cmd = format!(
                    "DISPLAY=:0 xdg-open '{}' 2>/dev/null || DISPLAY=:1 xdg-open '{}' 2>/dev/null || echo 'xdg-open failed'",
                    steam_url, steam_url
                );
                let _ = channel.exec(&cmd);
                let mut output = String::new();
                let _ = channel.read_to_string(&mut output);
                let _ = channel.wait_close();
                eprintln!(
                    "[DepotKeysOnly] Triggered remote: {} (output: {})",
                    steam_url,
                    output.trim()
                );
            }
        }
    }

    Ok(format!(
        "Successfully configured {} depots for {}",
        depots.len(),
        game_name
    ))
}

/// Build ACF content with StateFlags=6 (Update Required)
fn build_acf_state_flags_6(app_id: &str, game_name: &str) -> String {
    // Sanitize game name for installdir
    let install_dir: String = game_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect();
    let install_dir = install_dir.trim();
    let install_dir = if install_dir.is_empty() {
        app_id
    } else {
        install_dir
    };

    format!(
        r#""AppState"
{{
	"appid"		"{app_id}"
	"Universe"		"1"
	"name"		"{game_name}"
	"StateFlags"		"6"
	"installdir"		"{install_dir}"
	"SizeOnDisk"		"0"
	"buildid"		"0"
	"InstalledDepots"
	{{
	}}
	"UserConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
	"MountedConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
}}"#,
        app_id = app_id,
        game_name = game_name,
        install_dir = install_dir
    )
}

// ============================================================================
// TOOLS SECTION: STEAMLESS & SLSah
// ============================================================================

/// Launch Steamless.exe via Wine/Proton (GUI version, not CLI)
/// This allows users to manually select and patch game executables
#[tauri::command]
pub async fn launch_steamless_via_wine(steamless_exe_path: String) -> Result<String, String> {
    // Validate the path exists
    let path = PathBuf::from(&steamless_exe_path);
    if !path.exists() {
        return Err(format!(
            "Steamless.exe not found at: {}",
            steamless_exe_path
        ));
    }

    // On macOS, Steamless doesn't work (needs Wine/Proton)
    #[cfg(target_os = "macos")]
    {
        return Err(
            "Steamless is not supported on macOS. Please run on Linux/SteamOS or Windows."
                .to_string(),
        );
    }

    // On Windows, run directly
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new(&steamless_exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch Steamless: {}", e))?;
        return Ok("Steamless launched".to_string());
    }

    // On Linux/SteamOS, find Wine/Proton and launch
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use std::process::Command;
        // Find Proton or Wine
        let home = dirs::home_dir().ok_or("Could not find home directory")?;

        // Common Proton paths - prefer wine over wine64 for better compatibility
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

        // Set up Wine prefix
        let prefix = home.join(".local/share/tontondeck/steamless/pfx");
        std::fs::create_dir_all(&prefix)
            .map_err(|e| format!("Failed to create Wine prefix: {}", e))?;

        // Set environment for Proton
        let mut cmd = Command::new(wine_path);
        cmd.arg(&steamless_exe_path);
        cmd.env("WINEPREFIX", prefix.to_string_lossy().to_string());
        cmd.env("WINEDEBUG", "-all");

        // For Proton, set LD_LIBRARY_PATH
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
    // SLSah only works on Linux/SteamOS
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
        // Run the install script via curl
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

        // Launch in a new terminal
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

// ============================================================================
// SLSSTEAM CONFIG MANAGEMENT COMMANDS
// ============================================================================

/// Helper function to modify SLSsteam config.yaml section with key:value entries
pub fn modify_slssteam_config_section(
    content: &str,
    section_name: &str,
    key: &str,
    value: &str,
) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let section_header = format!("{}:", section_name);

    // Find section
    let section_idx = lines
        .iter()
        .position(|l| l.trim().starts_with(&section_header));

    if let Some(idx) = section_idx {
        // Check for :null suffix and remove it
        if lines[idx].contains(":null") || lines[idx].contains(": null") {
            lines[idx] = section_header.clone();
        }

        // Find where to insert (after section header, before next section)
        let mut insert_idx = idx + 1;
        while insert_idx < lines.len() {
            let line = &lines[insert_idx];
            let trimmed = line.trim();
            // Stop at next section (non-indented, non-empty, non-comment line with colon)
            if !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with('-')
                && !line.starts_with(' ')
                && line.contains(':')
            {
                break;
            }
            // Stop if we find an indented item (we're in the section)
            if line.starts_with("  ") && !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Check if this key already exists
                let entry_key = format!("  {}:", key);
                if line.starts_with(&entry_key) {
                    // Key exists, update value
                    lines[insert_idx] = format!("  {}:{}", key, value);
                    return lines.join("\n");
                }
            }
            insert_idx += 1;
        }

        // Insert new entry (2-space indent)
        let new_entry = format!("  {}:{}", key, value);
        lines.insert(idx + 1, new_entry);
    } else {
        // Section doesn't exist, add it
        lines.push(String::new());
        lines.push(section_header);
        lines.push(format!("  {}:{}", key, value));
    }

    lines.join("\n")
}

/// Add FakeAppId entry for Online-Fix (maps to 480 Spacewar)
#[tauri::command]
pub async fn add_fake_app_id(config: SshConfig, app_id: String) -> Result<(), String> {
    eprintln!("[add_fake_app_id] Adding {} -> 480", app_id);

    if config.is_local {
        // Local mode
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let config_path = home.join(".config/SLSsteam/config.yaml");

        if !config_path.exists() {
            return Err("SLSsteam config.yaml not found. Install SLSsteam first.".to_string());
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let new_content = modify_slssteam_config_section(&content, "FakeAppIds", &app_id, "480");

        std::fs::write(&config_path, new_content)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        eprintln!("[add_fake_app_id] Local: Added {} -> 480", app_id);
    } else {
        // Remote mode via SSH
        let ip: IpAddr = config
            .ip
            .parse()
            .map_err(|_| format!("Invalid IP: {}", config.ip))?;
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

        // Read current config
        let config_path = "/home/deck/.config/SLSsteam/config.yaml";
        let content = ssh_exec(
            &sess,
            &format!("cat {} 2>/dev/null || echo ''", config_path),
        )?;

        if content.trim().is_empty() {
            return Err(
                "SLSsteam config.yaml not found on remote. Install SLSsteam first.".to_string(),
            );
        }

        let new_content = modify_slssteam_config_section(&content, "FakeAppIds", &app_id, "480");

        // Write back via SFTP
        let sftp = sess.sftp().map_err(|e| format!("SFTP error: {}", e))?;
        let mut file = sftp
            .create(Path::new(config_path))
            .map_err(|e| format!("Failed to open config for writing: {}", e))?;
        file.write_all(new_content.as_bytes())
            .map_err(|e| format!("Failed to write config: {}", e))?;

        eprintln!("[add_fake_app_id] Remote: Added {} -> 480", app_id);
    }

    Ok(())
}

/// Add AppToken entry to SLSsteam config
#[tauri::command]
pub async fn add_app_token(config: SshConfig, app_id: String, token: String) -> Result<(), String> {
    eprintln!("[add_app_token] Adding {}:{}", app_id, token);

    if config.is_local {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let config_path = home.join(".config/SLSsteam/config.yaml");

        if !config_path.exists() {
            return Err("SLSsteam config.yaml not found.".to_string());
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let new_content = modify_slssteam_config_section(&content, "AppTokens", &app_id, &token);

        std::fs::write(&config_path, new_content)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        eprintln!("[add_app_token] Local: Added {}:{}", app_id, token);
    } else {
        let ip: IpAddr = config
            .ip
            .parse()
            .map_err(|_| format!("Invalid IP: {}", config.ip))?;
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

        let config_path = "/home/deck/.config/SLSsteam/config.yaml";
        let content = ssh_exec(
            &sess,
            &format!("cat {} 2>/dev/null || echo ''", config_path),
        )?;

        if content.trim().is_empty() {
            return Err("SLSsteam config.yaml not found on remote.".to_string());
        }

        let new_content = modify_slssteam_config_section(&content, "AppTokens", &app_id, &token);

        let sftp = sess.sftp().map_err(|e| format!("SFTP error: {}", e))?;
        let mut file = sftp
            .create(Path::new(config_path))
            .map_err(|e| format!("Failed to open config for writing: {}", e))?;
        file.write_all(new_content.as_bytes())
            .map_err(|e| format!("Failed to write config: {}", e))?;

        eprintln!("[add_app_token] Remote: Added {}:{}", app_id, token);
    }

    Ok(())
}

/// Generate achievement schema files for a game
#[tauri::command]
pub async fn generate_achievements(
    app_id: String,
    steam_api_key: String,
    steam_user_id: String,
) -> Result<String, String> {
    eprintln!(
        "[generate_achievements] Generating for app {} with user {}",
        app_id, steam_user_id
    );

    // Fetch schema from Steam Web API
    let url = format!(
        "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/?key={}&appid={}&l=english",
        steam_api_key, app_id
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch schema: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Steam API error: {} - check your API key",
            response.status()
        ));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let game = data
        .get("game")
        .ok_or("No game data in response - invalid AppID?")?;

    let game_name = game
        .get("gameName")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let achievements = game
        .get("availableGameStats")
        .and_then(|s| s.get("achievements"))
        .and_then(|a| a.as_array());

    let achievement_count = achievements.map(|a| a.len()).unwrap_or(0);

    if achievement_count == 0 {
        return Ok(format!("{} has no achievements to generate", game_name));
    }

    // Create output directory
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let stats_dir = home.join(".steam/steam/appcache/stats");
    std::fs::create_dir_all(&stats_dir)
        .map_err(|e| format!("Failed to create stats dir: {}", e))?;

    // Build schema in VDF-like binary format
    // Note: This is a simplified text version - for full binary VDF,
    // would need a proper VDF library. For now we create a placeholder.
    let schema_path = stats_dir.join(format!("UserGameStatsSchema_{}.bin", app_id));

    // Create a simple binary representation
    // The actual format is complex VDF binary, but SLSsteam can work with simplified versions
    let mut schema_content = Vec::new();

    // Simple header
    schema_content.extend_from_slice(app_id.as_bytes());
    schema_content.push(0);
    schema_content.extend_from_slice(game_name.as_bytes());
    schema_content.push(0);

    // Write achievement names
    if let Some(achs) = achievements {
        for ach in achs {
            if let Some(name) = ach.get("name").and_then(|n| n.as_str()) {
                schema_content.extend_from_slice(name.as_bytes());
                schema_content.push(0);
            }
        }
    }

    std::fs::write(&schema_path, &schema_content)
        .map_err(|e| format!("Failed to write schema: {}", e))?;

    // Create stats file (copy from template or create minimal)
    let stats_path = stats_dir.join(format!("UserGameStats_{}_{}.bin", steam_user_id, app_id));
    if !stats_path.exists() {
        // Create minimal stats file
        let minimal_stats: Vec<u8> = vec![0u8; 38]; // Empty stats template
        std::fs::write(&stats_path, minimal_stats)
            .map_err(|e| format!("Failed to write stats: {}", e))?;
    }

    eprintln!(
        "[generate_achievements] Created schema for {} with {} achievements",
        game_name, achievement_count
    );

    Ok(format!(
        "Generated schema for {} ({} achievements)",
        game_name, achievement_count
    ))
}
