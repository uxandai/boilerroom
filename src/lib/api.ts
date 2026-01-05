import { invoke } from "@tauri-apps/api/core";
import type { SshConfig, SearchResult } from "@/store/useAppStore";

// Connection commands
export async function checkDeckStatus(ip: string, port: number): Promise<string> {
  return invoke<string>("check_deck_status", { ip, port });
}

export async function testSshConnection(config: SshConfig): Promise<string> {
  return invoke<string>("test_ssh", { config });
}

// SSH config persistence using tauri-plugin-store
export async function saveSshConfig(config: SshConfig): Promise<void> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  await store.set("sshConfig", config);
  await store.save();
}

export async function loadSshConfig(): Promise<SshConfig | null> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  const config = await store.get<SshConfig>("sshConfig");
  return config || null;
}

// Tool settings persistence
export interface ToolSettings {
  depotDownloaderPath: string;
  steamlessPath: string;
  slssteamPath: string;
  steamGridDbApiKey?: string; // Optional SteamGridDB API key
  steamApiKey?: string; // Steam Web API key for achievements
  steamUserId?: string; // Steam User ID for achievements
}

export async function saveToolSettings(settings: ToolSettings): Promise<void> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  await store.set("toolSettings", settings);
  await store.save();
}

export async function loadToolSettings(): Promise<ToolSettings | null> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  const settings = await store.get<ToolSettings>("toolSettings");
  return settings || null;
}

// Search commands
export async function searchBundles(query: string): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_bundles", { query });
}

// Download/Install commands
export interface DownloadProgress {
  bytesDownloaded: number;
  bytesTotal: number;
}

export async function downloadBundle(appId: string): Promise<string> {
  return invoke<string>("download_bundle", { appId });
}

export async function uploadToDeck(
  config: SshConfig,
  localPath: string,
  remotePath: string
): Promise<void> {
  return invoke<void>("upload_to_deck", { config, localPath, remotePath });
}

export async function extractRemote(
  config: SshConfig,
  zipPath: string,
  destDir: string
): Promise<void> {
  return invoke<void>("extract_remote", { config, zipPath, destDir });
}

export async function updateSlssteamConfig(
  config: SshConfig,
  appId: string,
  gameName: string
): Promise<void> {
  return invoke<void>("update_slssteam_config", { config, appId, gameName });
}

// Settings commands
export async function saveApiKey(key: string): Promise<void> {
  return invoke<void>("save_api_key", { key });
}

export async function getApiKey(): Promise<string> {
  return invoke<string>("get_api_key");
}

// DepotDownloaderMod commands
export interface DepotInfo {
  depot_id: string;
  name: string;
  manifest_id: string;
  manifest_path: string;
  key: string;
  size: number;
  oslist?: string; // Parsed from LUA comment: "windows", "linux", "macos"
}

export interface GameManifestData {
  app_id: string;
  game_name: string;
  install_dir: string;
  depots: DepotInfo[];
  app_token?: string; // From addtoken() in LUA - optional
}

export async function extractManifestZip(zipPath: string): Promise<GameManifestData> {
  return invoke<GameManifestData>("extract_manifest_zip", { zipPath });
}

export async function runDepotDownloader(
  depotDownloaderPath: string,
  appId: string,
  depotId: string,
  manifestId: string,
  manifestFile: string,
  depotKey: string,
  outputDir: string
): Promise<string> {
  return invoke<string>("run_depot_downloader", {
    depotDownloaderPath,
    appId,
    depotId,
    manifestId,
    manifestFile,
    depotKey,
    outputDir,
  });
}

export async function getDepotDownloaderPath(): Promise<string> {
  return invoke<string>("get_depot_downloader_path");
}

export async function cleanupTempFiles(appId: string): Promise<void> {
  return invoke<void>("cleanup_temp_files", { appId });
}

// Steamless DRM removal commands
export interface GameExecutable {
  path: string;
  name: string;
  size: number;
  priority: number;
}

export interface InstallProgress {
  state: string;
  message: string;
  files_downloaded: number;
  files_processed: number;
  files_uploaded: number;
  total_bytes_downloaded: number;
  total_bytes_uploaded: number;
}

export async function startPipelinedInstall(
  appId: string,
  gameName: string,
  depotIds: string[],
  manifestIds: string[],
  manifestFiles: string[],
  depotKeys: [string, string][], // [depot_id, key] pairs
  depotDownloaderPath: string,
  steamlessPath: string,
  sshConfig: SshConfig,
  targetDirectory: string,
  appToken?: string // Optional app token from LUA addtoken()
): Promise<void> {
  return invoke("start_pipelined_install", {
    appId,
    gameName,
    depotIds,
    manifestIds,
    manifestFiles,
    depotKeys,
    depotDownloaderPath,
    steamlessPath,
    sshConfig,
    targetDirectory,
    appToken: appToken || null
  });
}

export async function cancelInstallation(): Promise<void> {
  return invoke("cancel_installation", {});
}

export async function cancelCopyToRemote(): Promise<void> {
  return invoke("cancel_copy_to_remote", {});
}

export async function pauseInstallation(): Promise<void> {
  return invoke("pause_installation", {});
}

export async function resumeInstallation(): Promise<void> {
  return invoke("resume_installation", {});
}

export async function cleanupCancelledInstall(
  appId: string,
  gameName: string,
  libraryPath: string,
  sshConfig: SshConfig
): Promise<string> {
  return invoke<string>("cleanup_cancelled_install", {
    appId,
    gameName,
    libraryPath,
    sshConfig
  });
}

export async function findGameExecutables(
  gameDirectory: string,
  gameName: string
): Promise<GameExecutable[]> {
  return invoke<GameExecutable[]>("find_game_executables", { gameDirectory, gameName });
}

export async function runSteamless(
  steamlessPath: string,
  exePath: string
): Promise<string> {
  return invoke<string>("run_steamless", { steamlessPath, exePath });
}

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
  steam_jupiter_patched: boolean;
  desktop_entry_patched: boolean;
  additional_apps_count: number;
}

export async function verifySlssteam(config: SshConfig): Promise<SlssteamStatus> {
  return invoke<SlssteamStatus>("verify_slssteam", { config });
}

// Library management commands
export interface InstalledGame {
  app_id: string;
  name: string;
  path: string;
  size_bytes: number;
  has_depotdownloader_marker: boolean; // true if installed by TonTonDeck/ACCELA
  header_image?: string;
}

export async function listInstalledGames(config: SshConfig): Promise<InstalledGame[]> {
  return invoke<InstalledGame[]>("list_installed_games", { config });
}

export async function listInstalledGamesLocal(): Promise<InstalledGame[]> {
  return invoke<InstalledGame[]>("list_installed_games_local");
}

export async function uninstallGame(
  config: SshConfig,
  gamePath: string,
  appId: string
): Promise<string> {
  return invoke<string>("uninstall_game", { config, gamePath, appId });
}

export interface InstalledDepot {
  depot_id: string;
  manifest_id: string;
}

export async function checkGameInstalled(
  config: SshConfig,
  appId: string
): Promise<InstalledDepot[]> {
  return invoke<InstalledDepot[]>("check_game_installed", { config, appId });
}

export async function checkGameUpdate(
  appId: string
): Promise<boolean> {
  return invoke<boolean>("check_game_update", { appId });
}

export async function getSteamLibraries(
  config: SshConfig
): Promise<string[]> {
  return invoke<string[]>("get_steam_libraries", { config });
}

// Copy game from local to remote via rsync
export async function copyGameToRemote(
  config: SshConfig,
  localPath: string,
  remotePath: string,
  appId: string,
  gameName: string
): Promise<void> {
  return invoke<void>("copy_game_to_remote", { config, localPath, remotePath, appId, gameName });
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

// Connection mode persistence
export async function saveConnectionMode(mode: "local" | "remote"): Promise<void> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  await store.set("connectionMode", mode);
  await store.save();
}

export async function loadConnectionMode(): Promise<"local" | "remote" | null> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  const mode = await store.get<"local" | "remote">("connectionMode");
  return mode || null;
}

// Steam Deck detection
export interface SteamDeckDetection {
  is_steam_deck: boolean;
  is_steamos: boolean;
  os_name: string;
}

export async function detectSteamDeck(): Promise<SteamDeckDetection> {
  return invoke<SteamDeckDetection>("detect_steam_deck");
}

// Clear connection mode (for reset)
export async function clearConnectionMode(): Promise<void> {
  const { Store } = await import("@tauri-apps/plugin-store");
  const store = await Store.load("settings.json");
  await store.delete("connectionMode");
  await store.save();
}

// Check if sshpass is available (needed for rsync password auth)
export async function checkSshpassAvailable(): Promise<boolean> {
  return invoke<boolean>("check_sshpass_available");
}

// Disable Steam updates to prevent SLSsteam hash mismatch
export async function disableSteamUpdates(config: SshConfig): Promise<string> {
  return invoke<string>("disable_steam_updates", { config });
}

// Fix libcurl32 symlink for Steam
export async function fixLibcurl32(config: SshConfig): Promise<string> {
  return invoke<string>("fix_libcurl32", { config });
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

// 32-bit library dependencies status (required for Steam)
export interface Lib32DependenciesStatus {
  lib32_curl_installed: boolean;
  lib32_openssl_installed: boolean;
  lib32_glibc_installed: boolean;
  all_installed: boolean;
}

export async function checkLib32Dependencies(config: SshConfig): Promise<Lib32DependenciesStatus> {
  return invoke<Lib32DependenciesStatus>("check_lib32_dependencies", { config });
}

// Depot keys only install (no download) - configures Steam to recognize game
export interface DepotKeyInfo {
  depot_id: string;
  manifest_id: string;
  manifest_path: string;
  key: string;
}

export async function installDepotKeysOnly(
  appId: string,
  gameName: string,
  depots: DepotKeyInfo[],
  sshConfig: SshConfig,
  targetLibrary: string,
  triggerSteamInstall: boolean = false
): Promise<string> {
  return invoke<string>("install_depot_keys_only", {
    appId,
    gameName,
    depots,
    sshConfig,
    targetLibrary,
    triggerSteamInstall
  });
}

// ============================================================================
// TOOLS: Steamless & SLSah
// ============================================================================

// Launch Steamless.exe via Wine/Proton (GUI version)
export async function launchSteamlessViaWine(steamlessExePath: string): Promise<string> {
  return invoke<string>("launch_steamless_via_wine", { steamlessExePath });
}

// Check if SLSah is installed
export async function checkSlsahInstalled(): Promise<boolean> {
  return invoke<boolean>("check_slsah_installed");
}

// Install SLSah (SLSsteam Achievement Helper)
export async function installSlsah(): Promise<string> {
  return invoke<string>("install_slsah");
}

// Launch SLSah in a terminal
export async function launchSlsah(): Promise<string> {
  return invoke<string>("launch_slsah");
}

// ============================================================================
// API STATUS
// ============================================================================

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

// ============================================================================
// SLSSTEAM CONFIG MANAGEMENT
// ============================================================================

// Add FakeAppId mapping for Online-Fix (maps to 480 for Spacewar networking)
// Note: _config is currently unused (local operation only) but kept in API for future remote support
export async function addFakeAppId(_config: SshConfig, appId: string, fakeAppId: string = "480"): Promise<void> {
  return invoke<void>("add_fake_app_id", { appId, fakeAppId });
}

// NOTE: AppTokens functionality disabled - not needed for current workflow
// // Add AppToken entry to SLSsteam config
// export async function addAppToken(config: SshConfig, appId: string, token: string): Promise<void> {
//   return invoke<void>("add_app_token", { config, appId, token });
// }

// ============================================================================
// ACHIEVEMENT GENERATION
// ============================================================================

// Generate achievement schema files for a game
export async function generateAchievements(
  appId: string,
  steamApiKey: string,
  steamUserId: string
): Promise<string> {
  return invoke<string>("generate_achievements", { appId, steamApiKey, steamUserId });
}

// ============================================================================
// STEAMCMD INTEGRATION
// ============================================================================

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

// Get app/depot info from SteamCMD (optional, fails gracefully)
export async function steamcmdGetAppInfo(appId: string): Promise<AppSteamInfo | null> {
  try {
    return await invoke<AppSteamInfo>("steamcmd_get_app_info", { appId });
  } catch (error) {
    console.log("[SteamCMD] Not available or failed:", error);
    return null;
  }
}
