mod commands;
mod install_manager;

use commands::*;
use install_manager::InstallManager;
use std::fs;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Linux WebKit/Wayland fix - must be set before WebKit initializes
    // WebKit has issues with Wayland on many distros (Arch, SteamOS, etc.)
    #[cfg(target_os = "linux")]
    {
        // Check if running on Wayland
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
            || std::env::var("WAYLAND_DISPLAY").is_ok();

        // Check if SteamOS specifically
        let is_steamos = std::path::Path::new("/etc/steamos-release").exists();

        if is_wayland || is_steamos {
            eprintln!(
                "[Tauri] Wayland/SteamOS detected, forcing X11 backend for WebKit compatibility"
            );
            std::env::set_var("GDK_BACKEND", "x11");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            // Linux WebKit cache fix - clear cache on startup to prevent white screen
            #[cfg(target_os = "linux")]
            {
                if let Some(home) = std::env::var_os("HOME") {
                    let app_name = app.package_info().name.clone();
                    let cache_path = std::path::Path::new(&home).join(".cache").join(&app_name);
                    if cache_path.exists() {
                        let _ = fs::remove_dir_all(&cache_path);
                        eprintln!("[Tauri] Cleared WebKit cache: {:?}", cache_path);
                    }
                }
            }

            let handle = app.handle().clone();
            app.manage(InstallManager::new(handle));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            // Connection commands
            check_deck_status,
            test_ssh,
            // Search commands
            search_bundles,
            // Download/Install commands
            download_bundle,
            extract_manifest_zip,
            run_depot_downloader,
            start_pipelined_install,
            cancel_installation,
            pause_installation,
            resume_installation,
            get_depot_downloader_path,
            cleanup_temp_files,
            // Steamless commands
            find_game_executables,
            run_steamless,
            // Deploy commands
            upload_to_deck,
            extract_remote,
            update_slssteam_config,
            // SLSsteam installation commands
            check_readonly_status,
            install_slssteam,
            verify_slssteam,
            verify_slssteam_local,
            detect_steam_deck,
            check_sshpass_available,
            // Library management commands
            list_installed_games,
            list_installed_games_local,
            uninstall_game,
            check_game_update,
            check_game_installed,
            get_steam_libraries,
            // Settings commands
            save_api_key,
            get_api_key,
            // SteamGridDB commands
            fetch_steamgriddb_artwork,
            // SLSsteam auto-fetch commands
            fetch_latest_slssteam,
            get_cached_slssteam_version,
            get_cached_slssteam_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
