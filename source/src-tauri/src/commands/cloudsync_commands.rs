//! CloudSync Commands - Tauri commands for cloud save synchronization
//!
//! This module provides commands for:
//! - Managing CloudSync configuration
//! - Getting sync status for games
//! - Triggering manual sync operations

use crate::cloudsync::{
    parse_remotecache_vdf, resolve_cloud_file_path, CloudSyncConfig, CloudStatus,
    GameCloudStatus, GlobalCloudStatus, SyncResult, WebDavClient,
};
use crate::cloudsync_watcher::CloudSyncWatcherState;
use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use walkdir::WalkDir;
use crate::pcgamingwiki;
use crate::cloudsync::{get_steam_user_id, CloudFile};
use std::collections::HashMap;

// ============================================================================
// Configuration Commands
// ============================================================================

/// Save CloudSync configuration to settings store
#[tauri::command]
pub async fn save_cloudsync_config(
    config: CloudSyncConfig,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.set(
        "cloudsync_config",
        serde_json::to_value(&config).map_err(|e| format!("Failed to serialize config: {}", e))?,
    );
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

/// Get CloudSync configuration from settings store
#[tauri::command]
pub async fn get_cloudsync_config(
    app_handle: tauri::AppHandle,
) -> Result<Option<CloudSyncConfig>, String> {
    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let config = store
        .get("cloudsync_config")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    Ok(config)
}

/// Test WebDAV connection with provided configuration
#[tauri::command]
pub async fn test_cloudsync_connection(config: CloudSyncConfig) -> Result<String, String> {
    if !config.enabled {
        return Err("CloudSync is not enabled".to_string());
    }

    if config.webdav_url.is_empty() {
        return Err("WebDAV URL is required".to_string());
    }

    let client = WebDavClient::new(&config)?;
    client.test_connection().await
}

// ============================================================================
// Status Commands
// ============================================================================

/// Get cloud sync status for a specific game
#[tauri::command]
pub async fn get_game_cloud_status(
    app_id: String,
    app_handle: tauri::AppHandle,
) -> Result<GameCloudStatus, String> {
    // Check if CloudSync is enabled
    let config = get_cloudsync_config(app_handle.clone()).await?;
    let config = match config {
        Some(c) if c.enabled => c,
        _ => {
            return Ok(GameCloudStatus {
                app_id,
                status: CloudStatus::None,
                last_sync: None,
                pending_files: None,
                error_message: None,
                source: "none".to_string(),
            });
        }
    };

    // Find remotecache.vdf for this game
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".local/share/Steam")
    };

    let userdata_path = steam_path.join("userdata");
    let mut remotecache_path: Option<PathBuf> = None;

    // Find the remotecache.vdf file for this app
    if let Ok(user_dirs) = std::fs::read_dir(&userdata_path) {
        for user_entry in user_dirs.flatten() {
            let candidate = user_entry.path().join(&app_id).join("remotecache.vdf");
            if candidate.exists() {
                remotecache_path = Some(candidate);
                break;
            }
        }
    }

    let remotecache_path = match remotecache_path {
        Some(p) => p,
        None => {
            // Attempt PCGamingWiki fallback
            let steam_id = get_steam_user_id();
            let save_roots = pcgamingwiki::find_save_locations(&app_id, steam_id.as_deref())
                .await
                .ok();

            if let Some(roots) = save_roots {
                if !roots.is_empty() {
                    let mut local_count = 0;
                    
                    // Count files in these roots
                    for root in roots {
                        if root.exists() {
                             local_count += WalkDir::new(root)
                                .into_iter()
                                .filter_map(|e| e.ok())
                                .filter(|e| e.file_type().is_file())
                                .count();
                        }
                    }

                    // Check remote status
                    let client = WebDavClient::new(&config)?;
                    let remote_files = client.list_files(&app_id).await.unwrap_or_default();
                    let remote_count = remote_files.len();

                    let status = if remote_count == 0 && local_count > 0 {
                        CloudStatus::Pending
                    } else if remote_count == local_count {
                        CloudStatus::Synced
                    } else {
                        CloudStatus::Pending
                    };

                   return Ok(GameCloudStatus {
                        app_id,
                        status,
                        last_sync: None,
                        pending_files: Some((local_count.abs_diff(remote_count)) as u32),
                        error_message: None, 
                        source: "pcgamingwiki".to_string(),
                    });
                }
            }

            return Ok(GameCloudStatus {
                app_id,
                status: CloudStatus::None,
                last_sync: None,
                pending_files: None,
                error_message: Some("No cloud save data found (checked Steam & PCGamingWiki)".to_string()),
                source: "none".to_string(),
            });
        }
    };

    // Parse the remotecache.vdf
    let content = std::fs::read_to_string(&remotecache_path)
        .map_err(|e| format!("Failed to read remotecache.vdf: {}", e))?;

    let files = parse_remotecache_vdf(&content)?;

    if files.is_empty() {
        return Ok(GameCloudStatus {
            app_id,
            status: CloudStatus::None,
            last_sync: None,
            pending_files: None,
            error_message: None,
        });
    }

    // Check remote status
    let client = WebDavClient::new(&config)?;
    let remote_files = client.list_files(&app_id).await.unwrap_or_default();

    let local_count = files.len();
    let remote_count = remote_files.len();

    // Simple status determination
    let status = if remote_count == 0 && local_count > 0 {
        CloudStatus::Pending
    } else if remote_count == local_count {
        CloudStatus::Synced
    } else {
        CloudStatus::Pending
    };

    Ok(GameCloudStatus {
        app_id,
        status,
        last_sync: None, // TODO: Track last sync time
        pending_files: Some((local_count.abs_diff(remote_count)) as u32),
        error_message: None,
        source: "steam_cloud".to_string(),
    })
}

/// Get global cloud sync status
#[tauri::command]
pub async fn get_global_cloud_status(
    app_handle: tauri::AppHandle,
) -> Result<GlobalCloudStatus, String> {
    let config = get_cloudsync_config(app_handle.clone()).await?;

    match config {
        Some(c) if c.enabled => Ok(GlobalCloudStatus {
            enabled: true,
            is_syncing: false, // TODO: Track active sync state
            games_synced: 0,
            games_pending: 0,
            games_with_conflicts: 0,
            last_sync: None,
        }),
        _ => Ok(GlobalCloudStatus {
            enabled: false,
            is_syncing: false,
            games_synced: 0,
            games_pending: 0,
            games_with_conflicts: 0,
            last_sync: None,
        }),
    }
}

// ============================================================================
// Sync Commands
// ============================================================================

/// Sync cloud saves for a specific game
#[tauri::command]
pub async fn sync_game_cloud_saves(
    app_id: String,
    app_handle: tauri::AppHandle,
) -> Result<SyncResult, String> {
    // Get config
    let config = get_cloudsync_config(app_handle.clone()).await?;
    let config = match config {
        Some(c) if c.enabled => c,
        _ => {
            return Err("CloudSync is not enabled".to_string());
        }
    };

    // Find remotecache.vdf
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".local/share/Steam")
    };

    let userdata_path = steam_path.join("userdata");
    let mut remotecache_path: Option<PathBuf> = None;
    let mut user_id: Option<String> = None;

    if let Ok(user_dirs) = std::fs::read_dir(&userdata_path) {
        for user_entry in user_dirs.flatten() {
            let candidate = user_entry.path().join(&app_id).join("remotecache.vdf");
            if candidate.exists() {
                remotecache_path = Some(candidate);
                user_id = user_entry.file_name().into_string().ok();
                break;
            }
        }
    }

    let (files, is_fallback) = match remotecache_path {
        Some(p) => {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| format!("Failed to read remotecache.vdf: {}", e))?;
             (parse_remotecache_vdf(&content)?, false)
        },
        None => {
            // PCGamingWiki Fallback
            let mut found_files = HashMap::new();
            let steam_id = get_steam_user_id();
            
            // Update the outer user_id option if we found one
            if let Some(ref sid) = steam_id {
                user_id = Some(sid.clone());
            }

            let save_roots = pcgamingwiki::find_save_locations(&app_id, steam_id.as_deref())
                .await
                .map_err(|e| format!("PCGamingWiki lookup failed: {}", e))?;

            if save_roots.is_empty() {
                 return Ok(SyncResult {
                    success: false,
                    message: "No cloud save data found via PCGamingWiki".to_string(),
                    files_uploaded: 0,
                    files_downloaded: 0,
                    conflicts: vec![],
                });
            }

            for root in save_roots {
                if root.exists() {
                    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                        if entry.file_type().is_file() {
                             let path = entry.path();
                             // Create relative path from root
                             if let Ok(rel_path) = path.strip_prefix(&root) {
                                let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
                                
                                let metadata = std::fs::metadata(path).ok();
                                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                                let time = metadata.as_ref()
                                    .and_then(|m| m.modified().ok())
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);

                                found_files.insert(rel_path_str.clone(), CloudFile {
                                    path: rel_path_str,
                                    root: -1, // Custom/Absolute
                                    size,
                                    localtime: time,
                                    remotetime: 0,
                                    sha: String::new(),
                                    syncstate: 0,
                                    resolved_path: Some(path.to_path_buf()),
                                });
                             }
                        }
                    }
                }
            }
             
            if found_files.is_empty() {
                 return Ok(SyncResult {
                    success: false,
                    message: "PCGamingWiki path found but no files exist".to_string(),
                    files_uploaded: 0,
                    files_downloaded: 0,
                    conflicts: vec![],
                });
            }
            
            (found_files, true)
        }
    };

    let user_id = user_id.unwrap_or_default();

    if files.is_empty() {
        return Ok(SyncResult {
            success: true,
            message: "No files to sync".to_string(),
            files_uploaded: 0,
            files_downloaded: 0,
            conflicts: vec![],
        });
    }

    let client = WebDavClient::new(&config)?;
    let mut files_uploaded = 0u32;
    let mut files_downloaded = 0u32;
    let conflicts: Vec<String> = Vec::new();

    // Process each file
    for (file_path, cloud_file) in &files {
        // Resolve local path
        // Resolve local path
        let local_path = if is_fallback {
             cloud_file.resolved_path.clone().unwrap() // Fallback always has resolved_path
        } else {
             match resolve_cloud_file_path(&cloud_file, &app_id, &user_id, None) {
                Some(p) => p,
                None => {
                    eprintln!(
                        "[CloudSync] Could not resolve path for {}: root={}",
                        file_path, cloud_file.root
                    );
                    continue;
                }
            }
        };

        // Check if local file exists
        let local_exists = local_path.exists();
        let local_time = if local_exists {
            std::fs::metadata(&local_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(cloud_file.localtime)
        } else {
            0
        };

        // Check remote file
        let remote_time = client
            .get_file_info(&app_id, file_path)
            .await
            .unwrap_or(None);

        match (local_exists, remote_time) {
            (true, None) => {
                // Local exists, remote doesn't - upload
                if let Ok(data) = std::fs::read(&local_path) {
                    if let Err(e) = client.upload_file(&app_id, file_path, data).await {
                        eprintln!("[CloudSync] Upload failed for {}: {}", file_path, e);
                    } else {
                        files_uploaded += 1;
                        eprintln!("[CloudSync] Uploaded: {}", file_path);
                    }
                }
            }
            (false, Some(_)) => {
                // Remote exists, local doesn't - download
                match client.download_file(&app_id, file_path).await {
                    Ok(data) => {
                        // Create parent directories
                        if let Some(parent) = local_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = std::fs::write(&local_path, data) {
                            eprintln!("[CloudSync] Write failed for {}: {}", file_path, e);
                        } else {
                            files_downloaded += 1;
                            eprintln!("[CloudSync] Downloaded: {}", file_path);
                        }
                    }
                    Err(e) => {
                        eprintln!("[CloudSync] Download failed for {}: {}", file_path, e);
                    }
                }
            }
            (true, Some(remote_ts)) => {
                // Both exist - compare timestamps
                let time_diff = if local_time > remote_ts {
                    local_time - remote_ts
                } else {
                    remote_ts - local_time
                };

                // Allow 5 second tolerance for clock skew
                if time_diff <= 5 {
                    // Files are in sync
                    continue;
                }

                if local_time > remote_ts {
                    // Local is newer - upload
                    if let Ok(data) = std::fs::read(&local_path) {
                        if let Err(e) = client.upload_file(&app_id, file_path, data).await {
                            eprintln!("[CloudSync] Upload failed for {}: {}", file_path, e);
                        } else {
                            files_uploaded += 1;
                            eprintln!("[CloudSync] Uploaded (newer): {}", file_path);
                        }
                    }
                } else {
                    // Remote is newer - download
                    match client.download_file(&app_id, file_path).await {
                        Ok(data) => {
                            if let Some(parent) = local_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Err(e) = std::fs::write(&local_path, data) {
                                eprintln!("[CloudSync] Write failed for {}: {}", file_path, e);
                            } else {
                                files_downloaded += 1;
                                eprintln!("[CloudSync] Downloaded (newer): {}", file_path);
                            }
                        }
                        Err(e) => {
                            eprintln!("[CloudSync] Download failed for {}: {}", file_path, e);
                        }
                    }
                }
            }
            (false, None) => {
                // Neither exists - skip
            }
        }
    }

    Ok(SyncResult {
        success: true,
        message: format!(
            "Sync complete: {} uploaded, {} downloaded",
            files_uploaded, files_downloaded
        ),
        files_uploaded,
        files_downloaded,
        conflicts,
    })
}

// ============================================================================
// Watcher Commands
// ============================================================================

/// Start the cloud sync file watcher
#[tauri::command]
pub async fn start_cloud_watcher(
    app_ids: Vec<String>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Get or create watcher state
    let state = match app_handle.try_state::<CloudSyncWatcherState>() {
        Some(s) => s,
        None => {
            // State not initialized - this would be done in setup
            return Err("CloudSync watcher not initialized".to_string());
        }
    };
    
    // Start watcher with event handler
    let _handle = app_handle.clone();
    state.start(app_ids, move |event| {
        eprintln!(
            "[CloudSync] File changed: app_id={}, path={:?}",
            event.app_id,
            event.path
        );
        // TODO: Trigger sync for this app_id
        // This would emit an event to the frontend or directly call sync
    })?;

    Ok(())
}

/// Stop the cloud sync file watcher
#[tauri::command]
pub async fn stop_cloud_watcher(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(state) = app_handle.try_state::<CloudSyncWatcherState>() {
        state.stop();
    }

    Ok(())
}

/// Check if cloud watcher is running
#[tauri::command]
pub async fn is_cloud_watcher_running(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(app_handle
        .try_state::<CloudSyncWatcherState>()
        .map(|s| s.is_running())
        .unwrap_or(false))
}
