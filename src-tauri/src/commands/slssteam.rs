//! SLSsteam installation and verification commands

use super::connection::SshConfig;
use super::settings::get_slssteam_cache_dir;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

/// Status of SLSsteam installation components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlssteamStatus {
    pub is_readonly: bool,
    pub slssteam_so_exists: bool,
    pub library_inject_so_exists: bool,
    pub config_exists: bool,
    pub config_play_not_owned: bool,
    pub config_safe_mode_on: bool,
    pub steam_jupiter_patched: bool,
    pub desktop_entry_patched: bool,
    pub additional_apps_count: usize,
}

/// Status of SLSsteam installation components (local version - simpler)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlssteamLocalStatus {
    pub slssteam_so_exists: bool,
    pub library_inject_so_exists: bool,
    pub config_exists: bool,
    pub config_play_not_owned: bool,
    pub additional_apps_count: usize,
    pub desktop_entry_patched: bool,
}

/// Detect if running on Steam Deck / SteamOS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamDeckDetection {
    pub is_steam_deck: bool,
    pub is_steamos: bool,
    pub os_name: String,
}

/// Helper function to execute SSH command and return output
pub fn ssh_exec(sess: &ssh2::Session, cmd: &str) -> Result<String, String> {
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec(cmd)
        .map_err(|e| format!("Failed to exec: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output).ok();
    channel.wait_close().ok();

    Ok(output)
}

/// Verify SLSsteam installation status on Steam Deck
#[tauri::command]
pub async fn verify_slssteam(config: SshConfig) -> Result<SlssteamStatus, String> {
    if config.is_local {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;

        let slssteam_so_path = home.join(".local/share/SLSsteam/SLSsteam.so");
        let slssteam_so_exists = slssteam_so_path.exists();

        let library_inject_path = home.join(".local/share/SLSsteam/library-inject.so");
        let library_inject_so_exists = library_inject_path.exists();

        let config_path = home.join(".config/SLSsteam/config.yaml");
        let config_exists = config_path.exists();

        let (config_play_not_owned, config_safe_mode_on, additional_apps_count) = if config_exists {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                let play_not_owned = content.contains("PlayNotOwnedGames: true")
                    || content.contains("PlayNotOwnedGames: yes");
                let safe_mode_on =
                    content.contains("SafeMode: true") || content.contains("SafeMode: yes");
                let apps_count = content.matches("- ").count();
                (play_not_owned, safe_mode_on, apps_count)
            } else {
                (false, false, 0)
            }
        } else {
            (false, false, 0)
        };

        let desktop_path = home.join(".local/share/applications/steam.desktop");
        let desktop_entry_patched = if desktop_path.exists() {
            std::fs::read_to_string(&desktop_path)
                .map(|c| c.contains("LD_AUDIT"))
                .unwrap_or(false)
        } else {
            false
        };

        let jupiter_path = Path::new("/usr/bin/steam-jupiter");
        let steam_jupiter_patched = if jupiter_path.exists() {
            std::fs::read_to_string(jupiter_path)
                .map(|c| c.contains("LD_AUDIT"))
                .unwrap_or(false)
        } else {
            false
        };

        let readonly_cmd = Path::new("/usr/bin/steamos-readonly");
        let is_readonly = if readonly_cmd.exists() {
            std::process::Command::new("steamos-readonly")
                .arg("status")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("enabled"))
                .unwrap_or(false)
        } else {
            false
        };

        return Ok(SlssteamStatus {
            is_readonly,
            slssteam_so_exists,
            library_inject_so_exists,
            config_exists,
            config_play_not_owned,
            config_safe_mode_on,
            steam_jupiter_patched,
            desktop_entry_patched,
            additional_apps_count,
        });
    }

    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let readonly_out = ssh_exec(&sess, "steamos-readonly status 2>/dev/null || echo 'not-steamos'")?;
    let is_readonly = readonly_out.to_lowercase().contains("enabled");

    let so_out = ssh_exec(&sess, "test -f ~/.local/share/SLSsteam/SLSsteam.so && echo 'EXISTS' || echo 'MISSING'")?;
    let slssteam_so_exists = so_out.contains("EXISTS");

    let inject_out = ssh_exec(&sess, "test -f ~/.local/share/SLSsteam/library-inject.so && echo 'EXISTS' || echo 'MISSING'")?;
    let library_inject_so_exists = inject_out.contains("EXISTS");

    let config_out = ssh_exec(&sess, "cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''")?;
    let config_exists = !config_out.is_empty() && !config_out.trim().is_empty();

    let config_play_not_owned = config_out.to_lowercase().contains("playnotownedgames: true")
        || config_out.to_lowercase().contains("playnotownedgames: yes");
    let config_safe_mode_on = config_out.to_lowercase().contains("safemode: true")
        || config_out.to_lowercase().contains("safemode: yes");

    let additional_apps_count = config_out
        .lines()
        .filter(|l| l.trim().starts_with("- ") && l.trim().len() > 2)
        .count();

    let jupiter_out = ssh_exec(&sess, "grep -c 'LD_AUDIT' /usr/bin/steam-jupiter 2>/dev/null || echo '0'")?;
    let steam_jupiter_patched = jupiter_out.trim().parse::<i32>().unwrap_or(0) > 0;

    let desktop_out = ssh_exec(&sess, "grep -c 'LD_AUDIT' ~/.local/share/applications/steam.desktop 2>/dev/null || echo '0'")?;
    let desktop_entry_patched = desktop_out.trim().parse::<i32>().unwrap_or(0) > 0;

    Ok(SlssteamStatus {
        is_readonly,
        slssteam_so_exists,
        library_inject_so_exists,
        config_exists,
        config_play_not_owned,
        config_safe_mode_on,
        steam_jupiter_patched,
        desktop_entry_patched,
        additional_apps_count,
    })
}

#[tauri::command]
pub async fn verify_slssteam_local() -> Result<SlssteamLocalStatus, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;

    let slssteam_so_path = home.join(".local/share/SLSsteam/SLSsteam.so");
    let slssteam_so_exists = slssteam_so_path.exists();

    let library_inject_path = home.join(".local/share/SLSsteam/library-inject.so");
    let library_inject_so_exists = library_inject_path.exists();

    let config_path = home.join(".config/SLSsteam/config.yaml");
    let config_exists = config_path.exists();

    let (config_play_not_owned, additional_apps_count) = if config_exists {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let play_not_owned = content.to_lowercase().contains("playnotownedgames: true")
                || content.to_lowercase().contains("playnotownedgames: yes");
            let apps_count = content
                .lines()
                .filter(|l| l.trim().starts_with("- ") && l.trim().len() > 2)
                .count();
            (play_not_owned, apps_count)
        } else {
            (false, 0)
        }
    } else {
        (false, 0)
    };

    let desktop_path = home.join(".local/share/applications/steam.desktop");
    let desktop_entry_patched = if desktop_path.exists() {
        std::fs::read_to_string(&desktop_path)
            .map(|c| c.contains("LD_AUDIT"))
            .unwrap_or(false)
    } else {
        false
    };

    Ok(SlssteamLocalStatus {
        slssteam_so_exists,
        library_inject_so_exists,
        config_exists,
        config_play_not_owned,
        additional_apps_count,
        desktop_entry_patched,
    })
}

#[tauri::command]
pub async fn detect_steam_deck() -> Result<SteamDeckDetection, String> {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let is_steamos = os_release.contains("SteamOS");
    let is_bazzite = os_release.to_lowercase().contains("bazzite");
    let has_jupiter = std::path::Path::new("/usr/bin/steam-jupiter").exists();
    let has_deck_user = std::path::Path::new("/home/deck").exists();

    let os_name = if is_steamos {
        "SteamOS".to_string()
    } else if is_bazzite {
        "Bazzite".to_string()
    } else if cfg!(target_os = "linux") {
        os_release
            .lines()
            .find(|l| l.starts_with("PRETTY_NAME="))
            .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            .unwrap_or_else(|| "Linux".to_string())
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else {
        "Unknown".to_string()
    };

    let is_steam_deck = is_steamos || is_bazzite || (has_jupiter && has_deck_user);

    Ok(SteamDeckDetection {
        is_steam_deck,
        is_steamos,
        os_name,
    })
}

#[tauri::command]
pub async fn check_sshpass_available() -> Result<bool, String> {
    use std::process::Command;
    let result = Command::new("which").arg("sshpass").output();
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn check_readonly_status(config: SshConfig) -> Result<bool, String> {
    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("SSH session error: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let mut channel = sess.channel_session().map_err(|e| format!("Channel error: {}", e))?;
    channel.exec("steamos-readonly status 2>/dev/null || echo 'unknown'")
        .map_err(|e| format!("Exec error: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output).ok();
    channel.wait_close().ok();

    Ok(output.to_lowercase().contains("enabled"))
}

#[tauri::command]
pub async fn install_slssteam(
    config: SshConfig,
    slssteam_path: String,
    root_password: String,
) -> Result<String, String> {
    use std::fs;

    if config.is_local {
        use std::process::Command;
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let mut log = String::new();

        log.push_str("Checking for SteamOS/immutable distro...\n");
        if Path::new("/usr/bin/steamos-readonly").exists() {
            let _ = Command::new("sudo").args(["steamos-readonly", "disable"]).status();
            log.push_str("Readonly disable attempted.\n");
        }

        if !Path::new(&slssteam_path).exists() {
            return Err(format!("SLSsteam.so not found at: {}", slssteam_path));
        }

        let slssteam_dir = home.join(".local/share/SLSsteam");
        let config_dir = home.join(".config/SLSsteam");
        let apps_dir = home.join(".local/share/applications");

        std::fs::create_dir_all(&slssteam_dir)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
        std::fs::create_dir_all(&apps_dir)
            .map_err(|e| format!("Failed to create dir: {}", e))?;

        let dest_so = slssteam_dir.join("SLSsteam.so");
        std::fs::copy(&slssteam_path, &dest_so)
            .map_err(|e| format!("Failed to copy: {}", e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest_so)
                .map_err(|e| format!("Metadata error: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_so, perms)
                .map_err(|e| format!("Permissions error: {}", e))?;
        }
        log.push_str("SLSsteam.so copied.\n");

        let slssteam_cache_dir = get_slssteam_cache_dir()?;
        let library_inject_source = slssteam_cache_dir.join("library-inject.so");
        if library_inject_source.exists() {
            let dest_inject = slssteam_dir.join("library-inject.so");
            std::fs::copy(&library_inject_source, &dest_inject)
                .map_err(|e| format!("Failed to copy: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest_inject)
                    .map_err(|e| format!("Metadata error: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest_inject, perms)
                    .map_err(|e| format!("Permissions error: {}", e))?;
            }
            log.push_str("library-inject.so copied.\n");
        }

        let config_file = config_dir.join("config.yaml");
        let default_config_content = "PlayNotOwnedGames: yes\nSafeMode: yes\nAdditionalApps:\n- 480\n";
        std::fs::write(&config_file, default_config_content)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        log.push_str("Config.yaml written.\n");

        let library_inject_path = slssteam_dir.join("library-inject.so");
        let ld_audit_path = if library_inject_path.exists() {
            format!("{}:{}", library_inject_path.to_string_lossy(), dest_so.to_string_lossy())
        } else {
            dest_so.to_string_lossy().to_string()
        };

        if Path::new("/usr/share/applications/steam.desktop").exists() {
            let original = std::fs::read_to_string("/usr/share/applications/steam.desktop")
                .map_err(|e| format!("Failed to read: {}", e))?;
            let patched = original.replace("Exec=/", &format!("Exec=env LD_AUDIT=\"{}\" /", ld_audit_path));
            let user_desktop = apps_dir.join("steam.desktop");
            std::fs::write(&user_desktop, patched)
                .map_err(|e| format!("Failed to write: {}", e))?;
            log.push_str("steam.desktop patched.\n");
        }

        if Path::new("/usr/bin/steam-jupiter").exists() {
            let _ = Command::new("sudo").args(["cp", "/usr/bin/steam-jupiter", &config_dir.join("steam-jupiter.bak").to_string_lossy()]).status();
            let patch_cmd = format!(
                "sudo sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT=\"{}\" /usr/lib/steam/steam|' /usr/bin/steam-jupiter",
                ld_audit_path
            );
            let _ = Command::new("sh").args(["-c", &patch_cmd]).status();
            log.push_str("steam-jupiter patched.\n");
        }

        log.push_str("Local SLSsteam installation complete!\n");
        return Ok(log);
    }

    // Remote mode
    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }
    if !Path::new(&slssteam_path).exists() {
        return Err(format!("SLSsteam.so not found at: {}", slssteam_path));
    }

    let slssteam_bytes = fs::read(&slssteam_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
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

    let mut log = String::new();

    ssh_exec(&sess, "mkdir -p ~/.local/share/SLSsteam ~/.config/SLSsteam")?;
    log.push_str("Directories created.\n");

    let default_config = "PlayNotOwnedGames: yes\nSafeMode: yes\nAdditionalApps:\n- 480\n";
    let config_remote_path = "/home/deck/.config/SLSsteam/config.yaml";
    let sftp_config = sess.sftp()
        .map_err(|e| format!("SFTP error: {}", e))?;
    let mut config_file = sftp_config.create(Path::new(config_remote_path))
        .map_err(|e| format!("Create file error: {}", e))?;
    config_file.write_all(default_config.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    drop(config_file);
    drop(sftp_config);
    log.push_str("Config written.\n");

    let sftp = sess.sftp()
        .map_err(|e| format!("SFTP error: {}", e))?;
    let remote_path = "/home/deck/.local/share/SLSsteam/SLSsteam.so";
    let mut remote_file = sftp.create(Path::new(remote_path))
        .map_err(|e| format!("Create file error: {}", e))?;
    remote_file.write_all(&slssteam_bytes)
        .map_err(|e| format!("Write error: {}", e))?;
    drop(remote_file);

    ssh_exec(&sess, "chmod 755 ~/.local/share/SLSsteam/SLSsteam.so")?;
    log.push_str("SLSsteam.so uploaded.\n");

    let slssteam_cache_dir = get_slssteam_cache_dir()?;
    let library_inject_path = slssteam_cache_dir.join("library-inject.so");
    if library_inject_path.exists() {
        let inject_bytes = fs::read(&library_inject_path)
            .map_err(|e| format!("Read error: {}", e))?;
        let sftp2 = sess.sftp()
            .map_err(|e| format!("SFTP error: {}", e))?;
        let remote_inject_path = "/home/deck/.local/share/SLSsteam/library-inject.so";
        let mut remote_inject = sftp2.create(Path::new(remote_inject_path))
            .map_err(|e| format!("Create file error: {}", e))?;
        remote_inject.write_all(&inject_bytes)
            .map_err(|e| format!("Write error: {}", e))?;
        ssh_exec(&sess, "chmod 755 ~/.local/share/SLSsteam/library-inject.so")?;
        log.push_str("library-inject.so uploaded.\n");
    }

    ssh_exec(&sess, "mkdir -p ~/.local/share/applications")?;

    let desktop_cmd = r#"
if [ -f /usr/share/applications/steam.desktop ]; then
    cp /usr/share/applications/steam.desktop ~/.local/share/applications/
    if [ -f ~/.local/share/SLSsteam/library-inject.so ]; then
        sed -i 's|^Exec=/|Exec=env LD_AUDIT="/home/deck/.local/share/SLSsteam/library-inject.so:/home/deck/.local/share/SLSsteam/SLSsteam.so" /|' ~/.local/share/applications/steam.desktop
    else
        sed -i 's|^Exec=/|Exec=env LD_AUDIT="/home/deck/.local/share/SLSsteam/SLSsteam.so" /|' ~/.local/share/applications/steam.desktop
    fi
    echo 'DESKTOP_OK'
fi
"#;
    ssh_exec(&sess, desktop_cmd)?;
    log.push_str("steam.desktop patched.\n");

    let sudo_backup = format!(
        "echo '{}' | sudo -S cp /usr/bin/steam-jupiter ~/.config/SLSsteam/steam-jupiter.bak 2>&1",
        root_password
    );
    ssh_exec(&sess, &sudo_backup)?;

    let check_inject = ssh_exec(&sess, "test -f ~/.local/share/SLSsteam/library-inject.so && echo 'EXISTS' || echo 'MISSING'");
    let ld_audit_remote = if check_inject.as_ref().map(|s| s.contains("EXISTS")).unwrap_or(false) {
        "/home/deck/.local/share/SLSsteam/library-inject.so:/home/deck/.local/share/SLSsteam/SLSsteam.so"
    } else {
        "/home/deck/.local/share/SLSsteam/SLSsteam.so"
    };
    let patch_cmd = format!(
        r#"echo '{}' | sudo -S sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT="{}" /usr/lib/steam/steam|' /usr/bin/steam-jupiter 2>&1"#,
        root_password, ld_audit_remote
    );
    ssh_exec(&sess, &patch_cmd)?;
    log.push_str("steam-jupiter patched.\n");

    log.push_str("\nSLSsteam installation complete!\n");
    log.push_str("Please restart Steam for changes to take effect.");

    Ok(log)
}

/// Modify a section in SLSsteam config.yaml
/// Used for adding AppTokens, FakeAppIds, etc.
pub fn modify_slssteam_config_section(content: &str, section: &str, key: &str, value: &str) -> String {
    // Check if key already exists in section
    let section_key = format!("{}:", section);
    let key_pattern = format!("  {}: ", key);
    
    if content.contains(&key_pattern) {
        // Key exists, update it
        let mut result = String::new();
        let mut in_section = false;
        let mut found_key = false;
        
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&format!("{}:", section)) {
                in_section = true;
                result.push_str(line);
                result.push('\n');
            } else if in_section && trimmed.starts_with(&format!("{}: ", key)) {
                result.push_str(&format!("  {}: {}\n", key, value));
                found_key = true;
            } else if in_section && !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with(' ') {
                in_section = false;
                result.push_str(line);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        if !found_key && !result.is_empty() {
            result.pop(); // Remove trailing newline
        }
        return result;
    }
    
    // Check if section exists
    if content.contains(&section_key) {
        // Section exists, add key under it
        let mut result = String::new();
        for line in content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim().starts_with(&section_key) {
                result.push_str(&format!("  {}: {}\n", key, value));
            }
        }
        return result;
    }
    
    // Section doesn't exist, add it
    let mut result = content.to_string();
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&format!("\n{}:\n  {}: {}\n", section, key, value));
    result
}

/// Add a fake app ID to SLSsteam config
#[tauri::command]
pub async fn add_fake_app_id(
    app_handle: tauri::AppHandle,
    app_id: String,
    fake_app_id: String,
) -> Result<String, String> {
    let _ = app_handle; // Used for future async operations
    
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_path = home.join(".config/SLSsteam/config.yaml");
    
    if !config_path.exists() {
        return Err("SLSsteam config not found".to_string());
    }
    
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    
    let new_config = modify_slssteam_config_section(&content, "FakeAppIds", &app_id, &fake_app_id);
    
    std::fs::write(&config_path, &new_config)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
    Ok(format!("Added FakeAppId: {} -> {}", app_id, fake_app_id))
}

/// Add an app token to SLSsteam config
#[tauri::command]
pub async fn add_app_token(
    app_handle: tauri::AppHandle,
    app_id: String,
    token: String,
) -> Result<String, String> {
    let _ = app_handle;
    
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_path = home.join(".config/SLSsteam/config.yaml");
    
    if !config_path.exists() {
        return Err("SLSsteam config not found".to_string());
    }
    
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    
    let new_config = modify_slssteam_config_section(&content, "AppTokens", &app_id, &token);
    
    std::fs::write(&config_path, &new_config)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
    Ok(format!("Added AppToken for: {}", app_id))
}

/// Generate achievements file (placeholder - not implemented)
#[tauri::command]
pub async fn generate_achievements(
    app_handle: tauri::AppHandle,
    app_id: String,
) -> Result<String, String> {
    let _ = app_handle;
    // Placeholder - actual achievement generation requires Steam API integration
    Ok(format!("Achievement generation not yet implemented for {}", app_id))
}
