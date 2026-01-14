//! Manifest cache management
//!
//! Caches downloaded manifest ZIPs to avoid re-downloading and re-entering API keys.
//! Cache location: ~/.cache/boilerroom/manifests/{appId}.zip

use std::path::PathBuf;

/// Get the cache directory for manifest files
fn get_manifest_cache_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let cache_dir = home.join(".cache/boilerroom/manifests");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;
    Ok(cache_dir)
}

/// Get the path where a manifest would be cached
fn get_cached_manifest_path(app_id: &str) -> Result<PathBuf, String> {
    let cache_dir = get_manifest_cache_dir()?;
    Ok(cache_dir.join(format!("{}.zip", app_id)))
}

/// Cache a manifest ZIP file for an app
/// Copies the downloaded ZIP to the cache directory
#[tauri::command]
pub async fn cache_manifest(app_id: String, source_path: String) -> Result<String, String> {
    let cache_path = get_cached_manifest_path(&app_id)?;

    std::fs::copy(&source_path, &cache_path)
        .map_err(|e| format!("Failed to cache manifest: {}", e))?;

    Ok(cache_path.to_string_lossy().to_string())
}

/// Check if a cached manifest exists for an app
/// Returns the path if it exists, None otherwise
#[tauri::command]
pub async fn get_cached_manifest(app_id: String) -> Result<Option<String>, String> {
    let cache_path = get_cached_manifest_path(&app_id)?;

    if cache_path.exists() {
        Ok(Some(cache_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

/// Clear a single cached manifest
#[tauri::command]
pub async fn clear_cached_manifest(app_id: String) -> Result<(), String> {
    let cache_path = get_cached_manifest_path(&app_id)?;

    if cache_path.exists() {
        std::fs::remove_file(&cache_path)
            .map_err(|e| format!("Failed to remove cached manifest: {}", e))?;
    }

    Ok(())
}

/// Clear all cached manifests and return count of files removed
#[tauri::command]
pub async fn clear_manifest_cache() -> Result<usize, String> {
    let cache_dir = get_manifest_cache_dir()?;

    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "zip")
                .unwrap_or(false)
            {
                if std::fs::remove_file(entry.path()).is_ok() {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

/// Get info about the manifest cache
#[tauri::command]
pub async fn get_manifest_cache_info() -> Result<ManifestCacheInfo, String> {
    let cache_dir = get_manifest_cache_dir()?;

    let mut total_size: u64 = 0;
    let mut count: usize = 0;

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "zip")
                .unwrap_or(false)
            {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    count += 1;
                }
            }
        }
    }

    Ok(ManifestCacheInfo {
        count,
        total_size,
        path: cache_dir.to_string_lossy().to_string(),
    })
}

#[derive(serde::Serialize)]
pub struct ManifestCacheInfo {
    pub count: usize,
    pub total_size: u64,
    pub path: String,
}
