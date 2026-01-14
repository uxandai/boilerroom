/**
 * Setup Wizard hook - manages setup wizard state and progress
 * Uses Zustand store for shared state across components
 */
import { useState, useEffect, useCallback } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "@/store/useAppStore";
import {
    isFirstLaunch,
    runFullSetup,
    resetSetup,
    type SetupState,
    type SetupResult,
} from "@/lib/api/setup";

export interface UseSetupWizardReturn {
    // State
    isOpen: boolean;
    isRunning: boolean;
    setupState: SetupState | null;
    result: SetupResult | null;

    // Actions
    openWizard: () => void;
    closeWizard: () => void;
    startSetup: () => Promise<void>;
    restartSetup: () => Promise<void>;
}

export function useSetupWizard(): UseSetupWizardReturn {
    // Use shared store state for isOpen
    const setupWizardOpen = useAppStore((s) => s.setupWizardOpen);
    const setSetupWizardOpen = useAppStore((s) => s.setSetupWizardOpen);

    // Local state for transient data
    const [isRunning, setIsRunning] = useState(false);
    const [setupState, setSetupState] = useState<SetupState | null>(null);
    const [result, setResult] = useState<SetupResult | null>(null);

    const { connectionMode, sshConfig, addLog } = useAppStore();

    // Check for first launch on mount
    useEffect(() => {
        const checkFirstLaunch = async () => {
            try {
                const firstLaunch = await isFirstLaunch();
                if (firstLaunch) {
                    setSetupWizardOpen(true);
                }
            } catch (err) {
                console.error("Failed to check first launch:", err);
            }
        };

        checkFirstLaunch();
    }, [setSetupWizardOpen]);

    // Listen for setup progress events
    useEffect(() => {
        let unlisten: UnlistenFn | null = null;

        const setupListener = async () => {
            unlisten = await listen<SetupState>("setup_progress", (event) => {
                setSetupState(event.payload);
            });
        };

        setupListener();

        return () => {
            if (unlisten) {
                unlisten();
            }
        };
    }, []);

    const openWizard = useCallback(() => {
        setSetupWizardOpen(true);
        setResult(null);
        setSetupState(null);
    }, [setSetupWizardOpen]);

    const closeWizard = useCallback(() => {
        if (!isRunning) {
            setSetupWizardOpen(false);
        }
    }, [isRunning, setSetupWizardOpen]);

    const startSetup = useCallback(async () => {
        setIsRunning(true);
        setResult(null);
        addLog("info", "Starting setup wizard...");

        try {
            const setupResult = await runFullSetup(
                connectionMode,
                connectionMode === "remote" ? sshConfig : undefined
            );

            setResult(setupResult);

            if (setupResult.success) {
                addLog("info", "Setup completed successfully!");
            } else {
                addLog("error", `Setup failed: ${setupResult.error}`);
            }
        } catch (err) {
            const error = String(err);
            setResult({
                success: false,
                steps_completed: 0,
                error,
            });
            addLog("error", `Setup error: ${error}`);
        } finally {
            setIsRunning(false);
        }
    }, [connectionMode, sshConfig, addLog]);

    const restartSetup = useCallback(async () => {
        try {
            await resetSetup();
            setSetupState(null);
            setResult(null);
            await startSetup();
        } catch (err) {
            addLog("error", `Failed to restart setup: ${err}`);
        }
    }, [startSetup, addLog]);

    return {
        isOpen: setupWizardOpen,
        isRunning,
        setupState,
        result,
        openWizard,
        closeWizard,
        startSetup,
        restartSetup,
    };
}
