import { invoke } from "@tauri-apps/api/core";

// Settings commands
export async function saveApiKey(key: string): Promise<void> {
    return invoke<void>("save_api_key", { key });
}

export async function getApiKey(): Promise<string> {
    return invoke<string>("get_api_key");
}

// SteamGridDB commands
export async function fetchSteamGridDbArtwork(
    apiKey: string,
    steamAppId: string
): Promise<string | null> {
    return invoke<string | null>("fetch_steamgriddb_artwork", { apiKey, steamAppId });
}

// Artwork cache commands
export async function cacheArtwork(appId: string, url: string): Promise<string> {
    return invoke<string>("cache_artwork", { appId, url });
}

export async function getCachedArtworkPath(appId: string): Promise<string | null> {
    return invoke<string | null>("get_cached_artwork_path", { appId });
}

export async function clearArtworkCache(): Promise<number> {
    return invoke<number>("clear_artwork_cache");
}

// Morrenus API Status
export interface MorrenusUserStats {
    user_id: string;
    username: string;
    api_key_usage_count: number;
    daily_usage: number;
    daily_limit: number;
    can_make_requests: boolean;
}

export interface MorrenusApiStatus {
    health_ok: boolean;
    user_stats: MorrenusUserStats | null;
    error: string | null;
}

export async function checkMorrenusApiStatus(apiKey: string): Promise<MorrenusApiStatus> {
    return invoke<MorrenusApiStatus>("check_morrenus_api_status", { apiKey });
}

// Achievement Generation
export async function generateAchievements(
    appId: string,
    steamApiKey: string,
    steamUserId: string
): Promise<string> {
    return invoke<string>("generate_achievements", { appId, steamApiKey, steamUserId });
}

// SteamCMD Integration
export interface DepotSteamInfo {
    name?: string;
    oslist?: string;
    size?: number;
}

export interface AppSteamInfo {
    app_id: string;
    name?: string;
    oslist?: string;
    installdir?: string;
    depots: Record<string, DepotSteamInfo>;
}

export async function steamcmdGetAppInfo(appId: string): Promise<AppSteamInfo | null> {
    try {
        return await invoke<AppSteamInfo>("steamcmd_get_app_info", { appId });
    } catch (error) {
        console.log("[SteamCMD] Not available or failed:", error);
        return null;
    }
}

// TOOLS: Steamless & SLSah
// Launch Steamless.exe via Wine/Proton (GUI version)
export async function launchSteamlessViaWine(steamlessExePath: string): Promise<string> {
    return invoke<string>("launch_steamless_via_wine", { steamlessExePath });
}
