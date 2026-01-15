//! Update checking commands - Check for app updates from GitHub releases

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Update information returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
}

/// GitHub release API response structure
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// Parse version string (e.g., "v1.5.0" or "1.5.0") into comparable tuple
fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    let version = version.trim_start_matches('v');
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some((major, minor, patch))
    } else if parts.len() == 2 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        Some((major, minor, 0))
    } else {
        None
    }
}

/// Compare two versions, returns true if latest > current
fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(curr), Some(lat)) => lat > curr,
        _ => false,
    }
}

/// Check for application updates from GitHub releases
#[tauri::command]
pub async fn check_for_update(app_handle: tauri::AppHandle) -> Result<UpdateInfo, String> {
    // Get current version from package info
    let current_version = app_handle.package_info().version.to_string();

    eprintln!(
        "[Update] Checking for updates, current version: {}",
        current_version
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("BoilerRoom-UpdateChecker")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch releases from GitHub API (includes prereleases)
    let response = client
        .get("https://api.github.com/repos/uxandai/boilerroom/releases")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }

    let releases: Vec<GitHubRelease> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse releases: {}", e))?;

    // Get the latest release (first in the list)
    let latest = releases
        .first()
        .ok_or_else(|| "No releases found".to_string())?;

    let latest_version = latest.tag_name.trim_start_matches('v').to_string();
    let update_available = is_newer_version(&current_version, &latest.tag_name);

    eprintln!(
        "[Update] Latest version: {}, update available: {}",
        latest_version, update_available
    );

    Ok(UpdateInfo {
        update_available,
        current_version,
        latest_version,
        release_url: latest.html_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.5.0"), Some((1, 5, 0)));
        assert_eq!(parse_version("v1.5.0"), Some((1, 5, 0)));
        assert_eq!(parse_version("v2.0.1"), Some((2, 0, 1)));
        assert_eq!(parse_version("1.0"), Some((1, 0, 0)));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("1.5.0", "1.6.0"));
        assert!(is_newer_version("1.5.0", "v1.5.1"));
        assert!(is_newer_version("1.5.0", "2.0.0"));
        assert!(!is_newer_version("1.5.0", "1.5.0"));
        assert!(!is_newer_version("1.5.0", "1.4.0"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
    }
}
