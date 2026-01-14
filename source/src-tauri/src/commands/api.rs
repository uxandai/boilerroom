//! Depot Provider API, SteamGridDB, and artwork caching commands

// use reqwest::Client; // Unused (used fully qualified)
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// Search result from Depot Provider API
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

/// API status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotProviderApiStatus {
    pub health_ok: bool,
    pub user_stats: Option<DepotProviderUserStats>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotProviderUserStats {
    pub user_id: String,
    pub username: String,
    pub api_key_usage_count: i64,
    pub daily_usage: i64,
    pub daily_limit: i64,
    pub can_make_requests: bool,
}

/// Check Depot Provider API status and get user stats
#[tauri::command]
pub async fn check_depot_provider_api_status(
    api_key: String,
) -> Result<DepotProviderApiStatus, String> {
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
        return Ok(DepotProviderApiStatus {
            health_ok: false,
            user_stats: None,
            error: Some("API server unavailable".to_string()),
        });
    }

    // If no API key, return health only
    if api_key.is_empty() {
        return Ok(DepotProviderApiStatus {
            health_ok: true,
            user_stats: None,
            error: Some("No API key configured".to_string()),
        });
    }

    // Get user stats
    let stats_response = client
        .get(format!(
            "https://manifest.morrenus.xyz/api/v1/user/stats?api_key={}",
            api_key
        ))
        .send()
        .await;

    match stats_response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<DepotProviderUserStats>().await {
                    Ok(stats) => Ok(DepotProviderApiStatus {
                        health_ok: true,
                        user_stats: Some(stats),
                        error: None,
                    }),
                    Err(e) => Ok(DepotProviderApiStatus {
                        health_ok: true,
                        user_stats: None,
                        error: Some(format!("Failed to parse stats: {}", e)),
                    }),
                }
            } else if resp.status().as_u16() == 401 {
                Ok(DepotProviderApiStatus {
                    health_ok: true,
                    user_stats: None,
                    error: Some("Invalid API key".to_string()),
                })
            } else {
                Ok(DepotProviderApiStatus {
                    health_ok: true,
                    user_stats: None,
                    error: Some(format!("API error: {}", resp.status())),
                })
            }
        }
        Err(e) => Ok(DepotProviderApiStatus {
            health_ok: true,
            user_stats: None,
            error: Some(format!("Request failed: {}", e)),
        }),
    }
}

/// Search bundles from Depot Provider API
#[tauri::command]
pub async fn search_bundles(
    query: String,
    app_handle: tauri::AppHandle,
) -> Result<Vec<SearchResult>, String> {
    println!("[DEBUG] search_bundles called with query: {}", query);

    // Get API key from store
    let api_key = super::settings::get_api_key_internal(&app_handle)?;

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
                "API key expired or invalid. Please get a new key from Depot Provider.".to_string(),
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
pub fn get_artwork_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = dirs::cache_dir()
        .ok_or("Failed to find cache directory")?
        .join("boilerroom")
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

/// Download bundle from Depot Provider API to temp directory
#[tauri::command]
pub async fn download_bundle(
    app_id: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let api_key = super::settings::get_api_key_internal(&app_handle)?;

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
    let file_path = temp_dir.join(format!("boilerroom_{}.zip", app_id));

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}
