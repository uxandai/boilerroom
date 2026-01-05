//! Installation orchestration and deployment commands

use crate::install_manager::InstallManager;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;
use tauri::State;

use super::connection::SshConfig;

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

/// Cleanup cancelled installation - delete partial files and ACF
#[tauri::command]
pub async fn cleanup_cancelled_install(
    app_id: String,
    game_name: String,
    library_path: String,
    ssh_config: SshConfig,
) -> Result<String, String> {
    use std::path::PathBuf;

    let mut deleted_items = Vec::new();

    let steamapps_path = PathBuf::from(&library_path).join("steamapps");
    let game_folder = steamapps_path.join("common").join(&game_name);
    let acf_file = steamapps_path.join(format!("appmanifest_{}.acf", app_id));

    if ssh_config.is_local {
        // LOCAL: Delete directly
        if game_folder.exists() {
            match std::fs::remove_dir_all(&game_folder) {
                Ok(_) => {
                    eprintln!("[Cleanup] Deleted game folder: {:?}", game_folder);
                    deleted_items.push(format!("Game folder: {}", game_folder.display()));
                }
                Err(e) => {
                    eprintln!("[Cleanup] Failed to delete game folder: {}", e);
                }
            }
        }

        if acf_file.exists() {
            match std::fs::remove_file(&acf_file) {
                Ok(_) => {
                    eprintln!("[Cleanup] Deleted ACF: {:?}", acf_file);
                    deleted_items.push(format!("ACF manifest: appmanifest_{}.acf", app_id));
                }
                Err(e) => {
                    eprintln!("[Cleanup] Failed to delete ACF: {}", e);
                }
            }
        }
    } else {
        // REMOTE: Delete via SSH
        let remote_steamapps = format!("{}/steamapps", library_path.trim_end_matches('/'));
        let remote_game_folder = format!("{}/common/{}", remote_steamapps, game_name);
        let remote_acf = format!("{}/appmanifest_{}.acf", remote_steamapps, app_id);

        let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
        match TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
            Ok(tcp) => {
                if let Ok(mut sess) = ssh2::Session::new() {
                    sess.set_tcp_stream(tcp);
                    if sess.handshake().is_ok()
                        && sess
                            .userauth_password(&ssh_config.username, &ssh_config.password)
                            .is_ok()
                    {
                        // Delete game folder
                        let cmd = format!("rm -rf \"{}\"", remote_game_folder);
                        if let Ok(mut channel) = sess.channel_session() {
                            if channel.exec(&cmd).is_ok() {
                                let _ = channel.wait_close();
                                if channel.exit_status().unwrap_or(-1) == 0 {
                                    deleted_items
                                        .push(format!("Remote game folder: {}", game_name));
                                }
                            }
                        }

                        // Delete ACF
                        let cmd = format!("rm -f \"{}\"", remote_acf);
                        if let Ok(mut channel) = sess.channel_session() {
                            if channel.exec(&cmd).is_ok() {
                                let _ = channel.wait_close();
                                if channel.exit_status().unwrap_or(-1) == 0 {
                                    deleted_items
                                        .push(format!("Remote ACF: appmanifest_{}.acf", app_id));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to connect for cleanup: {}", e));
            }
        }
    }

    if deleted_items.is_empty() {
        Ok("No files found to clean up".to_string())
    } else {
        Ok(format!("Cleaned up: {}", deleted_items.join(", ")))
    }
}

#[tauri::command]
pub async fn upload_to_deck(
    config: SshConfig,
    local_path: String,
    remote_path: String,
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

    let local_data =
        std::fs::read(&local_path).map_err(|e| format!("Failed to read local file: {}", e))?;

    let remote_path_obj = Path::new(&remote_path);
    if let Some(parent) = remote_path_obj.parent() {
        let mkdir_cmd = format!("mkdir -p {}", parent.display());
        let mut channel = sess
            .channel_session()
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        channel.exec(&mkdir_cmd).ok();
        channel.wait_close().ok();
    }

    let mut remote_file = sftp
        .create(Path::new(&remote_path))
        .map_err(|e| format!("Failed to create remote file: {}", e))?;

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

    let config_content = match sftp.open(Path::new(config_path)) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content).ok();
            content
        }
        Err(_) => String::new(),
    };

    let backup_path = "/home/deck/.config/SLSsteam/config.yaml.bak";
    let backup_cmd = format!("cp {} {} 2>/dev/null || true", config_path, backup_path);
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel.exec(&backup_cmd).ok();
    channel.wait_close().ok();

    let new_content =
        crate::install_manager::add_app_to_config_yaml(&config_content, &app_id, &game_name);

    let mkdir_cmd = "mkdir -p /home/deck/.config/SLSsteam";
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel.exec(mkdir_cmd).ok();
    channel.wait_close().ok();

    let mut remote_file = sftp
        .create(Path::new(config_path))
        .map_err(|e| format!("Failed to create config file: {}", e))?;

    remote_file
        .write_all(new_content.as_bytes())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}
