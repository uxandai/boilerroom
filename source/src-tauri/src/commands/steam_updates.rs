//! Smart Steam update handling
//! Implements the enter-the-wired pattern: allow update → reblock

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::connection::SshConfig;

/// Status of Steam update handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamUpdateResult {
    pub success: bool,
    pub steam_updated: bool,
    pub slssteam_reapplied: bool,
    pub updates_blocked: bool,
    pub message: String,
}

/// Get the LD_AUDIT path for SLSsteam
fn get_ld_audit_path(is_flatpak: bool) -> String {
    if is_flatpak {
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/SLSsteam/library-inject.so:{}/.var/app/com.valvesoftware.Steam/.local/share/SLSsteam/SLSsteam.so",
            std::env::var("HOME").unwrap_or_default(),
            std::env::var("HOME").unwrap_or_default()
        )
    } else {
        format!(
            "{}/.local/share/SLSsteam/library-inject.so:{}/.local/share/SLSsteam/SLSsteam.so",
            std::env::var("HOME").unwrap_or_default(),
            std::env::var("HOME").unwrap_or_default()
        )
    }
}

/// Check if Steam is using Flatpak
fn is_flatpak_steam() -> bool {
    let flatpak_steam = PathBuf::from(format!(
        "{}/.var/app/com.valvesoftware.Steam/.steam/steam",
        std::env::var("HOME").unwrap_or_default()
    ));
    flatpak_steam.exists()
}

/// Get steam.cfg path
fn get_steam_cfg_path(is_flatpak: bool) -> PathBuf {
    if is_flatpak {
        PathBuf::from(format!(
            "{}/.var/app/com.valvesoftware.Steam/.steam/steam/steam.cfg",
            std::env::var("HOME").unwrap_or_default()
        ))
    } else {
        PathBuf::from(format!(
            "{}/.steam/steam/steam.cfg",
            std::env::var("HOME").unwrap_or_default()
        ))
    }
}

/// Remove steam.cfg to allow Steam to update
fn remove_steam_cfg(is_flatpak: bool) -> Result<(), String> {
    let cfg_path = get_steam_cfg_path(is_flatpak);
    if cfg_path.exists() {
        std::fs::remove_file(&cfg_path)
            .map_err(|e| format!("Failed to remove steam.cfg: {}", e))?;
        eprintln!("[steam_updates] Removed steam.cfg to allow updates");
    }
    Ok(())
}

/// Create steam.cfg to block updates
fn create_steam_cfg(is_flatpak: bool) -> Result<(), String> {
    let cfg_path = get_steam_cfg_path(is_flatpak);
    let content = "BootStrapperInhibitAll=enable\nBootStrapperForceSelfUpdate=disable\n";
    std::fs::write(&cfg_path, content).map_err(|e| format!("Failed to create steam.cfg: {}", e))?;
    eprintln!("[steam_updates] Created steam.cfg to block updates");
    Ok(())
}

/// Launch Steam with SLSsteam injection and wait for it to exit
fn launch_steam_with_sls_and_wait(is_flatpak: bool, timeout: Duration) -> Result<bool, String> {
    let ld_audit = get_ld_audit_path(is_flatpak);

    eprintln!("[steam_updates] Launching Steam with LD_AUDIT={}", ld_audit);

    let steam_cmd = if is_flatpak {
        "com.valvesoftware.Steam"
    } else {
        "steam"
    };

    // Launch Steam with LD_AUDIT
    let child = Command::new("env")
        .arg(format!("LD_AUDIT={}", ld_audit))
        .arg(steam_cmd)
        .spawn()
        .map_err(|e| format!("Failed to launch Steam: {}", e))?;

    let pid = child.id();
    eprintln!("[steam_updates] Steam started with PID {}", pid);

    // Wait for Steam to exit (with timeout)
    let start = std::time::Instant::now();
    loop {
        // Check if Steam process is still running
        let output = Command::new("pgrep").arg("-x").arg("steam").output();

        match output {
            Ok(o) if o.stdout.is_empty() => {
                eprintln!("[steam_updates] Steam has exited");
                return Ok(true);
            }
            _ => {}
        }

        if start.elapsed() > timeout {
            eprintln!("[steam_updates] Timeout waiting for Steam to exit");
            // Try to close Steam gracefully
            let _ = Command::new(steam_cmd).arg("-shutdown").spawn();
            std::thread::sleep(Duration::from_secs(5));
            return Ok(false);
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Handle Steam update with the enter-the-wired pattern:
/// 1. Remove steam.cfg to allow updates
/// 2. Launch Steam with SLSsteam (allows controlled update)
/// 3. Wait for Steam to finish and exit
/// 4. Recreate steam.cfg to block future updates
/// 5. Re-verify SLSsteam patches
#[tauri::command]
pub async fn handle_steam_update(config: SshConfig) -> Result<SteamUpdateResult, String> {
    let is_local = config.is_local;

    if !is_local {
        return Err("Steam update handling is only supported in local mode".into());
    }

    let is_flatpak = is_flatpak_steam();
    eprintln!(
        "[steam_updates] Using {} Steam",
        if is_flatpak { "Flatpak" } else { "Native" }
    );

    // Step 1: Remove steam.cfg
    remove_steam_cfg(is_flatpak)?;

    // Step 2 & 3: Launch Steam with SLSsteam and wait
    let steam_exited = launch_steam_with_sls_and_wait(is_flatpak, Duration::from_secs(300))?;

    // Step 4: Recreate steam.cfg
    create_steam_cfg(is_flatpak)?;

    // Step 5: Verify SLSsteam is still working
    let slssteam_ok = verify_slssteam_files(is_flatpak);

    Ok(SteamUpdateResult {
        success: true,
        steam_updated: steam_exited,
        slssteam_reapplied: slssteam_ok,
        updates_blocked: true,
        message: if steam_exited {
            "Steam updated successfully. SLSsteam re-applied and updates blocked.".into()
        } else {
            "Steam update timed out. Updates are now blocked.".into()
        },
    })
}

/// Verify SLSsteam files exist
fn verify_slssteam_files(is_flatpak: bool) -> bool {
    let base = if is_flatpak {
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/SLSsteam",
            std::env::var("HOME").unwrap_or_default()
        )
    } else {
        format!(
            "{}/.local/share/SLSsteam",
            std::env::var("HOME").unwrap_or_default()
        )
    };

    let slssteam_so = PathBuf::from(&base).join("SLSsteam.so");
    let library_inject = PathBuf::from(&base).join("library-inject.so");

    slssteam_so.exists() && library_inject.exists()
}

/// Quick check if Steam updates are currently blocked
#[tauri::command]
pub async fn are_steam_updates_blocked(config: SshConfig) -> Result<bool, String> {
    let is_local = config.is_local;

    if !is_local {
        // For remote, use the existing SSH-based check
        return super::steam_fixes::check_steam_updates_status(config)
            .await
            .map(|status| status.is_configured);
    }

    let is_flatpak = is_flatpak_steam();
    let cfg_path = get_steam_cfg_path(is_flatpak);

    if !cfg_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("Failed to read steam.cfg: {}", e))?;

    Ok(content.contains("BootStrapperInhibitAll=enable"))
}
