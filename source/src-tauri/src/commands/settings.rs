//! Settings and SLSsteam cache management commands

use serde::Deserialize;
use std::path::PathBuf;

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
pub fn get_api_key_internal(app_handle: &tauri::AppHandle) -> Result<String, String> {
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
// Achievement Method Settings (SLScheevo vs SLSah)
// ============================================================================

/// Save achievement generation method: "steam_cm" (SLScheevo) or "web_api" (SLSah)
#[tauri::command]
pub async fn save_achievement_method(method: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    if method != "steam_cm" && method != "web_api" {
        return Err(format!("Invalid method '{}'. Use 'steam_cm' or 'web_api'", method));
    }

    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.set("achievement_method", serde_json::json!(method));
    store.save().map_err(|e| format!("Failed to save: {}", e))?;

    Ok(())
}

/// Get current achievement method (defaults to "web_api")
#[tauri::command]
pub async fn get_achievement_method(app_handle: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;

    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let method = store
        .get("achievement_method")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "web_api".to_string());

    Ok(method)
}

/// Get the path to the cached SLSsteam.so file
pub fn get_slssteam_cache_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let cache_dir = home.join(".cache/boilerroom/slssteam");
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
        .user_agent("BoilerRoom/1.0")
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
