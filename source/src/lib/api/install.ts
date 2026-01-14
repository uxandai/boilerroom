import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "@/store/useAppStore";

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

// DepotDownloaderMod commands
export interface DepotInfo {
    depot_id: string;
    name: string;
    manifest_id: string;
    manifest_path: string;
    key: string;
    size: number;
    oslist?: string;
}

export interface GameManifestData {
    app_id: string;
    game_name: string;
    install_dir: string;
    depots: DepotInfo[];
    app_token?: string;
}

/**
 * Extracts and parses a game manifest ZIP file (typically from Depot Provider).
 * 
 * The ZIP is expected to contain LUA configuration scripts which are parsed to extract:
 * - Game Name and App ID.
 * - Depot information (IDs, keys, manifests).
 * - App Tokens (if present).
 * 
 * @param zipPath - Absolute path to the downloaded manifest ZIP file.
 * @returns Parsed game manifest data including depots.
 */
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

/**
 * Starts a pipelined installation process for a game.
 * 
 * This function initiates a complex installation flow that involves:
 * 1. Downloading depots using DepotDownloader.
 * 2. Decrypting and manifesting files.
 * 3. Cleaning DRM using Steamless (if configured).
 * 4. Transferring files to the Steam Deck (if remote).
 * 5. Configuring the Steam library.
 * 
 * @param appId - The Steam Application ID.
 * @param gameName - The readable name of the game.
 * @param depotIds - List of depot IDs to install.
 * @param manifestIds - Corresponding manifest IDs for each depot.
 * @param manifestFiles - Paths to the manifest files.
 * @param depotKeys - Pairs of [depotId, depotKey].
 * @param depotDownloaderPath - Path to the DepotDownloader executable.
 * @param steamlessPath - Path to the Steamless executable.
 * @param sshConfig - SSH configuration for remote operations.
 * @param targetDirectory - Target directory on the Steam Deck or local library.
 * @param appToken - Optional app token (from LUA addtoken) for protected apps.
 */
export async function startPipelinedInstall(
    appId: string,
    gameName: string,
    depotIds: string[],
    manifestIds: string[],
    manifestFiles: string[],
    depotKeys: [string, string][],
    depotDownloaderPath: string,
    steamlessPath: string,
    sshConfig: SshConfig,
    targetDirectory: string,
    appToken?: string
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

// Depot keys only install
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

// Steamless full pipeline result
export interface SteamlessResult {
    success: boolean;
    message: string;
    processed_file?: string;
}

/**
 * Apply Steamless to a game directory to remove DRM.
 * Uses Wine/Proton on Linux to run Steamless.CLI.exe on the main game executable.
 * 
 * @param gamePath - Path to the installed game directory
 * @param steamlessCliPath - Path to Steamless.CLI.exe
 * @returns Result with success status and message
 */
export async function applySteamlessToGame(
    gamePath: string,
    steamlessCliPath: string
): Promise<SteamlessResult> {
    return invoke<SteamlessResult>("apply_steamless_to_game", {
        gamePath,
        steamlessCliPath
    });
}

// Manifest cache commands
export interface ManifestCacheInfo {
    count: number;
    total_size: number;
    path: string;
}

export async function cacheManifest(appId: string, sourcePath: string): Promise<string> {
    return invoke<string>("cache_manifest", { appId, sourcePath });
}

export async function getCachedManifest(appId: string): Promise<string | null> {
    return invoke<string | null>("get_cached_manifest", { appId });
}

export async function clearCachedManifest(appId: string): Promise<void> {
    return invoke<void>("clear_cached_manifest", { appId });
}

export async function clearManifestCache(): Promise<number> {
    return invoke<number>("clear_manifest_cache", {});
}

export async function getManifestCacheInfo(): Promise<ManifestCacheInfo> {
    return invoke<ManifestCacheInfo>("get_manifest_cache_info", {});
}
