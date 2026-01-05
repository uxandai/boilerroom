mod commands;
mod config_vdf;
mod install_manager;
mod steamless;

use commands::*;
use install_manager::InstallManager;
#[cfg(target_os = "linux")]
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

        // Check if SteamOS specifically (Steam Deck Gaming Mode has known WebKit issues)
        let is_steamos = std::path::Path::new("/etc/steamos-release").exists();

        // Apply WebKit fixes for Wayland (DMABUF renderer causes protocol errors)
        if is_wayland {
            eprintln!(
                "[Tauri] Wayland detected, disabling DMABUF renderer for WebKit compatibility"
            );
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }

        // SteamOS needs more aggressive fixes
        if is_steamos {
            eprintln!("[Tauri] SteamOS detected, applying full WebKit compatibility fixes");
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            std::env::set_var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1");
            // Force X11 backend for WebKit compatibility on SteamOS
            std::env::set_var("GDK_BACKEND", "x11");
            // Software rendering fallback for AMD GPU (Steam Deck)
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
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
            cleanup_cancelled_install,
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
            copy_game_to_remote,
            cancel_copy_to_remote,
            // Settings commands
            save_api_key,
            get_api_key,
            // SteamGridDB commands
            fetch_steamgriddb_artwork,
            cache_artwork,
            get_cached_artwork_path,
            clear_artwork_cache,
            // SLSsteam auto-fetch commands
            fetch_latest_slssteam,
            get_cached_slssteam_version,
            get_cached_slssteam_path,
            // Steam update disable and libcurl fix commands
            disable_steam_updates,
            fix_libcurl32,
            check_steam_updates_status,
            check_libcurl32_status,
            check_lib32_dependencies,
            // Depot keys only install command
            install_depot_keys_only,
            // Tools: Steamless & SLSah
            launch_steamless_via_wine,
            check_slsah_installed,
            install_slsah,
            launch_slsah,
            // API status
            check_morrenus_api_status,
            // SLSsteam config management
            add_fake_app_id,
            // add_app_token, // NOTE: AppTokens functionality disabled
            generate_achievements,
            // SteamCMD integration
            steamcmd_get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
