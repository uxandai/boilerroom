import { invoke } from "@tauri-apps/api/core";
import type { SshConfig, SearchResult } from "@/store/useAppStore";

// Search commands
export async function searchBundles(query: string): Promise<SearchResult[]> {
    return invoke<SearchResult[]>("search_bundles", { query });
}

// Library management commands
export interface InstalledGame {
    app_id: string;
    name: string;
    path: string;
    size_bytes: number;
    has_depotdownloader_marker: boolean; // true if installed by BoilerRoom/ACCELA
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
/**
 * Copies a game listing from the local machine to the remote Steam Deck.
 * 
 * This uses `rsync` under the hood to transfer game files efficiently.
 * It respects the SSH configuration (password/key) and updates progress.
 * 
 * @param config - SSH connection configuration.
 * @param localPath - Source path on the local machine.
 * @param remotePath - Destination path on the Steam Deck.
 * @param appId - The Steam App ID.
 * @param gameName - The name of the game (for logging/progress).
 */
export async function copyGameToRemote(
    config: SshConfig,
    localPath: string,
    remotePath: string,
    appId: string,
    gameName: string
): Promise<void> {
    return invoke<void>("copy_game_to_remote", { config, localPath, remotePath, appId, gameName });
}
