import { useState, useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import {
    AlertCircle,
    Check,
    RefreshCw,
    Wrench,
    Shield,
    FolderOpen,
    ChevronDown,
    ChevronUp,
    Activity
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

// Interfaces
interface Lib32Status {
    lib32_curl_installed: boolean;
    lib32_openssl_installed: boolean;
    lib32_glibc_installed: boolean;
    all_installed: boolean;
}

interface SteamUpdatesStatus {
    is_configured: boolean; // steam.cfg exists
    inhibit_all: boolean;
    force_self_update_disabled: boolean;
}

interface Libcurl32Status {
    source_exists: boolean;
    symlink_exists: boolean;
    symlink_correct: boolean;
}

interface VerifyStatus {
    is_readonly: boolean;
    slssteam_so_exists: boolean;
    library_inject_so_exists: boolean;
    config_exists: boolean;
    config_safe_mode_on: boolean;
    steam_jupiter_patched: boolean;
    desktop_entry_patched: boolean;
    additional_apps_count: number;
}

export function SystemHealthPanel() {
    const { settings, setSettings, connectionMode, sshConfig, addLog } = useAppStore();
    const [isExpanded, setIsExpanded] = useState(false);

    // States
    const [lib32Status, setLib32Status] = useState<Lib32Status | null>(null);
    const [steamUpdatesStatus, setSteamUpdatesStatus] = useState<SteamUpdatesStatus | null>(null);
    const [libcurl32Status, setLibcurl32Status] = useState<Libcurl32Status | null>(null);
    const [verifyStatus, setVerifyStatus] = useState<VerifyStatus | null>(null);

    // Loading states
    const [loading, setLoading] = useState({
        lib32: false,
        updates: false,
        libcurl: false,
        slssteam: false,
        togglingUpdates: false,
        allowingUpdate: false,
        fixingLibcurl: false
    });

    // Auto-check on mount
    useEffect(() => {
        refreshAll();
    }, [connectionMode]);

    const refreshAll = () => {
        handleCheckUpdates();
        handleCheckSlssteam();
        if (connectionMode === "local") {
            handleCheckLib32();
            handleCheckLibcurl32();
        }
    };

    // --- Handlers ---

    const handleCheckLib32 = async () => {
        if (connectionMode !== "local") return;
        setLoading(prev => ({ ...prev, lib32: true }));
        try {
            const { checkLib32Dependencies } = await import("@/lib/api");
            const config = { ...sshConfig, is_local: true };
            const status = await checkLib32Dependencies(config);
            setLib32Status(status);
        } catch (e) { /* quiet */ }
        finally { setLoading(prev => ({ ...prev, lib32: false })); }
    };

    const handleCheckUpdates = async () => {
        setLoading(prev => ({ ...prev, updates: true }));
        try {
            const { checkSteamUpdatesStatus } = await import("@/lib/api");
            const config = connectionMode === "local" ? { ...sshConfig, is_local: true } : sshConfig;
            if (connectionMode === "remote" && (!config.ip || !config.password)) return;
            const status = await checkSteamUpdatesStatus(config);
            setSteamUpdatesStatus(status);
        } catch (e) { /* quiet */ }
        finally { setLoading(prev => ({ ...prev, updates: false })); }
    };

    const handleCheckLibcurl32 = async () => {
        if (connectionMode !== "local") return;
        setLoading(prev => ({ ...prev, libcurl: true }));
        try {
            const { checkLibcurl32Status } = await import("@/lib/api");
            const config = { ...sshConfig, is_local: true };
            const status = await checkLibcurl32Status(config);
            setLibcurl32Status(status);
        } catch (e) { /* quiet */ }
        finally { setLoading(prev => ({ ...prev, libcurl: false })); }
    };

    const handleCheckSlssteam = async () => {
        setLoading(prev => ({ ...prev, slssteam: true }));
        try {
            const configToUse = { ...sshConfig };
            if (connectionMode === "local") {
                configToUse.is_local = true;
                const { verifySlssteamLocal } = await import("@/lib/api");
                const status = await verifySlssteamLocal();
                setVerifyStatus({
                    is_readonly: false,
                    slssteam_so_exists: status.slssteam_so_exists,
                    library_inject_so_exists: status.library_inject_so_exists || false,
                    config_exists: status.config_exists,
                    config_safe_mode_on: false,
                    steam_jupiter_patched: false,
                    desktop_entry_patched: status.desktop_entry_patched || false,
                    additional_apps_count: status.additional_apps_count,
                });
            } else {
                if (!sshConfig.ip) return;
                const { verifySlssteam } = await import("@/lib/api");
                const status = await verifySlssteam(sshConfig);
                setVerifyStatus(status);
            }
        } catch (e) { /* quiet */ }
        finally { setLoading(prev => ({ ...prev, slssteam: false })); }
    };

    const handleToggleUpdates = async () => {
        setLoading(prev => ({ ...prev, togglingUpdates: true }));
        const action = steamUpdatesStatus?.is_configured ? "enable" : "disable";
        try {
            const { toggleSteamUpdates } = await import("@/lib/api");
            const config = connectionMode === "local" ? { ...sshConfig, is_local: true } : sshConfig;
            await toggleSteamUpdates(config, action === "disable");
            addLog("info", `Steam updates ${action}d successfully`);
            await handleCheckUpdates();
        } catch (e) { addLog("error", `Failed: ${e}`); }
        finally { setLoading(prev => ({ ...prev, togglingUpdates: false })); }
    };

    const handleAllowUniqueUpdate = async () => {
        setLoading(prev => ({ ...prev, allowingUpdate: true }));
        try {
            const { handleSteamUpdate } = await import("@/lib/api/setup");
            const config = connectionMode === "local" ? { ...sshConfig, is_local: true } : sshConfig;
            const result = await handleSteamUpdate(config);
            addLog("info", `Result: ${result.message}`);
            await handleCheckUpdates();
        } catch (e) { addLog("error", `Failed: ${e}`); }
        finally { setLoading(prev => ({ ...prev, allowingUpdate: false })); }
    };

    const handleFixLibcurl32 = async () => {
        setLoading(prev => ({ ...prev, fixingLibcurl: true }));
        try {
            const { fixLibcurl32 } = await import("@/lib/api");
            const config = { ...sshConfig, is_local: true };
            await fixLibcurl32(config);
            addLog("info", "libcurl32 fixed successfully");
            await handleCheckLibcurl32();
        } catch (e) { addLog("error", `Failed: ${e}`); }
        finally { setLoading(prev => ({ ...prev, fixingLibcurl: false })); }
    };

    const handleBrowseDepotDownloader = async () => {
        try {
            const selected = await open({ multiple: false, directory: false, title: "Select DepotDownloaderMod binary" });
            if (selected) {
                const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
                setSettings({ depotDownloaderPath: path });
            }
        } catch (error) { addLog("error", `Failed to select file: ${error}`); }
    };

    const handleBrowseSteamless = async () => {
        try {
            const selected = await open({ multiple: false, directory: false, filters: [{ name: "Steamless", extensions: ["exe"] }], title: "Select Steamless.exe" });
            if (selected) {
                const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
                setSettings({ steamlessPath: path });
            }
        } catch (error) { addLog("error", `Failed to select file: ${error}`); }
    };

    // Computed Health Status
    const isHealthy =
        (steamUpdatesStatus?.is_configured ?? false) &&
        (verifyStatus?.slssteam_so_exists ?? false) &&
        (connectionMode !== "local" || ((lib32Status?.all_installed ?? false) && (libcurl32Status?.symlink_correct ?? false)));

    return (
        <Card className="bg-[#1b2838] border-[#2a475e]">
            <CardHeader className="pb-3 cursor-pointer select-none" onClick={() => setIsExpanded(!isExpanded)}>
                <div className="flex items-center justify-between">
                    <CardTitle className="text-white flex items-center gap-2">
                        <Activity className="w-5 h-5 text-[#67c1f5]" />
                        System Health & Tools
                    </CardTitle>
                    <div className="flex items-center gap-3">
                        <div className={`flex items-center gap-2 text-sm px-3 py-1 rounded-full border ${isHealthy ? "bg-green-900/30 border-green-800 text-green-400" : "bg-yellow-900/30 border-yellow-800 text-yellow-400"}`}>
                            {isHealthy ? <Check className="w-3 h-3" /> : <AlertCircle className="w-3 h-3" />}
                            {isHealthy ? "Healthy" : "Attention Needed"}
                        </div>
                        {isExpanded ? <ChevronUp className="w-5 h-5 text-gray-400" /> : <ChevronDown className="w-5 h-5 text-gray-400" />}
                    </div>
                </div>
                <CardDescription>System dependencies, SLSteam status, and external tool paths.</CardDescription>
            </CardHeader>

            {isExpanded && (
                <CardContent className="space-y-6 pt-0 animate-in slide-in-from-top-2 duration-200">
                    {/* Steam Updates */}
                    <div className="space-y-2 pt-4 border-t border-[#2a475e]">
                        <div className="flex justify-between items-center text-sm font-medium text-gray-300">
                            <span className="flex items-center gap-2"><Shield className="w-4 h-4" /> Steam Updates & SLSteam</span>
                            <Button size="sm" variant="ghost" onClick={handleCheckUpdates} disabled={loading.updates} className="h-6 w-6 p-0">
                                <RefreshCw className={`w-3 h-3 ${loading.updates ? "animate-spin" : ""}`} />
                            </Button>
                        </div>

                        <div className="flex items-center justify-between bg-[#0a0a0a] p-3 rounded border border-[#2a475e]">
                            <div className="text-sm">
                                <div className={steamUpdatesStatus?.is_configured ? "text-green-400" : "text-red-400"}>
                                    {steamUpdatesStatus?.is_configured ? "Updates Blocked (Safe)" : "Updates Enabled (Risk)"}
                                </div>
                            </div>
                            <div className="flex gap-2">
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="h-7 text-xs border-[#2a475e]"
                                    onClick={handleToggleUpdates}
                                    disabled={loading.togglingUpdates}
                                >
                                    {steamUpdatesStatus?.is_configured ? "Unblock" : "Block"}
                                </Button>
                                {steamUpdatesStatus?.is_configured && (
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="h-7 text-xs border-[#2a475e]"
                                        onClick={handleAllowUniqueUpdate}
                                        disabled={loading.allowingUpdate}
                                    >
                                        Allow One
                                    </Button>
                                )}
                            </div>
                        </div>

                        {/* SLSteam Check */}
                        <div className="bg-[#0a0a0a] p-3 rounded border border-[#2a475e] text-sm">
                            <div className="flex justify-between items-center mb-2">
                                <span className={verifyStatus?.slssteam_so_exists ? "text-green-400" : "text-red-400"}>
                                    SLSteam: {verifyStatus?.slssteam_so_exists ? "Installed" : "Missing/Broken"}
                                </span>
                                <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={handleCheckSlssteam} disabled={loading.slssteam}>
                                    <RefreshCw className={`w-3 h-3 ${loading.slssteam ? "animate-spin" : ""}`} />
                                </Button>
                            </div>
                            {verifyStatus && (
                                <div className="grid grid-cols-1 gap-y-1 text-xs text-gray-400 mt-2">
                                    <div className="flex justify-between">
                                        <span>Config (SLS config.yaml):</span>
                                        <span className={verifyStatus.config_exists ? "text-green-400" : "text-red-400"}>
                                            {verifyStatus.config_exists ? "OK" : "Missing"}
                                        </span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>library-inject.so:</span>
                                        <span className={verifyStatus.library_inject_so_exists ? "text-green-400" : "text-red-400"}>
                                            {verifyStatus.library_inject_so_exists ? "OK" : "Missing"}
                                        </span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>steam-jupiter.patch:</span>
                                        <span className={verifyStatus.steam_jupiter_patched ? "text-green-400" : "text-gray-500"}>
                                            {verifyStatus.steam_jupiter_patched ? "OK" : "N/A"}
                                        </span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Desktop Entry:</span>
                                        <span className={verifyStatus.desktop_entry_patched ? "text-green-400" : "text-red-400"}>
                                            {verifyStatus.desktop_entry_patched ? "OK" : "Missing"}
                                        </span>
                                    </div>
                                    {connectionMode === "remote" && (
                                        <div className="flex justify-between">
                                            <span>Filesystem Read-only:</span>
                                            <span className={verifyStatus.is_readonly ? "text-yellow-400" : "text-green-400"}>
                                                {verifyStatus.is_readonly ? "Yes" : "No"}
                                            </span>
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Local Dependencies */}
                    {connectionMode === "local" && (
                        <div className="space-y-2 pt-4 border-t border-[#2a475e]">
                            <div className="flex justify-between items-center text-sm font-medium text-gray-300">
                                <span className="flex items-center gap-2"><Wrench className="w-4 h-4" /> Local Dependencies</span>
                                <Button size="sm" variant="ghost" onClick={handleCheckLib32} disabled={loading.lib32} className="h-6 w-6 p-0">
                                    <RefreshCw className={`w-3 h-3 ${loading.lib32 ? "animate-spin" : ""}`} />
                                </Button>
                            </div>

                            <div className="grid grid-cols-3 gap-2 text-xs">
                                <StatusPill ok={lib32Status?.lib32_curl_installed ?? false} label="lib32-curl" />
                                <StatusPill ok={lib32Status?.lib32_openssl_installed ?? false} label="lib32-openssl" />
                                <StatusPill ok={lib32Status?.lib32_glibc_installed ?? false} label="lib32-glibc" />
                            </div>

                            {/* Libcurl32 Fix */}
                            <div className="bg-[#0a0a0a] p-3 rounded border border-[#2a475e] flex justify-between items-center mt-2">
                                <div className="text-xs">
                                    <div className={libcurl32Status?.symlink_correct ? "text-green-400" : "text-yellow-400"}>
                                        libcurl Compat: {libcurl32Status?.symlink_correct ? "OK" : "Fix Needed"}
                                    </div>
                                </div>
                                {!libcurl32Status?.symlink_correct && (
                                    <Button
                                        size="sm"
                                        variant="secondary"
                                        className="h-7 text-xs"
                                        onClick={handleFixLibcurl32}
                                        disabled={loading.fixingLibcurl}
                                    >
                                        Fix
                                    </Button>
                                )}
                            </div>
                        </div>
                    )}

                    {/* Tools Paths */}
                    <div className="space-y-2 pt-4 border-t border-[#2a475e]">
                        <Label className="flex items-center gap-2"><FolderOpen className="w-4 h-4" /> Tool Paths</Label>

                        <div className="grid gap-2">
                            <div className="flex gap-2">
                                <Input value={settings.depotDownloaderPath || ""} placeholder="DepotDownloaderMod Path" readOnly className="bg-[#0a0a0a] border-[#2a475e] text-xs h-8" />
                                <Button onClick={handleBrowseDepotDownloader} variant="secondary" size="sm" className="h-8 w-8 p-0"><FolderOpen className="w-3 h-3" /></Button>
                            </div>
                            <div className="flex gap-2">
                                <Input value={settings.steamlessPath || ""} placeholder="Steamless.exe Path" readOnly className="bg-[#0a0a0a] border-[#2a475e] text-xs h-8" />
                                <Button onClick={handleBrowseSteamless} variant="secondary" size="sm" className="h-8 w-8 p-0"><FolderOpen className="w-3 h-3" /></Button>
                            </div>
                        </div>
                    </div>
                </CardContent>
            )}
        </Card>
    );
}

function StatusPill({ ok, label }: { ok: boolean; label: string }) {
    return (
        <div className={`px-2 py-1 rounded text-center border ${ok ? "bg-green-900/20 border-green-800 text-green-400" : "bg-red-900/20 border-red-800 text-red-400"}`}>
            {ok ? "✓" : "✗"} {label}
        </div>
    );
}
