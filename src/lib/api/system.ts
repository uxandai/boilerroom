import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "@/store/useAppStore";

// Connection commands
/**
 * Checks if the Steam Deck is online and reachable via SSH.
 * 
 * Tries to establish a TCP connection to the specified IP and Port.
 * 
 * @param ip - IP address of the Steam Deck.
 * @param port - SSH port (usually 22).
 * @returns "online" if reachable, otherwise throws or returns error status.
 */
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

// Clear connection mode (for reset)
export async function clearConnectionMode(): Promise<void> {
    const { Store } = await import("@tauri-apps/plugin-store");
    const store = await Store.load("settings.json");
    await store.delete("connectionMode");
    await store.save();
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

// Check if sshpass is available (needed for rsync password auth)
export async function checkSshpassAvailable(): Promise<boolean> {
    return invoke<boolean>("check_sshpass_available");
}
