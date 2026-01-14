//! Depot keys only install command - configures Steam without downloading

use super::connection::SshConfig;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

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
    target_library: String,
    trigger_steam_install: bool,
) -> Result<String, String> {
    use crate::config_vdf;

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

    let depot_keys: Vec<(String, String)> = depots
        .iter()
        .filter(|d| !d.key.is_empty())
        .map(|d| (d.depot_id.clone(), d.key.clone()))
        .collect();

    let steam_root = target_library
        .trim_end_matches('/')
        .trim_end_matches("/steamapps/common");
    let steam_root = steam_root.trim_end_matches("/steamapps");
    let config_vdf_path = format!("{}/config/config.vdf", steam_root);
    let depotcache_path = format!("{}/depotcache", steam_root);
    let steamapps_path = format!("{}/steamapps", steam_root);

    if ssh_config.is_local {
        // ====== LOCAL MODE ======
        let config_vdf_expanded = shellexpand::tilde(&config_vdf_path).to_string();

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

        let acf_content = build_acf_state_flags_6(&app_id, &game_name);
        let acf_path =
            shellexpand::tilde(&format!("{}/appmanifest_{}.acf", steamapps_path, app_id))
                .to_string();
        std::fs::write(&acf_path, &acf_content)
            .map_err(|e| format!("Failed to write ACF: {}", e))?;
        eprintln!("[DepotKeysOnly] Created ACF: {}", acf_path);

        let slssteam_config = shellexpand::tilde("~/.config/SLSsteam/config.yaml").to_string();
        if std::path::Path::new(&slssteam_config).exists() {
            if let Ok(content) = std::fs::read_to_string(&slssteam_config) {
                let new_config =
                    crate::install_manager::add_app_to_config_yaml(&content, &app_id, &game_name);
                let _ = std::fs::write(&slssteam_config, &new_config);
                eprintln!("[DepotKeysOnly] Updated SLSsteam config");
            }
        }

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

        // Add decryption keys
        if !depot_keys.is_empty() {
            let mut config_content = String::new();
            if let Ok(mut channel) = sess.channel_session() {
                let cmd = format!("cat \"{}\" 2>/dev/null || echo ''", config_vdf_path);
                if channel.exec(&cmd).is_ok() {
                    let _ = channel.read_to_string(&mut config_content);
                    let _ = channel.wait_close();
                }
            }

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
                }
            }
        }

        // Create depotcache directory
        if let Ok(mut channel) = sess.channel_session() {
            let cmd = format!("mkdir -p \"{}\"", depotcache_path);
            let _ = channel.exec(&cmd);
            let _ = channel.wait_close();
        }

        // Copy manifest files via SFTP
        for depot in &depots {
            if !depot.manifest_path.is_empty()
                && std::path::Path::new(&depot.manifest_path).exists()
            {
                let manifest_filename =
                    format!("{}_{}.manifest", depot.depot_id, depot.manifest_id);
                let remote_path = format!("{}/{}", depotcache_path, manifest_filename);

                if let Ok(content) = std::fs::read(&depot.manifest_path) {
                    if let Ok(sftp) = sess.sftp() {
                        if let Ok(mut remote_file) = sftp.create(Path::new(&remote_path)) {
                            let _ = remote_file.write_all(&content);
                        }
                    }
                }
            }
        }

        // Create ACF
        let acf_content = build_acf_state_flags_6(&app_id, &game_name);
        let acf_remote_path = format!("{}/appmanifest_{}.acf", steamapps_path, app_id);

        if let Ok(mut channel) = sess.channel_session() {
            let cmd = format!("cat > \"{}\"", acf_remote_path);
            if channel.exec(&cmd).is_ok() {
                let _ = channel.write_all(acf_content.as_bytes());
                let _ = channel.send_eof();
                let _ = channel.wait_close();
            }
        }

        // Update SLSsteam config
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
            }
        }

        // Trigger steam://install on remote
        if trigger_steam_install {
            let steam_url = format!("steam://install/{}", app_id);
            if let Ok(mut channel) = sess.channel_session() {
                let cmd = format!(
                    "DISPLAY=:0 xdg-open '{}' 2>/dev/null || DISPLAY=:1 xdg-open '{}' 2>/dev/null || echo 'xdg-open failed'",
                    steam_url, steam_url
                );
                let _ = channel.exec(&cmd);
                let _ = channel.wait_close();
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
