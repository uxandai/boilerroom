import { invoke } from "@tauri-apps/api/core";

// Settings commands
export async function saveApiKey(key: string): Promise<void> {
    return invoke<void>("save_api_key", { key });
}

export async function getApiKey(): Promise<string> {
    return invoke<string>("get_api_key");
}

// Achievement method settings
export type AchievementMethod = "web_api" | "steam_cm";

export async function saveAchievementMethod(method: AchievementMethod): Promise<void> {
    return invoke<void>("save_achievement_method", { method });
}

export async function getAchievementMethod(): Promise<AchievementMethod> {
    return invoke<AchievementMethod>("get_achievement_method");
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

// Depot Provider API Status
export interface DepotProviderUserStats {
    user_id: string;
    username: string;
    api_key_usage_count: number;
    daily_usage: number;
    daily_limit: number;
    can_make_requests: boolean;
}

export interface DepotProviderApiStatus {
    health_ok: boolean;
    user_stats: DepotProviderUserStats | null;
    error: string | null;
}

export async function checkDepotProviderApiStatus(apiKey: string): Promise<DepotProviderApiStatus> {
    return invoke<DepotProviderApiStatus>("check_depot_provider_api_status", { apiKey });
}

// Achievement Generation
export interface BatchAchievementResult {
    processed: number;
    skipped: number;
    errors: number;
    messages: string[];
}

export async function generateAchievements(
    appId: string,
    steamApiKey: string,
    steamUserId: string
): Promise<string> {
    return invoke<string>("generate_achievements", { appId, steamApiKey, steamUserId });
}

// Generate achievements using Steam CM protocol (SLScheevo method)
// Requires Steam login - will wait for mobile app approval
export async function generateAchievementsCm(
    appId: string,
    steamUsername: string,
    steamPassword: string
): Promise<string> {
    return invoke<string>("generate_achievements_cm", {
        appId,
        steamUsername,
        steamPassword
    });
}

export async function generateAllAchievements(
    steamApiKey: string,
    steamUserId: string
): Promise<BatchAchievementResult> {
    return invoke<BatchAchievementResult>("generate_all_achievements", { steamApiKey, steamUserId });
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
