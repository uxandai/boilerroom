import { invoke } from "@tauri-apps/api/core";

export interface CloudSyncConfig {
    enabled: boolean;
    provider: string; // "webdav", "gdrive", "dropbox", "onedrive"
    webdav_url: string;
    username: string;
    password: string;
}

export interface GameCloudStatus {
    app_id: string;
    status: "synced" | "pending" | "syncing" | "conflict" | "error" | "none";
    last_sync?: string | null;
    pending_files?: number | null;
    error_message?: string | null;
    source: "steam_cloud" | "pcgamingwiki" | "none";
}

export interface GlobalCloudStatus {
    enabled: boolean;
    is_syncing: boolean;
    games_synced: number;
    games_pending: number;
    games_with_conflicts: number;
    last_sync?: string | null;
}

export interface SyncResult {
    success: boolean;
    message: string;
    files_uploaded: number;
    files_downloaded: number;
    conflicts: string[];
}

export async function saveCloudSyncConfig(config: CloudSyncConfig): Promise<void> {
    return invoke("save_cloudsync_config", { config });
}

export async function getCloudSyncConfig(): Promise<CloudSyncConfig | null> {
    return invoke("get_cloudsync_config");
}

export async function testCloudSyncConnection(config: CloudSyncConfig): Promise<string> {
    return invoke("test_cloudsync_connection", { config });
}

export async function getGameCloudStatus(appId: string): Promise<GameCloudStatus> {
    return invoke("get_game_cloud_status", { appId });
}

export async function getGlobalCloudStatus(): Promise<GlobalCloudStatus> {
    return invoke("get_global_cloud_status");
}

export async function syncGameCloudSaves(appId: string): Promise<SyncResult> {
    return invoke("sync_game_cloud_saves", { appId });
}

export async function startCloudWatcher(appIds: string[]): Promise<void> {
    return invoke("start_cloud_watcher", { appIds });
}

export async function stopCloudWatcher(): Promise<void> {
    return invoke("stop_cloud_watcher");
}

export async function isCloudWatcherRunning(): Promise<boolean> {
    return invoke("is_cloud_watcher_running");
}
