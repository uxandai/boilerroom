import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "@/store/useAppStore";

// SLSsteam installation commands
export async function checkReadonlyStatus(config: SshConfig): Promise<boolean> {
    return invoke<boolean>("check_readonly_status", { config });
}

export async function installSlssteam(
    config: SshConfig,
    slssteamPath: string,
    rootPassword: string
): Promise<string> {
    return invoke<string>("install_slssteam", { config, slssteamPath, rootPassword });
}

export interface SlssteamStatus {
    is_readonly: boolean;
    slssteam_so_exists: boolean;
    library_inject_so_exists: boolean;
    config_exists: boolean;
    config_play_not_owned: boolean;
    config_safe_mode_on: boolean;
    steam_sh_patched: boolean;  // headcrab patches steam.sh
    desktop_entry_patched: boolean;
    additional_apps_count: number;
}

/**
 * Verifies the installation status of SLSsteam components on the remote Steam Deck.
 * 
 * Checks for:
 * - Read-only filesystem status.
 * - Existence of `slssteam.so` and `library_inject.so`.
 * - Configuration file presence and validity.
 * - Patch status of `steam_jupiter` and desktop entries.
 * 
 * @param config - SSH connection configuration.
 * @returns A status object detailing which components are installed/configured.
 */
export async function verifySlssteam(config: SshConfig): Promise<SlssteamStatus> {
    return invoke<SlssteamStatus>("verify_slssteam", { config });
}

// SLSsteam auto-fetch commands
export async function fetchLatestSlssteam(): Promise<string> {
    return invoke<string>("fetch_latest_slssteam");
}

export async function getCachedSlssteamVersion(): Promise<string | null> {
    return invoke<string | null>("get_cached_slssteam_version");
}

export async function getCachedSlssteamPath(): Promise<string | null> {
    return invoke<string | null>("get_cached_slssteam_path");
}

// Local SLSsteam verification (for running on Steam Deck itself)
export interface SlssteamLocalStatus {
    slssteam_so_exists: boolean;
    library_inject_so_exists: boolean;
    config_exists: boolean;
    config_play_not_owned: boolean;
    additional_apps_count: number;
    desktop_entry_patched: boolean;
}

export async function verifySlssteamLocal(): Promise<SlssteamLocalStatus> {
    return invoke<SlssteamLocalStatus>("verify_slssteam_local");
}

export async function updateSlssteamConfig(
    config: SshConfig,
    appId: string,
    gameName: string
): Promise<void> {
    return invoke<void>("update_slssteam_config", { config, appId, gameName });
}

// Add FakeAppId mapping for Online-Fix
export async function addFakeAppId(_config: SshConfig, appId: string, fakeAppId: string = "480"): Promise<void> {
    return invoke<void>("add_fake_app_id", { appId, fakeAppId });
}

// Steam updates/hash mismatch prevention
export async function disableSteamUpdates(config: SshConfig): Promise<string> {
    return invoke<string>("disable_steam_updates", { config });
}

export async function enableSteamUpdates(config: SshConfig): Promise<string> {
    return invoke<string>("enable_steam_updates", { config });
}

export async function toggleSteamUpdates(config: SshConfig, disable: boolean): Promise<string> {
    if (disable) {
        return disableSteamUpdates(config);
    } else {
        return enableSteamUpdates(config);
    }
}

// Steam updates status
export interface SteamUpdatesStatus {
    is_configured: boolean;
    inhibit_all: boolean;
    force_self_update_disabled: boolean;
    config_path: string;
}

export async function checkSteamUpdatesStatus(config: SshConfig): Promise<SteamUpdatesStatus> {
    return invoke<SteamUpdatesStatus>("check_steam_updates_status", { config });
}

// libcurl32 symlink status
export interface Libcurl32Status {
    source_exists: boolean;
    symlink_exists: boolean;
    symlink_correct: boolean;
    source_path: string;
    target_path: string;
}

export async function checkLibcurl32Status(config: SshConfig): Promise<Libcurl32Status> {
    return invoke<Libcurl32Status>("check_libcurl32_status", { config });
}

// Fix libcurl32 symlink for Steam
export async function fixLibcurl32(config: SshConfig): Promise<string> {
    return invoke<string>("fix_libcurl32", { config });
}

// 32-bit library dependencies status
export interface Lib32DependenciesStatus {
    lib32_curl_installed: boolean;
    lib32_openssl_installed: boolean;
    lib32_glibc_installed: boolean;
    all_installed: boolean;
}

export async function checkLib32Dependencies(config: SshConfig): Promise<Lib32DependenciesStatus> {
    return invoke<Lib32DependenciesStatus>("check_lib32_dependencies", { config });
}
