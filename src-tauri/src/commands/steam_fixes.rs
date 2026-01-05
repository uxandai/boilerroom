//! Steam fixes commands - Steam update disabling and libcurl32 symlink

use super::connection::SshConfig;
use super::slssteam::ssh_exec;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// Disable Steam updates to prevent hash mismatch with SLSsteam
/// Creates/modifies $HOME/.steam/steam/steam.cfg
#[tauri::command]
pub async fn disable_steam_updates(config: SshConfig) -> Result<String, String> {
    let config_content = r#"BootStrapperInhibitAll=enable
BootStrapperForceSelfUpdate=disable
"#;

    if config.is_local || config.ip.is_empty() {
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let steam_dir = PathBuf::from(&home).join(".steam/steam");
        let config_path = steam_dir.join("steam.cfg");

        std::fs::create_dir_all(&steam_dir)
            .map_err(|e| format!("Failed to create steam directory: {}", e))?;

        let existing_content = if config_path.exists() {
            std::fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let has_inhibit = existing_content.contains("BootStrapperInhibitAll=enable");
        let has_force_disable = existing_content.contains("BootStrapperForceSelfUpdate=disable");

        if has_inhibit && has_force_disable {
            return Ok("Steam update disable already configured. No changes needed.".to_string());
        }

        let mut new_content = existing_content
            .lines()
            .filter(|line| {
                !line.starts_with("BootStrapperInhibitAll=")
                    && !line.starts_with("BootStrapperForceSelfUpdate=")
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(config_content);

        std::fs::write(&config_path, &new_content)
            .map_err(|e| format!("Failed to write steam.cfg: {}", e))?;

        return Ok(format!(
            "Steam updates disabled locally.\nModified: {}",
            config_path.display()
        ));
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let cmd = r#"
mkdir -p ~/.steam/steam
CONFIG_FILE="$HOME/.steam/steam/steam.cfg"
if [ -f "$CONFIG_FILE" ]; then
    sed -i '/^BootStrapperInhibitAll=/d' "$CONFIG_FILE"
    sed -i '/^BootStrapperForceSelfUpdate=/d' "$CONFIG_FILE"
fi
echo 'BootStrapperInhibitAll=enable' >> "$CONFIG_FILE"
echo 'BootStrapperForceSelfUpdate=disable' >> "$CONFIG_FILE"
echo "Steam updates disabled."
"#;

    let output = ssh_exec(&sess, cmd)?;
    Ok(format!("Steam updates disabled on remote.\n\n{}", output.trim()))
}

/// Fix libcurl32 symlink issue for Steam
#[tauri::command]
pub async fn fix_libcurl32(config: SshConfig) -> Result<String, String> {
    use std::os::unix::fs::symlink;

    let source = "/usr/lib32/libcurl.so.4";

    if config.is_local || config.ip.is_empty() {
        let home = std::env::var("HOME")
            .map_err(|_| "Could not get HOME environment variable".to_string())?;

        let target_dir = PathBuf::from(&home).join(".steam/steam/ubuntu12_32");
        let target = target_dir.join("libcurl.so.4");

        if !PathBuf::from(source).exists() {
            return Err(format!("Source library not found: {}\n\nMake sure lib32-curl is installed", source));
        }

        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create target directory: {}", e))?;

        if target.exists() || target.symlink_metadata().is_ok() {
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove existing file: {}", e))?;
        }

        symlink(source, &target).map_err(|e| format!("Failed to create symlink: {}", e))?;

        return Ok(format!("libcurl32 symlink created: {} -> {}", target.display(), source));
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let cmd = format!(
        r#"
if [ ! -f "{source}" ]; then
    echo "ERROR: {source} not found"
    exit 1
fi
mkdir -p ~/.steam/steam/ubuntu12_32
ln -sf "{source}" ~/.steam/steam/ubuntu12_32/libcurl.so.4
echo "Symlink created:"
ls -la ~/.steam/steam/ubuntu12_32/libcurl.so.4
"#, source = source);

    let output = ssh_exec(&sess, &cmd)?;
    if output.contains("ERROR:") {
        return Err(output);
    }
    Ok(format!("libcurl32 symlink created on remote.\n\n{}", output.trim()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamUpdatesStatus {
    pub is_configured: bool,
    pub inhibit_all: bool,
    pub force_self_update_disabled: bool,
    pub config_path: String,
}

#[tauri::command]
pub async fn check_steam_updates_status(config: SshConfig) -> Result<SteamUpdatesStatus, String> {
    if config.is_local || config.ip.is_empty() {
        let home = std::env::var("HOME").map_err(|_| "Could not get HOME".to_string())?;
        let config_path = PathBuf::from(&home).join(".steam/steam/steam.cfg");

        if !config_path.exists() {
            return Ok(SteamUpdatesStatus {
                is_configured: false,
                inhibit_all: false,
                force_self_update_disabled: false,
                config_path: config_path.to_string_lossy().to_string(),
            });
        }

        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let inhibit_all = content.contains("BootStrapperInhibitAll=enable");
        let force_self_update_disabled = content.contains("BootStrapperForceSelfUpdate=disable");

        return Ok(SteamUpdatesStatus {
            is_configured: inhibit_all && force_self_update_disabled,
            inhibit_all,
            force_self_update_disabled,
            config_path: config_path.to_string_lossy().to_string(),
        });
    }

    let ip: IpAddr = config.ip.parse().map_err(|_| format!("Invalid IP: {}", config.ip))?;
    let addr = SocketAddr::new(ip, config.port);
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;
    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;
    sess.userauth_password(&config.username, &config.password)
        .map_err(|e| format!("SSH auth failed: {}", e))?;

    let output = ssh_exec(&sess, "cat ~/.steam/steam/steam.cfg 2>/dev/null || echo 'FILE_NOT_FOUND'")?;

    if output.contains("FILE_NOT_FOUND") {
        return Ok(SteamUpdatesStatus {
            is_configured: false,
            inhibit_all: false,
            force_self_update_disabled: false,
            config_path: "~/.steam/steam/steam.cfg".to_string(),
        });
    }

    let inhibit_all = output.contains("BootStrapperInhibitAll=enable");
    let force_self_update_disabled = output.contains("BootStrapperForceSelfUpdate=disable");

    Ok(SteamUpdatesStatus {
        is_configured: inhibit_all && force_self_update_disabled,
        inhibit_all,
        force_self_update_disabled,
        config_path: "~/.steam/steam/steam.cfg".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Libcurl32Status {
    pub source_exists: bool,
    pub symlink_exists: bool,
    pub symlink_correct: bool,
    pub source_path: String,
    pub target_path: String,
}

#[tauri::command]
pub async fn check_libcurl32_status(config: SshConfig) -> Result<Libcurl32Status, String> {
    let source = "/usr/lib32/libcurl.so.4";

    if config.is_local || config.ip.is_empty() {
        let home = std::env::var("HOME").map_err(|_| "Could not get HOME".to_string())?;
        let target_path = PathBuf::from(&home).join(".steam/steam/ubuntu12_32/libcurl.so.4");
        let source_exists = PathBuf::from(source).exists();

        let (symlink_exists, symlink_correct) = if target_path.symlink_metadata().is_ok() {
            if let Ok(link_target) = std::fs::read_link(&target_path) {
                (true, link_target == PathBuf::from(source))
            } else {
                (true, false)
            }
        } else {
            (false, false)
        };

        return Ok(Libcurl32Status {
            source_exists,
            symlink_exists,
            symlink_correct,
            source_path: source.to_string(),
            target_path: target_path.to_string_lossy().to_string(),
        });
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

    let cmd = format!(r#"
SOURCE_EXISTS="false"
SYMLINK_EXISTS="false"
SYMLINK_CORRECT="false"
if [ -f "{source}" ]; then SOURCE_EXISTS="true"; fi
TARGET="$HOME/.steam/steam/ubuntu12_32/libcurl.so.4"
if [ -L "$TARGET" ]; then
    SYMLINK_EXISTS="true"
    LINK_TARGET=$(readlink "$TARGET")
    if [ "$LINK_TARGET" = "{source}" ]; then SYMLINK_CORRECT="true"; fi
elif [ -f "$TARGET" ]; then
    SYMLINK_EXISTS="true"
fi
echo "SOURCE_EXISTS=$SOURCE_EXISTS"
echo "SYMLINK_EXISTS=$SYMLINK_EXISTS"
echo "SYMLINK_CORRECT=$SYMLINK_CORRECT"
"#, source = source);

    let output = ssh_exec(&sess, &cmd)?;

    Ok(Libcurl32Status {
        source_exists: output.contains("SOURCE_EXISTS=true"),
        symlink_exists: output.contains("SYMLINK_EXISTS=true"),
        symlink_correct: output.contains("SYMLINK_CORRECT=true"),
        source_path: source.to_string(),
        target_path: "~/.steam/steam/ubuntu12_32/libcurl.so.4".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lib32DependenciesStatus {
    pub lib32_curl_installed: bool,
    pub lib32_openssl_installed: bool,
    pub lib32_glibc_installed: bool,
    pub all_installed: bool,
}

#[tauri::command]
pub async fn check_lib32_dependencies(config: SshConfig) -> Result<Lib32DependenciesStatus, String> {
    if config.is_local || config.ip.is_empty() {
        let lib32_curl_installed = PathBuf::from("/usr/lib32/libcurl.so.4").exists();
        let lib32_openssl_installed = PathBuf::from("/usr/lib32/libssl.so").exists()
            || PathBuf::from("/usr/lib32/libssl.so.3").exists();
        let lib32_glibc_installed = PathBuf::from("/usr/lib32/libc.so.6").exists();
        let all_installed = lib32_curl_installed && lib32_openssl_installed && lib32_glibc_installed;

        return Ok(Lib32DependenciesStatus {
            lib32_curl_installed,
            lib32_openssl_installed,
            lib32_glibc_installed,
            all_installed,
        });
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

    let cmd = r#"
LIB32_CURL="false"
LIB32_OPENSSL="false"
LIB32_GLIBC="false"
if [ -f "/usr/lib32/libcurl.so.4" ]; then LIB32_CURL="true"; fi
if [ -f "/usr/lib32/libssl.so" ] || [ -f "/usr/lib32/libssl.so.3" ]; then LIB32_OPENSSL="true"; fi
if [ -f "/usr/lib32/libc.so.6" ]; then LIB32_GLIBC="true"; fi
echo "LIB32_CURL=$LIB32_CURL"
echo "LIB32_OPENSSL=$LIB32_OPENSSL"
echo "LIB32_GLIBC=$LIB32_GLIBC"
"#;

    let output = ssh_exec(&sess, cmd)?;

    let lib32_curl_installed = output.contains("LIB32_CURL=true");
    let lib32_openssl_installed = output.contains("LIB32_OPENSSL=true");
    let lib32_glibc_installed = output.contains("LIB32_GLIBC=true");
    let all_installed = lib32_curl_installed && lib32_openssl_installed && lib32_glibc_installed;

    Ok(Lib32DependenciesStatus {
        lib32_curl_installed,
        lib32_openssl_installed,
        lib32_glibc_installed,
        all_installed,
    })
}
