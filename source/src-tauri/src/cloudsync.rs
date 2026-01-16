//! CloudSync - WebDAV-based cloud save synchronization for BoilerRoom games
//!
//! This module handles:
//! - Parsing Steam's remotecache.vdf files
//! - Resolving save file paths from VDF root values
//! - WebDAV client operations (upload, download, list)
//! - Sync logic with conflict resolution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Types
// ============================================================================

/// CloudSync configuration stored in settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudSyncConfig {
    pub enabled: bool,
    pub provider: String, // "webdav", "gdrive", "dropbox", "onedrive"
    pub webdav_url: String,
    pub username: String,
    pub password: String,
}

/// Sync status for a single game
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CloudStatus {
    Synced,
    Pending,
    Syncing,
    Conflict,
    Error,
    None,
}

impl Default for CloudStatus {
    fn default() -> Self {
        CloudStatus::None
    }
}

/// Detailed cloud status for a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCloudStatus {
    pub app_id: String,
    pub status: CloudStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_files: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default = "default_source")]
    pub source: String, // "steam_cloud", "pcgamingwiki", "none"
}

fn default_source() -> String {
    "none".to_string()
}

/// Global cloud sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalCloudStatus {
    pub enabled: bool,
    pub is_syncing: bool,
    pub games_synced: u32,
    pub games_pending: u32,
    pub games_with_conflicts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<String>,
}

/// A file tracked by Steam Cloud (from remotecache.vdf)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudFile {
    pub path: String,         // Relative path as stored in VDF
    pub root: i32,            // Root type (0-12)
    pub size: u64,            // File size in bytes
    pub localtime: u64,       // Local modification timestamp
    pub remotetime: u64,      // Remote sync timestamp
    pub sha: String,          // SHA1 hash of file
    pub syncstate: i32,       // Sync status from Steam
    pub resolved_path: Option<PathBuf>, // Actual file system path
}

/// Result of a sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub files_uploaded: u32,
    pub files_downloaded: u32,
    pub conflicts: Vec<String>,
}

// ============================================================================
// VDF Parser for remotecache.vdf
// ============================================================================

/// Parse remotecache.vdf content and extract cloud files
pub fn parse_remotecache_vdf(content: &str) -> Result<HashMap<String, CloudFile>, String> {
    let mut files = HashMap::new();
    let mut current_file: Option<String> = None;
    let mut current_data: HashMap<String, String> = HashMap::new();
    let mut brace_depth = 0;
    let mut in_file_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track braces
        if trimmed == "{" {
            brace_depth += 1;
            if brace_depth == 2 && current_file.is_some() {
                in_file_block = true;
            }
            continue;
        }
        if trimmed == "}" {
            if in_file_block && brace_depth == 2 {
                // End of file block, save the file
                if let Some(ref file_path) = current_file {
                    if let Some(cloud_file) = build_cloud_file(file_path, &current_data) {
                        files.insert(file_path.clone(), cloud_file);
                    }
                }
                current_file = None;
                current_data.clear();
                in_file_block = false;
            }
            brace_depth -= 1;
            continue;
        }

        // Parse key-value pairs
        let parts: Vec<&str> = trimmed.split('"').filter(|s| !s.trim().is_empty()).collect();
        
        if parts.len() >= 2 {
            let key = parts[0];
            let value = parts[1];

            if brace_depth == 1 && !in_file_block {
                // This is a file path definition (not ChangeNumber or app_id)
                if key != "ChangeNumber" && !key.chars().all(|c| c.is_ascii_digit()) {
                    current_file = Some(key.to_string());
                }
            } else if in_file_block {
                // Store the key-value in current file data
                current_data.insert(key.to_string(), value.to_string());
            }
        }
    }

    Ok(files)
}

/// Build a CloudFile from parsed VDF data
fn build_cloud_file(path: &str, data: &HashMap<String, String>) -> Option<CloudFile> {
    Some(CloudFile {
        path: path.to_string(),
        root: data.get("root").and_then(|v| v.parse().ok()).unwrap_or(0),
        size: data.get("size").and_then(|v| v.parse().ok()).unwrap_or(0),
        localtime: data.get("localtime").and_then(|v| v.parse().ok()).unwrap_or(0),
        remotetime: data.get("remotetime").and_then(|v| v.parse().ok()).unwrap_or(0),
        sha: data.get("sha").cloned().unwrap_or_default(),
        syncstate: data.get("syncstate").and_then(|v| v.parse().ok()).unwrap_or(0),
        resolved_path: None,
    })
}

// ============================================================================
// Root Path Resolution
// ============================================================================

/// Resolve the actual file system path for a cloud file based on its root type
/// 
/// Root value mappings (from Steam documentation):
/// - 0: userdata/[user_id]/[app_id]/remote/
/// - 1: Game installation directory
/// - 2: ~/Documents/
/// - 3: ~/.config/ (Windows %appdata%)
/// - 4: ~/.local/share/ (Windows %localappdata%)
/// - 12: ~/.local/share/ (Windows %localappdata%/Low)
pub fn resolve_cloud_file_path(
    file: &CloudFile,
    app_id: &str,
    user_id: &str,
    game_install_path: Option<&str>,
) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    
    let base_path = match file.root {
        0 => {
            // Steam userdata remote folder
            let steam_path = if cfg!(target_os = "macos") {
                home.join("Library/Application Support/Steam")
            } else {
                home.join(".local/share/Steam")
            };
            Some(steam_path.join("userdata").join(user_id).join(app_id).join("remote"))
        }
        1 => {
            // Game installation directory
            game_install_path.map(PathBuf::from)
        }
        2 => {
            // Documents folder
            dirs::document_dir()
        }
        3 => {
            // Config/AppData Roaming
            if cfg!(target_os = "macos") {
                Some(home.join("Library/Application Support"))
            } else {
                dirs::config_dir()
            }
        }
        4 | 12 => {
            // Local data / LocalLow
            if cfg!(target_os = "macos") {
                Some(home.join("Library/Application Support"))
            } else {
                dirs::data_local_dir()
            }
        }
        _ => None,
    };

    base_path.map(|base| base.join(&file.path))
}

// ============================================================================
// WebDAV Client
// ============================================================================

/// Encode a file path for WebDAV storage (replace / with __)
pub fn encode_webdav_path(path: &str) -> String {
    path.replace('/', "__").replace('\\', "__")
}

/// Decode a WebDAV path back to original (replace __ with /)
pub fn decode_webdav_path(encoded: &str) -> String {
    encoded.replace("__", "/")
}

/// Build the full WebDAV URL for a file
pub fn build_webdav_url(base_url: &str, app_id: &str, file_path: &str) -> String {
    let encoded_path = encode_webdav_path(file_path);
    let base = base_url.trim_end_matches('/');
    format!("{}/boilerroom/{}/{}", base, app_id, encoded_path)
}

/// WebDAV client for cloud sync operations
pub struct WebDavClient {
    client: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl WebDavClient {
    pub fn new(config: &CloudSyncConfig) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            base_url: config.webdav_url.trim_end_matches('/').to_string(),
            username: config.username.clone(),
            password: config.password.clone(),
        })
    }

    /// Test connection to WebDAV server
    pub async fn test_connection(&self) -> Result<String, String> {
        let url = format!("{}/boilerroom/", self.base_url);
        
        // Try PROPFIND to check if we can access the directory
        let response = self.client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .send()
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;

        match response.status().as_u16() {
            200..=299 => Ok("Connection successful".to_string()),
            401 => Err("Authentication failed - check username/password".to_string()),
            403 => Err("Access forbidden - check permissions".to_string()),
            404 => {
                // Directory doesn't exist, try to create it
                self.create_directory("boilerroom").await?;
                Ok("Connection successful (created boilerroom directory)".to_string())
            }
            status => Err(format!("Server returned status {}", status)),
        }
    }

    /// Create a directory on WebDAV server
    pub async fn create_directory(&self, path: &str) -> Result<(), String> {
        let url = format!("{}/{}/", self.base_url, path);
        
        let response = self.client
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        match response.status().as_u16() {
            200..=299 | 405 => Ok(()), // 405 = already exists
            status => Err(format!("Failed to create directory: status {}", status)),
        }
    }

    /// Upload a file to WebDAV
    pub async fn upload_file(&self, app_id: &str, relative_path: &str, content: Vec<u8>) -> Result<(), String> {
        // Ensure app directory exists
        self.create_directory(&format!("boilerroom/{}", app_id)).await?;

        let url = build_webdav_url(&self.base_url, app_id, relative_path);
        
        let response = self.client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .body(content)
            .send()
            .await
            .map_err(|e| format!("Upload failed: {}", e))?;

        if response.status().is_success() || response.status().as_u16() == 201 {
            Ok(())
        } else {
            Err(format!("Upload failed: status {}", response.status()))
        }
    }

    /// Download a file from WebDAV
    pub async fn download_file(&self, app_id: &str, relative_path: &str) -> Result<Vec<u8>, String> {
        let url = build_webdav_url(&self.base_url, app_id, relative_path);
        
        let response = self.client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        if response.status().is_success() {
            response.bytes().await
                .map(|b| b.to_vec())
                .map_err(|e| format!("Failed to read response: {}", e))
        } else if response.status().as_u16() == 404 {
            Err("File not found on server".to_string())
        } else {
            Err(format!("Download failed: status {}", response.status()))
        }
    }

    /// Check if a file exists on WebDAV and get its modification time
    pub async fn get_file_info(&self, app_id: &str, relative_path: &str) -> Result<Option<u64>, String> {
        let url = build_webdav_url(&self.base_url, app_id, relative_path);
        
        let response = self.client
            .head(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("Failed to check file: {}", e))?;

        if response.status().is_success() {
            // Try to get Last-Modified header
            if let Some(last_modified) = response.headers().get("last-modified") {
                if let Ok(date_str) = last_modified.to_str() {
                    // Parse HTTP date format
                    if let Ok(datetime) = chrono::DateTime::parse_from_rfc2822(date_str) {
                        return Ok(Some(datetime.timestamp() as u64));
                    }
                }
            }
            Ok(Some(0)) // File exists but no timestamp
        } else if response.status().as_u16() == 404 {
            Ok(None) // File doesn't exist
        } else {
            Err(format!("Failed to check file: status {}", response.status()))
        }
    }

    /// List files in a directory on WebDAV
    pub async fn list_files(&self, app_id: &str) -> Result<Vec<String>, String> {
        let url = format!("{}/boilerroom/{}/", self.base_url, app_id);
        
        let response = self.client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "1")
            .send()
            .await
            .map_err(|e| format!("Failed to list files: {}", e))?;

        if !response.status().is_success() && response.status().as_u16() != 207 {
            if response.status().as_u16() == 404 {
                return Ok(Vec::new()); // Directory doesn't exist yet
            }
            return Err(format!("Failed to list files: status {}", response.status()));
        }

        let body = response.text().await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Simple XML parsing to extract file names from PROPFIND response
        let mut files = Vec::new();
        for line in body.lines() {
            if line.contains("<D:href>") || line.contains("<d:href>") {
                if let Some(start) = line.find('>') {
                    if let Some(end) = line[start + 1..].find('<') {
                        let href = &line[start + 1..start + 1 + end];
                        if let Some(filename) = href.split('/').last() {
                            if !filename.is_empty() && filename != app_id {
                                files.push(decode_webdav_path(filename));
                            }
                        }
                    }
                }
            }
        }

        Ok(files)
    }
}

// ============================================================================
// Sync Operations
// ============================================================================

/// Find all remotecache.vdf files for BoilerRoom games
#[allow(dead_code)]
pub fn find_remotecache_files(boilerroom_app_ids: &[String]) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return results,
    };

    let steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".local/share/Steam")
    };

    let userdata_path = steam_path.join("userdata");
    
    if !userdata_path.exists() {
        return results;
    }

    // Iterate through user directories
    if let Ok(user_dirs) = std::fs::read_dir(&userdata_path) {
        for user_entry in user_dirs.flatten() {
            if !user_entry.path().is_dir() {
                continue;
            }

            // Check each app_id directory
            for app_id in boilerroom_app_ids {
                let remotecache = user_entry.path()
                    .join(app_id)
                    .join("remotecache.vdf");
                
                if remotecache.exists() {
                    results.push((app_id.clone(), remotecache));
                }
            }
        }
    }

    results
}

/// Get the Steam user ID from userdata directory
pub fn get_steam_user_id() -> Option<String> {
    let home = dirs::home_dir()?;
    
    let steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".local/share/Steam")
    };

    let userdata_path = steam_path.join("userdata");
    
    if let Ok(mut dirs) = std::fs::read_dir(&userdata_path) {
        // Return the first user ID found (most users have only one)
        if let Some(Ok(entry)) = dirs.next() {
            if entry.path().is_dir() {
                return entry.file_name().into_string().ok();
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_remotecache_vdf() {
        let vdf_content = r#"
"730"
{
    "ChangeNumber" "0"
    "cfg/csgo_saved_item_shuffles.txt"
    {
        "root" "0"
        "size" "24"
        "localtime" "1522861973"
        "time" "1522861973"
        "remotetime" "1522861973"
        "sha" "c7bdc563982fc4ddddb9e6f8853b298967d858f9"
        "syncstate" "1"
        "persiststate" "0"
        "platformstosync2" "-1"
    }
}
"#;
        let files = parse_remotecache_vdf(vdf_content).unwrap();
        assert_eq!(files.len(), 1);
        
        let file = files.get("cfg/csgo_saved_item_shuffles.txt").unwrap();
        assert_eq!(file.root, 0);
        assert_eq!(file.size, 24);
        assert_eq!(file.localtime, 1522861973);
        assert_eq!(file.sha, "c7bdc563982fc4ddddb9e6f8853b298967d858f9");
    }

    #[test]
    fn test_encode_decode_webdav_path() {
        let path = "saves/slot1/game.sav";
        let encoded = encode_webdav_path(path);
        assert_eq!(encoded, "saves__slot1__game.sav");
        
        let decoded = decode_webdav_path(&encoded);
        assert_eq!(decoded, path);
    }

    #[test]
    fn test_build_webdav_url() {
        let url = build_webdav_url(
            "https://cloud.example.com/remote.php/dav/files/user",
            "730",
            "saves/game.sav"
        );
        assert_eq!(
            url,
            "https://cloud.example.com/remote.php/dav/files/user/boilerroom/730/saves__game.sav"
        );
    }
}
