//! Transfer commands - rsync game copy to remote Steam Deck

use super::connection::SshConfig;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Global flags for copy cancellation
static COPY_CANCELLED: AtomicBool = AtomicBool::new(false);
static COPY_PROCESS_PID: AtomicU32 = AtomicU32::new(0);

/// Copy a locally installed game to a remote Steam Deck via rsync
#[tauri::command]
pub async fn copy_game_to_remote(
    app: tauri::AppHandle,
    config: SshConfig,
    local_path: String,
    remote_path: String,
    app_id: String,
    game_name: String,
) -> Result<(), String> {
    use std::io::BufReader;

    let local_path_buf = PathBuf::from(&local_path);
    let folder_name = local_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&app_id)
        .to_string();

    let remote_game_path = format!("{}/{}", remote_path, folder_name);
    let dst_path = format!("{}@{}:{}", config.username, config.ip, remote_game_path);
    let src_path = format!("{}/", local_path);

    eprintln!("[copy_game_to_remote] Starting copy: {} -> {}", src_path, dst_path);

    let has_sshpass = !config.password.is_empty()
        && Command::new("which")
            .arg("sshpass")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    let rsync_path = if std::path::Path::new("/opt/homebrew/bin/rsync").exists() {
        "/opt/homebrew/bin/rsync"
    } else if std::path::Path::new("/usr/local/bin/rsync").exists() {
        "/usr/local/bin/rsync"
    } else {
        "rsync"
    };

    let mut cmd = Command::new(rsync_path);
    cmd.args(["-avzs", "-i", "--progress", "--partial", "--no-inc-recursive"]);

    if has_sshpass {
        let ssh_cmd = format!(
            "sshpass -e ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        cmd.env("SSHPASS", &config.password);
        cmd.args(["-e", &ssh_cmd]);
    } else if !config.private_key_path.is_empty() {
        let ssh_cmd = format!(
            "ssh -p {} -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port, config.private_key_path
        );
        cmd.args(["-e", &ssh_cmd]);
    } else {
        let ssh_cmd = format!(
            "ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
            config.port
        );
        cmd.args(["-e", &ssh_cmd]);
    }

    cmd.args([&src_path, &dst_path]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let file_count: usize = walkdir::WalkDir::new(&local_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count();

    let total_bytes: u64 = walkdir::WalkDir::new(&local_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();

    let _ = app.emit("install-progress", serde_json::json!({
        "state": "transferring",
        "message": format!("Starting: {} files, {:.2} GB", file_count, total_bytes as f64 / 1_073_741_824.0),
        "download_percent": 0.0,
        "bytes_total": total_bytes
    }));

    COPY_CANCELLED.store(false, Ordering::SeqCst);

    let mut child = cmd.spawn().map_err(|e| format!("Failed to start rsync: {}", e))?;
    COPY_PROCESS_PID.store(child.id(), Ordering::SeqCst);

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let app_clone = app.clone();
        let file_count_clone = file_count;
        let total_bytes_clone = total_bytes;

        std::thread::spawn(move || {
            let mut stdout = reader.into_inner();
            let mut buffer = [0u8; 1];
            let mut line = String::new();
            let mut files_done: usize = 0;
            let emit_interval = Duration::from_secs(10);
            let mut last_emit_time = Instant::now() - emit_interval;

            while let Ok(1) = stdout.read(&mut buffer) {
                let ch = buffer[0] as char;
                if ch == '\r' || ch == '\n' {
                    if !line.is_empty() {
                        let trimmed = line.trim();
                        if (trimmed.starts_with(">f") || trimmed.starts_with("<f") || trimmed.starts_with("cf")) && trimmed.len() > 12 {
                            files_done += 1;
                            if last_emit_time.elapsed() >= emit_interval {
                                last_emit_time = Instant::now();
                                let percent = if file_count_clone > 0 {
                                    (files_done as f64 / file_count_clone as f64) * 100.0
                                } else { 0.0 };
                                let _ = app_clone.emit("install-progress", serde_json::json!({
                                    "state": "transferring",
                                    "message": format!("Copying: {}/{}", files_done, file_count_clone),
                                    "download_percent": percent,
                                    "files_transferred": files_done,
                                    "files_total": file_count_clone,
                                    "bytes_total": total_bytes_clone
                                }));
                            }
                        }
                        line.clear();
                    }
                } else {
                    line.push(ch);
                }
            }

            let _ = app_clone.emit("install-progress", serde_json::json!({
                "state": "transferring",
                "message": format!("Transfer complete: {}/{} files", files_done, file_count_clone),
                "download_percent": 100.0
            }));
        });
    }

    let status = child.wait().map_err(|e| format!("rsync wait failed: {}", e))?;

    if !status.success() {
        let exit_code = status.code().unwrap_or(-1);
        let error_msg = match exit_code {
            255 => format!("SSH connection failed. Check IP ({}), SSH enabled, password correct.", config.ip),
            _ => format!("rsync failed with exit code {}", exit_code),
        };
        let _ = app.emit("install-progress", serde_json::json!({
            "state": "error",
            "message": error_msg
        }));
        return Err(error_msg);
    }

    // Update SLSsteam config on remote
    let _ = app.emit("install-progress", serde_json::json!({
        "state": "configuring",
        "message": "Updating SLSsteam config..."
    }));

    let addr = format!("{}:{}", config.ip, config.port);
    if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
        if let Ok(mut sess) = ssh2::Session::new() {
            sess.set_tcp_stream(tcp);
            if sess.handshake().is_ok() && sess.userauth_password(&config.username, &config.password).is_ok() {
                let mut content = String::new();
                if let Ok(mut channel) = sess.channel_session() {
                    if channel.exec("cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''").is_ok() {
                        let _ = channel.read_to_string(&mut content);
                        let _ = channel.wait_close();
                    }
                }

                let new_config = crate::install_manager::add_app_to_config_yaml(&content, &app_id, &game_name);
                if let Ok(mut channel) = sess.channel_session() {
                    if channel.exec("mkdir -p ~/.config/SLSsteam && cat > ~/.config/SLSsteam/config.yaml").is_ok() {
                        let _ = channel.write_all(new_config.as_bytes());
                        let _ = channel.send_eof();
                        let _ = channel.wait_close();
                    }
                }

                // Create ACF manifest
                let steamapps_dir = remote_path.trim_end_matches('/').trim_end_matches("/common");
                let acf_path = format!("{}/appmanifest_{}.acf", steamapps_dir, app_id);
                let acf_content = format!(
                    r#""AppState"
{{
	"appid"		"{app_id}"
	"Universe"		"1"
	"name"		"{game_name}"
	"StateFlags"		"4"
	"installdir"		"{folder_name}"
	"UserConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
}}"#,
                    app_id = app_id, game_name = game_name, folder_name = folder_name
                );

                if let Ok(mut channel) = sess.channel_session() {
                    if channel.exec(&format!("cat > \"{}\"", acf_path)).is_ok() {
                        let _ = channel.write_all(acf_content.as_bytes());
                        let _ = channel.send_eof();
                        let _ = channel.wait_close();
                    }
                }
            }
        }
    }

    let _ = app.emit("install-progress", serde_json::json!({
        "state": "finished",
        "message": format!("{} copied successfully!", game_name),
        "download_percent": 100.0
    }));

    Ok(())
}

/// Cancel an ongoing copy_game_to_remote operation
#[tauri::command]
pub async fn cancel_copy_to_remote(app: tauri::AppHandle) -> Result<(), String> {
    eprintln!("[cancel_copy_to_remote] Cancel requested");
    COPY_CANCELLED.store(true, Ordering::SeqCst);
    let pid = COPY_PROCESS_PID.load(Ordering::SeqCst);

    if pid > 0 {
        eprintln!("[cancel_copy_to_remote] Killing rsync process PID: {}", pid);
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
            std::thread::sleep(Duration::from_millis(500));
            let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).status();
        }
        COPY_PROCESS_PID.store(0, Ordering::SeqCst);
    }

    let _ = app.emit("install-progress", serde_json::json!({
        "state": "cancelled",
        "message": "Copy cancelled by user"
    }));

    Ok(())
}
