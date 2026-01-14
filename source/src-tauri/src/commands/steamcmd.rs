//! SteamCMD integration commands

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotSteamInfo {
    pub name: Option<String>,
    pub oslist: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSteamInfo {
    pub app_id: String,
    pub name: Option<String>,
    pub oslist: Option<String>,
    pub installdir: Option<String>,
    pub depots: HashMap<String, DepotSteamInfo>,
}

/// Get app/depot info from SteamCMD (optional, fails gracefully)
#[tauri::command]
pub async fn steamcmd_get_app_info(app_id: String) -> Result<AppSteamInfo, String> {
    let output = Command::new("steamcmd")
        .args([
            "+login",
            "anonymous",
            "+app_info_update",
            "1",
            "+app_info_print",
            &app_id,
            "+quit",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[SteamCMD] Not available: {}", e);
            return Err(format!("SteamCMD not available: {}", e));
        }
    };

    if !output.status.success() {
        return Err("SteamCMD failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_steamcmd_output(&app_id, &stdout)
}

fn parse_steamcmd_output(app_id: &str, output: &str) -> Result<AppSteamInfo, String> {
    let mut info = AppSteamInfo {
        app_id: app_id.to_string(),
        name: None,
        oslist: None,
        installdir: None,
        depots: HashMap::new(),
    };

    let app_marker = format!("\"{}\"", app_id);
    let start_idx = match output.find(&app_marker) {
        Some(idx) => idx,
        None => return Err("App info not found in output".to_string()),
    };

    let app_section = &output[start_idx..];

    for line in app_section.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = parse_vdf_line(trimmed) {
            match key.as_str() {
                "name" if info.name.is_none() => info.name = Some(value),
                "oslist" if info.oslist.is_none() => info.oslist = Some(value),
                "installdir" if info.installdir.is_none() => info.installdir = Some(value),
                _ => {}
            }
        }
    }

    let mut in_depots = false;
    let mut current_depot_id: Option<String> = None;
    let mut brace_depth = 0;

    for line in app_section.lines() {
        let trimmed = line.trim();

        if trimmed.contains("\"depots\"") {
            in_depots = true;
            continue;
        }

        if !in_depots {
            continue;
        }

        if trimmed == "{" {
            brace_depth += 1;
            continue;
        }
        if trimmed == "}" {
            brace_depth -= 1;
            if brace_depth == 0 {
                in_depots = false;
                current_depot_id = None;
            } else if brace_depth == 1 {
                current_depot_id = None;
            }
            continue;
        }

        if brace_depth == 1 {
            if let Some(depot_id) = parse_depot_id(trimmed) {
                current_depot_id = Some(depot_id.clone());
                info.depots.insert(
                    depot_id,
                    DepotSteamInfo {
                        name: None,
                        oslist: None,
                        size: None,
                    },
                );
            }
        }

        if brace_depth >= 2 {
            if let Some(ref depot_id) = current_depot_id {
                if let Some((key, value)) = parse_vdf_line(trimmed) {
                    if let Some(depot) = info.depots.get_mut(depot_id) {
                        match key.as_str() {
                            "name" => depot.name = Some(value),
                            "oslist" => depot.oslist = Some(value),
                            "size" | "download" => {
                                if let Ok(size) = value.parse::<u64>() {
                                    depot.size = Some(size);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "[SteamCMD] Parsed {} depots for app {}",
        info.depots.len(),
        app_id
    );

    Ok(info)
}

fn parse_vdf_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if !line.starts_with('"') {
        return None;
    }

    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 {
        let key = parts[1].to_string();
        let value = parts[3].to_string();
        return Some((key, value));
    }

    None
}

fn parse_depot_id(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with('"') {
        return None;
    }

    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 2 {
        let id = parts[1];
        if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() {
            return Some(id.to_string());
        }
    }

    None
}
