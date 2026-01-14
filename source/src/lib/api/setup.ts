/**
 * Setup wizard API functions
 * Handles first-launch detection and unified SLSsteam setup
 */
import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "@/store/useAppStore";

// Setup step status
export type StepStatus = "pending" | "running" | "done" | "error" | "skipped";

// A single setup step's state
export interface SetupStep {
    id: string;
    name: string;
    status: StepStatus;
    message?: string;
}

// Overall setup state
export interface SetupState {
    steps: SetupStep[];
    current_step: number;
    is_complete: boolean;
    error?: string;
}

// Result of running the full setup
export interface SetupResult {
    success: boolean;
    steps_completed: number;
    error?: string;
}

// Steam update handling result
export interface SteamUpdateResult {
    success: boolean;
    steam_updated: boolean;
    slssteam_reapplied: boolean;
    updates_blocked: boolean;
    message: string;
}

/**
 * Check if this is the first launch (no settings saved)
 */
export async function isFirstLaunch(): Promise<boolean> {
    return invoke<boolean>("is_first_launch");
}

/**
 * Mark setup as complete (create marker file)
 */
export async function markSetupComplete(): Promise<void> {
    return invoke<void>("mark_setup_complete");
}

/**
 * Reset setup (remove marker for testing/re-run)
 */
export async function resetSetup(): Promise<void> {
    return invoke<void>("reset_setup");
}

/**
 * Run the full setup wizard
 * Emits 'setup_progress' events with SetupState during execution
 */
export async function runFullSetup(
    mode: "local" | "remote",
    sshConfig?: SshConfig
): Promise<SetupResult> {
    return invoke<SetupResult>("run_full_setup", {
        mode,
        sshConfig: sshConfig || null,
    });
}

/**
 * Handle Steam update with the enter-the-wired pattern:
 * 1. Remove steam.cfg to allow updates
 * 2. Launch Steam with SLSsteam
 * 3. Wait for Steam to finish and exit
 * 4. Recreate steam.cfg to block future updates
 */
export async function handleSteamUpdate(
    config: SshConfig
): Promise<SteamUpdateResult> {
    return invoke<SteamUpdateResult>("handle_steam_update", { config });
}

/**
 * Quick check if Steam updates are currently blocked
 */
export async function areSteamUpdatesBlocked(
    config: SshConfig
): Promise<boolean> {
    return invoke<boolean>("are_steam_updates_blocked", { config });
}
