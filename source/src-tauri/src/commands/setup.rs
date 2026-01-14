//! Setup wizard commands - unified installation/verification flow
//! Inspired by enter-the-wired and headcrab.sh patterns

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use super::connection::SshConfig;

/// Setup step status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Error,
    Skipped,
}

/// A single setup step's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStep {
    pub id: String,
    pub name: String,
    pub status: StepStatus,
    pub message: Option<String>,
}

/// Overall setup state emitted to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupState {
    pub steps: Vec<SetupStep>,
    pub current_step: usize,
    pub is_complete: bool,
    pub error: Option<String>,
}

/// Result of running the full setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupResult {
    pub success: bool,
    pub steps_completed: usize,
    pub error: Option<String>,
}

/// Check if this is the first launch (no settings saved)
#[tauri::command]
pub async fn is_first_launch(app_handle: AppHandle) -> Result<bool, String> {
    let app_data = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let settings_file = app_data.join("settings.json");
    let first_launch_marker = app_data.join(".setup_complete");

    // First launch if no settings and no marker
    Ok(!settings_file.exists() && !first_launch_marker.exists())
}

/// Mark setup as complete (create marker file)
#[tauri::command]
pub async fn mark_setup_complete(app_handle: AppHandle) -> Result<(), String> {
    let app_data = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    std::fs::create_dir_all(&app_data)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let marker = app_data.join(".setup_complete");
    std::fs::write(&marker, "1").map_err(|e| format!("Failed to write marker: {}", e))?;

    Ok(())
}

/// Reset setup (remove marker for testing/re-run)
#[tauri::command]
pub async fn reset_setup(app_handle: AppHandle) -> Result<(), String> {
    let app_data = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let marker = app_data.join(".setup_complete");
    if marker.exists() {
        std::fs::remove_file(&marker).map_err(|e| format!("Failed to remove marker: {}", e))?;
    }

    Ok(())
}

/// Run the full setup wizard
/// This orchestrates all setup steps and emits progress events
#[tauri::command]
pub async fn run_full_setup(
    app_handle: AppHandle,
    mode: String,
    ssh_config: Option<SshConfig>,
) -> Result<SetupResult, String> {
    let is_local = mode == "local";

    // Define setup steps
    let mut steps: Vec<SetupStep> = vec![
        SetupStep {
            id: "deps".into(),
            name: if is_local {
                "Check 32-bit Dependencies".into()
            } else {
                "Check SSH Connection".into()
            },
            status: StepStatus::Pending,
            message: None,
        },
        SetupStep {
            id: "slssteam_download".into(),
            name: "Download SLSsteam".into(),
            status: StepStatus::Pending,
            message: None,
        },
        SetupStep {
            id: "slssteam_install".into(),
            name: "Install SLSsteam".into(),
            status: StepStatus::Pending,
            message: None,
        },
        SetupStep {
            id: "patch_steam".into(),
            name: "Patch Steam Launcher".into(),
            status: StepStatus::Pending,
            message: None,
        },
        SetupStep {
            id: "block_updates".into(),
            name: "Block Steam Updates".into(),
            status: StepStatus::Pending,
            message: None,
        },
    ];

    let mut state = SetupState {
        steps: steps.clone(),
        current_step: 0,
        is_complete: false,
        error: None,
    };

    // Helper to emit state
    let emit_state = |app: &AppHandle, state: &SetupState| {
        let _ = app.emit("setup_progress", state.clone());
    };

    // Step 1: Check dependencies / SSH connection
    steps[0].status = StepStatus::Running;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    if is_local {
        // Check 32-bit deps locally
        match check_local_dependencies().await {
            Ok(msg) => {
                steps[0].status = StepStatus::Done;
                steps[0].message = Some(msg);
            }
            Err(e) => {
                steps[0].status = StepStatus::Error;
                steps[0].message = Some(e.clone());
                state.steps = steps;
                state.error = Some(e);
                emit_state(&app_handle, &state);
                return Ok(SetupResult {
                    success: false,
                    steps_completed: 0,
                    error: state.error,
                });
            }
        }
    } else if let Some(ref config) = ssh_config {
        // Test SSH connection
        match super::connection::test_ssh(config.clone()).await {
            Ok(_) => {
                steps[0].status = StepStatus::Done;
                steps[0].message = Some("SSH connection OK".into());
            }
            Err(e) => {
                steps[0].status = StepStatus::Error;
                steps[0].message = Some(e.clone());
                state.steps = steps;
                state.error = Some(e);
                emit_state(&app_handle, &state);
                return Ok(SetupResult {
                    success: false,
                    steps_completed: 0,
                    error: state.error,
                });
            }
        }
    } else {
        steps[0].status = StepStatus::Error;
        steps[0].message = Some("SSH config required for remote mode".into());
        state.steps = steps;
        state.error = Some("SSH config missing".into());
        emit_state(&app_handle, &state);
        return Ok(SetupResult {
            success: false,
            steps_completed: 0,
            error: state.error,
        });
    }

    state.current_step = 1;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    // Step 2: Download SLSsteam
    steps[1].status = StepStatus::Running;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    match super::settings::fetch_latest_slssteam().await {
        Ok(msg) => {
            steps[1].status = StepStatus::Done;
            steps[1].message = Some(msg);
        }
        Err(e) => {
            steps[1].status = StepStatus::Error;
            steps[1].message = Some(e.clone());
            state.steps = steps;
            state.error = Some(e);
            emit_state(&app_handle, &state);
            return Ok(SetupResult {
                success: false,
                steps_completed: 1,
                error: state.error,
            });
        }
    }

    state.current_step = 2;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    // Step 3: Install SLSsteam
    steps[2].status = StepStatus::Running;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    let slssteam_path = super::settings::get_cached_slssteam_path()
        .await
        .map_err(|e| format!("Failed to get SLSsteam path: {}", e))?
        .ok_or("SLSsteam not downloaded")?;

    let config_for_install = if is_local {
        SshConfig {
            ip: String::new(),
            port: 22,
            username: String::new(),
            password: String::new(),
            private_key_path: String::new(),
            is_local: true,
        }
    } else {
        ssh_config.clone().unwrap()
    };

    match super::slssteam::install_slssteam(
        config_for_install.clone(),
        slssteam_path,
        String::new(), // Root password not needed for most installs
    )
    .await
    {
        Ok(msg) => {
            steps[2].status = StepStatus::Done;
            steps[2].message = Some(msg);
        }
        Err(e) => {
            steps[2].status = StepStatus::Error;
            steps[2].message = Some(e.clone());
            state.steps = steps;
            state.error = Some(e);
            emit_state(&app_handle, &state);
            return Ok(SetupResult {
                success: false,
                steps_completed: 2,
                error: state.error,
            });
        }
    }

    state.current_step = 3;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    // Step 4: Patch Steam launcher (part of install, mark done)
    steps[3].status = StepStatus::Done;
    steps[3].message = Some("Steam launcher patched with LD_AUDIT".into());

    state.current_step = 4;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    // Step 5: Block Steam updates
    steps[4].status = StepStatus::Running;
    state.steps = steps.clone();
    emit_state(&app_handle, &state);

    match super::steam_fixes::disable_steam_updates(config_for_install).await {
        Ok(msg) => {
            steps[4].status = StepStatus::Done;
            steps[4].message = Some(msg);
        }
        Err(e) => {
            // Non-fatal - warn but continue
            steps[4].status = StepStatus::Done;
            steps[4].message = Some(format!("Warning: {}", e));
        }
    }

    // Mark as complete
    state.steps = steps;
    state.current_step = 5;
    state.is_complete = true;
    emit_state(&app_handle, &state);

    // Create marker
    let _ = mark_setup_complete(app_handle).await;

    Ok(SetupResult {
        success: true,
        steps_completed: 5,
        error: None,
    })
}

/// Check local 32-bit dependencies
async fn check_local_dependencies() -> Result<String, String> {
    let libs = [
        ("lib32-curl", "/usr/lib32/libcurl.so.4"),
        ("lib32-openssl", "/usr/lib32/libssl.so"),
        ("lib32-glibc", "/usr/lib32/libc.so.6"),
    ];

    let mut missing = Vec::new();
    for (name, path) in libs {
        if !PathBuf::from(path).exists() {
            missing.push(name);
        }
    }

    if missing.is_empty() {
        Ok("All 32-bit dependencies installed".into())
    } else {
        Err(format!(
            "Missing dependencies: {}. Install with: sudo pacman -S {}",
            missing.join(", "),
            missing.join(" ")
        ))
    }
}
