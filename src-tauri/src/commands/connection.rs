//! SSH connection and Steam Deck status commands

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

// SSH Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub ip: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub private_key_path: String,
    #[serde(default)]
    pub is_local: bool,
}

/// Check if the Steam Deck is reachable (ping via TCP connect)
#[tauri::command]
pub async fn check_deck_status(ip: String, port: u16) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);

    // Try TCP connect with timeout (simulates ping + port check)
    match TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?,
        Duration::from_secs(3),
    ) {
        Ok(_) => Ok("online".to_string()),
        Err(_) => Ok("offline".to_string()),
    }
}

/// Test SSH connection with credentials
#[tauri::command]
pub async fn test_ssh(config: SshConfig) -> Result<String, String> {
    use std::net::{IpAddr, SocketAddr};

    // Validate and parse IP address
    if config.ip.is_empty() {
        return Err("IP address is required".to_string());
    }

    let ip: IpAddr = config
        .ip
        .parse()
        .map_err(|_| format!("Invalid IP address: {}", config.ip))?;

    let addr = SocketAddr::new(ip, config.port);

    // Connect TCP first
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("Connection failed: {} ({}:{})", e, config.ip, config.port))?;

    // Create SSH session
    let mut sess =
        ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Try authentication (password auth since user doesn't use SSH keys)
    if !config.password.is_empty() {
        sess.userauth_password(&config.username, &config.password)
            .map_err(|e| format!("SSH authentication failed: {}", e))?;
    } else if !config.private_key_path.is_empty() {
        let key_path = Path::new(&config.private_key_path);
        sess.userauth_pubkey_file(&config.username, None, key_path, None)
            .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else {
        return Err("Password is required".to_string());
    }

    if !sess.authenticated() {
        return Err("Authentication failed".to_string());
    }

    // Run a simple command to verify
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec("echo 'SSH OK'")
        .map_err(|e| format!("Failed to exec command: {}", e))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| format!("Failed to read output: {}", e))?;

    channel.wait_close().ok();

    Ok(output.trim().to_string())
}
