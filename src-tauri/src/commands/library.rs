//! Library management commands - List, uninstall, and manage installed games

use super::connection::SshConfig;
use super::slssteam::ssh_exec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledGame {
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub has_depotdownloader_marker: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledDepot {
    pub depot_id: String,
    pub manifest_id: String,
}

/// Helper to extract library paths from VDF content
fn extract_library_paths_from_vdf(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut tokens = Vec::new();
    let mut in_quote = false;
    let mut current_token = String::new();

    for c in content.chars() {
        if c == '"' {
            if in_quote {
                tokens.push(current_token.clone());
                current_token.clear();
                in_quote = false;
            } else {
                in_quote = true;
            }
        } else if in_quote {
            current_token.push(c);
        }
    }

    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "path" && i + 1 < tokens.len() {
            let path = &tokens[i + 1];
            if !path.is_empty() {
                paths.push(path.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    paths
}

/// Helper function to parse Steam library paths from libraryfolders.vdf
fn get_steam_library_paths(sess: &ssh2::Session) -> Result<Vec<String>, String> {
    use std::collections::HashSet;
    let mut libraries_set: HashSet<String> = HashSet::new();

    let real_path_out = ssh_exec(sess, "readlink -f ~/.steam/steam 2>/dev/null || echo '/home/deck/.steam/steam'")?;
    let primary_path = real_path_out.trim().to_string();
    libraries_set.insert(primary_path.clone());

    let vdf_content = ssh_exec(sess, "cat ~/.steam/steam/steamapps/libraryfolders.vdf 2>/dev/null || echo ''")?;
    for path in extract_library_paths_from_vdf(&vdf_content) {
        libraries_set.insert(path);
    }

    Ok(libraries_set.into_iter().collect())
}

#[tauri::command]
pub async fn list_installed_games(config: SshConfig) -> Result<Vec<InstalledGame>, String> {
    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new()
        .map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let mut games = Vec::new();
    let libraries = get_steam_library_paths(&sess)?;

    for library in libraries {
        let steamapps_path = format!("{}/steamapps", library);
        let common_path = format!("{}/common", steamapps_path);

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
        let mut installdir_to_appid: HashMap<String, String> = HashMap::new();
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

        let list_cmd = format!("ls -1 '{}' 2>/dev/null || echo ''", common_path);
        let output = ssh_exec(&sess, &list_cmd)?;

        for line in output.lines() {
            let name = line.trim();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }

            let game_path = format!("{}/{}", common_path, name);

            let marker_cmd = format!("test -d '{}/.DepotDownloader' && echo 'YES' || echo 'NO'", game_path);
            let marker_out = ssh_exec(&sess, &marker_cmd)?;
            let has_depotdownloader_marker = marker_out.trim() == "YES";

            let size_cmd = format!("du -sb '{}' 2>/dev/null | cut -f1 || echo '0'", game_path);
            let size_out = ssh_exec(&sess, &size_cmd)?;
            let size_bytes: u64 = size_out.trim().parse().unwrap_or(0);

            let app_id = installdir_to_appid.get(name).cloned().unwrap_or_else(|| "unknown".to_string());

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

#[tauri::command]
pub async fn list_installed_games_local() -> Result<Vec<InstalledGame>, String> {
    use std::collections::HashSet;
    use std::fs;
    use walkdir::WalkDir;

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let mut games = Vec::new();

    let primary_steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".steam/steam")
    };

    let mut library_paths_set: HashSet<PathBuf> = HashSet::new();

    if let Ok(canonical) = std::fs::canonicalize(&primary_steam_path) {
        library_paths_set.insert(canonical.join("steamapps"));
    } else if primary_steam_path.exists() {
        library_paths_set.insert(primary_steam_path.join("steamapps"));
    }

    let vdf_path = primary_steam_path.join("steamapps/libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(&vdf_path) {
        for path_str in extract_library_paths_from_vdf(&content) {
            let p = Path::new(&path_str);
            let steamapps = if let Ok(canonical) = std::fs::canonicalize(p) {
                canonical.join("steamapps")
            } else {
                p.join("steamapps")
            };
            if steamapps.exists() {
                library_paths_set.insert(steamapps);
            }
        }
    }

    let library_paths: Vec<PathBuf> = library_paths_set.into_iter().collect();

    for steamapps in library_paths {
        if !steamapps.exists() {
            continue;
        }

        let mut installdir_to_appid: HashMap<String, String> = HashMap::new();

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

                let marker_path = path.join(".DepotDownloader");
                let has_depotdownloader_marker = marker_path.exists();

                let mut size_bytes: u64 = 0;
                for entry in WalkDir::new(&path).into_iter().flatten() {
                    if entry.file_type().is_file() {
                        if let Ok(meta) = entry.metadata() {
                            size_bytes += meta.len();
                        }
                    }
                }

                let app_id = installdir_to_appid.get(&name).cloned().unwrap_or_else(|| "unknown".to_string());

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

    Ok(games)
}

#[tauri::command]
pub async fn check_game_installed(config: SshConfig, app_id: String) -> Result<Vec<InstalledDepot>, String> {
    if config.is_local {
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

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new()
        .map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let libraries = get_steam_library_paths(&sess)?;
    let mut installed_depots: Vec<InstalledDepot> = Vec::new();

    for lib in libraries {
        let common_path = format!("{}/steamapps/common", lib);
        let find_cmd = format!(
            "find '{}' -maxdepth 2 -type d -name '.DepotDownloader' 2>/dev/null | while read dir; do ls -1 \"$dir\" 2>/dev/null; done",
            common_path
        );
        let output = ssh_exec(&sess, &find_cmd)?;

        for line in output.lines() {
            let name = line.trim();
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

    Ok(installed_depots)
}

#[tauri::command]
pub async fn get_steam_libraries(config: SshConfig) -> Result<Vec<String>, String> {
    use std::collections::HashSet;

    if config.is_local {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let mut paths_set: HashSet<String> = HashSet::new();

        let primary_steam_path = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Steam")
        } else {
            home.join(".steam/steam")
        };

        if primary_steam_path.exists() {
            let canonical_primary = std::fs::canonicalize(&primary_steam_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| primary_steam_path.to_string_lossy().to_string());
            paths_set.insert(canonical_primary);

            let vdf_path = primary_steam_path.join("steamapps/libraryfolders.vdf");
            if let Ok(content) = std::fs::read_to_string(&vdf_path) {
                let vdf_paths = extract_library_paths_from_vdf(&content);
                for path_str in vdf_paths {
                    let p = Path::new(&path_str);
                    let canonical = std::fs::canonicalize(p)
                        .map(|c| c.to_string_lossy().to_string())
                        .unwrap_or(path_str);
                    paths_set.insert(canonical);
                }
            }
        }

        let mut paths: Vec<String> = paths_set.into_iter().collect();
        paths.sort();
        return Ok(paths);
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new()
        .map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    get_steam_library_paths(&sess)
}

#[tauri::command]
pub async fn uninstall_game(config: SshConfig, game_path: String, app_id: String) -> Result<String, String> {
    if config.is_local {
        let game_dir = PathBuf::from(&game_path);
        if game_dir.exists() {
            std::fs::remove_dir_all(&game_dir)
                .map_err(|e| format!("Failed to remove directory: {}", e))?;
        }

        if let Some(common_dir) = game_dir.parent() {
            if let Some(steamapps_dir) = common_dir.parent() {
                let acf_path = steamapps_dir.join(format!("appmanifest_{}.acf", app_id));
                if acf_path.exists() {
                    std::fs::remove_file(&acf_path).ok();
                }
            }
        }

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

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new()
        .map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let rm_cmd = format!("rm -rf '{}'", game_path);
    ssh_exec(&sess, &rm_cmd)?;

    let acf_cmd = format!("rm -f \"$(dirname '{}')/../appmanifest_{}.acf\"", game_path, app_id);
    ssh_exec(&sess, &acf_cmd)?;

    let config_path = "/home/deck/.config/SLSsteam/config.yaml";
    let sftp = sess.sftp()
        .map_err(|e| format!("SFTP error: {}", e))?;

    let config_content = match sftp.open(Path::new(config_path)) {
        Ok(mut f) => {
            let mut buf = String::new();
            let _ = f.read_to_string(&mut buf);
            buf
        }
        Err(_) => String::new(),
    };

    let new_content = remove_app_from_config(&config_content, &app_id)?;

    let mut remote_f = sftp.create(Path::new(config_path))
        .map_err(|e| format!("Failed to create remote file: {}", e))?;
    remote_f.write_all(new_content.as_bytes())
        .map_err(|e| format!("Failed to write to remote file: {}", e))?;

    Ok(format!("Uninstalled game at {} (removed ACF and config)", game_path))
}

fn remove_app_from_config(content: &str, app_id: &str) -> Result<String, String> {
    if content.is_empty() {
        return Ok(String::new());
    }

    let mut doc: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;

    if let Some(mapping) = doc.as_mapping_mut() {
        if let Some(additional_apps) = mapping.get_mut(&serde_yaml::Value::String("AdditionalApps".to_string())) {
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

#[tauri::command]
pub async fn check_game_update(app_id: String, app_handle: tauri::AppHandle) -> Result<bool, String> {
    let api_key = super::settings::get_api_key(app_handle.clone()).await?;
    let url = format!("https://morrenus.martylek.com/api/bundles/search?query={}&key={}", app_id, api_key);

    let response = reqwest::get(&url).await.map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        return Err("API returned error".to_string());
    }

    // TODO: Implement proper manifest comparison
    Ok(false)
}

use std::io::Write;
