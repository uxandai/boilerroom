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
  appId: string
): Promise<void> {
  return invoke<void>("update_slssteam_config", { config, appId });
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
}

export interface GameManifestData {
  app_id: string;
  game_name: string;
  install_dir: string;
  depots: DepotInfo[];
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
  targetDirectory: string
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
    targetDirectory
  });
}

export async function cancelInstallation(): Promise<void> {
  return invoke("cancel_installation", {});
}

export async function pauseInstallation(): Promise<void> {
  return invoke("pause_installation", {});
}

export async function resumeInstallation(): Promise<void> {
  return invoke("resume_installation", {});
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

// SteamGridDB commands
export async function fetchSteamGridDbArtwork(
  apiKey: string,
  steamAppId: string
): Promise<string | null> {
  return invoke<string | null>("fetch_steamgriddb_artwork", { apiKey, steamAppId });
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
  config_exists: boolean;
  config_play_not_owned: boolean;
  additional_apps_count: number;
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
