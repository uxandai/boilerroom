import { useState, useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogCancel,
} from "@/components/ui/alert-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import {
    Loader2,
    Trash2,
    Search,
    Upload,
    Trophy,
    Wifi,
    Zap,
    Cloud,
    RefreshCw,
} from "lucide-react";
import {
    fetchSteamGridDbArtwork,
    getCachedArtworkPath,
    cacheArtwork,
    uninstallGame,
    type InstalledGame,
    type SyncResult,
    getGameCloudStatus,
    type GameCloudStatus,
} from "@/lib/api";
import { formatSize } from "@/lib/utils";
import { CopyToRemoteModal } from "@/components/CopyToRemoteModal";

interface GameCardModalProps {
    isOpen: boolean;
    onClose: () => void;
    game: InstalledGame | null;
    onGameRemoved?: () => void;
}

export function GameCardModal({
    isOpen,
    onClose,
    game,
    onGameRemoved,
}: GameCardModalProps) {
    const {
        sshConfig,
        addLog,
        settings,
        connectionMode,
        setSearchQuery,
        setActiveTab,
        setTriggerSearch,
    } = useAppStore();

    // Artwork state
    const [heroImage, setHeroImage] = useState<string | null>(null);
    const [posterImage, setPosterImage] = useState<string | null>(null);
    const [, setIsLoadingArtwork] = useState(false);

    // Action states
    const [isRemoving, setIsRemoving] = useState(false);
    const [isAddingOnlineFix, setIsAddingOnlineFix] = useState(false);
    const [isGeneratingAchievements, setIsGeneratingAchievements] = useState(false);
    const [isApplyingSteamless, setIsApplyingSteamless] = useState(false);
    const [isSyncing, setIsSyncing] = useState(false);
    const [showCopyModal, setShowCopyModal] = useState(false);
    const [showSyncConfirm, setShowSyncConfirm] = useState(false);
    const [cloudStatus, setCloudStatus] = useState<GameCloudStatus | null>(null);

    // Achievement result dialog state
    const [achievementResult, setAchievementResult] = useState<{
        isOpen: boolean;
        success: boolean;
        title: string;
        message: string;
    }>({ isOpen: false, success: true, title: "", message: "" });

    // Fetch artwork when game changes
    useEffect(() => {
        if (isOpen && game && game.app_id !== "unknown") {
            fetchArtwork();
        } else {
            setHeroImage(null);
            setPosterImage(null);
        }
    }, [isOpen, game?.app_id]);

    // Fetch artwork and cloud status when game changes
    useEffect(() => {
        if (isOpen && game && game.app_id !== "unknown") {
            fetchArtwork();
            getGameCloudStatus(game.app_id).then(setCloudStatus).catch(() => setCloudStatus(null));
        } else {
            setHeroImage(null);
            setPosterImage(null);
            setCloudStatus(null);
        }
    }, [isOpen, game?.app_id]);

    const fetchArtwork = async () => {
        if (!game || game.app_id === "unknown") return;

        setIsLoadingArtwork(true);

        try {
            // Try cache first for hero image
            const cachedPath = await getCachedArtworkPath(game.app_id);
            if (cachedPath) {
                setHeroImage(`asset://localhost/${cachedPath}`);
            } else if (settings.steamGridDbApiKey) {
                // Fetch from SteamGridDB
                const artwork = await fetchSteamGridDbArtwork(
                    settings.steamGridDbApiKey,
                    game.app_id
                );
                if (artwork) {
                    const localPath = await cacheArtwork(game.app_id, artwork);
                    setHeroImage(`asset://localhost/${localPath}`);
                }
            }

            // Use Steam CDN for hero/background
            setHeroImage(
                `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/library_hero.jpg`
            );
            // Poster from Steam CDN
            setPosterImage(
                `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/library_600x900.jpg`
            );
        } catch (error) {
            // Fallback to Steam CDN
            setHeroImage(
                `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/library_hero.jpg`
            );
            setPosterImage(
                `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/library_600x900.jpg`
            );
        } finally {
            setIsLoadingArtwork(false);
        }
    };

    const handleRemoveGame = async () => {
        if (!game) return;
        if (!confirm(`Are you sure you want to uninstall ${game.name}?`)) return;

        setIsRemoving(true);
        try {
            const configForUninstall = { ...sshConfig };
            if (connectionMode === "local") {
                configForUninstall.is_local = true;
            }
            await uninstallGame(configForUninstall, game.path, game.app_id);
            addLog("info", `Uninstalled: ${game.name}`);
            onClose();
            onGameRemoved?.();
        } catch (e) {
            addLog("error", `Uninstall error: ${e}`);
        } finally {
            setIsRemoving(false);
        }
    };

    const handleFindUpdate = () => {
        if (!game) return;
        setSearchQuery(game.app_id !== "unknown" ? game.app_id : game.name);
        setActiveTab("search");
        setTriggerSearch(true);
        onClose();
    };

    const handleCopyToRemote = () => {
        setShowCopyModal(true);
    };

    const handleOnlineFix = async () => {
        if (!game) return;
        setIsAddingOnlineFix(true);
        try {
            const { addFakeAppId } = await import("@/lib/api");
            const configToUse = { ...sshConfig };
            if (connectionMode === "local") {
                configToUse.is_local = true;
            }
            await addFakeAppId(configToUse, game.app_id);
            addLog("info", `Added Online-Fix for ${game.name} (mapped to 480)`);
        } catch (e) {
            addLog("error", `Online-Fix error: ${e}`);
        } finally {
            setIsAddingOnlineFix(false);
        }
    };

    const handleGenerateAchievements = async () => {
        if (!game) return;

        // Check for Steam API key
        if (!settings.steamApiKey) {
            setAchievementResult({
                isOpen: true,
                success: false,
                title: "Configuration Required",
                message: "Steam API Key not configured.\n\nGo to Settings → API Keys to add it."
            });
            return;
        }
        if (!settings.steamUserId) {
            setAchievementResult({
                isOpen: true,
                success: false,
                title: "Configuration Required",
                message: "Steam User ID not configured.\n\nGo to Settings → API Keys to add it."
            });
            return;
        }

        setIsGeneratingAchievements(true);
        try {
            const { generateAchievements } = await import("@/lib/api");
            const result = await generateAchievements(
                game.app_id,
                settings.steamApiKey,
                settings.steamUserId
            );
            addLog("info", `Achievements generated for ${game.name}: ${result}`);
            setAchievementResult({
                isOpen: true,
                success: true,
                title: "Success!",
                message: result
            });
        } catch (e) {
            const errorMsg = e instanceof Error ? e.message : String(e);
            addLog("error", `Achievement generation error: ${errorMsg}`);
            setAchievementResult({
                isOpen: true,
                success: false,
                title: "Failed to Generate Achievements",
                message: errorMsg
            });
        } finally {
            setIsGeneratingAchievements(false);
        }
    };

    const handleApplySteamless = async () => {
        if (!game) return;

        if (!settings.steamlessPath) {
            addLog("error", "Steamless CLI path not configured. Go to Settings → Paths and Tools.");
            return;
        }

        setIsApplyingSteamless(true);
        try {
            const { applySteamlessToGame } = await import("@/lib/api");
            const result = await applySteamlessToGame(game.path, settings.steamlessPath);
            if (result.success) {
                addLog("info", `Steamless: ${result.message}`);
            } else {
                addLog("warn", `Steamless: ${result.message}`);
            }
        } catch (e) {
            addLog("error", `Steamless error: ${e}`);
        } finally {
            setIsApplyingSteamless(false);
        }
    };

    const handleSyncCloud = () => {
        setShowSyncConfirm(true);
    };

    const confirmSync = async () => {
        if (!game) return;
        setIsSyncing(true);
        setShowSyncConfirm(false);
        try {
            const result = await invoke<SyncResult>("sync_game_cloud_saves", { appId: game.app_id });
            
            // Refresh status after sync
            getGameCloudStatus(game.app_id).then(setCloudStatus).catch(() => {});

            setAchievementResult({
                isOpen: true,
                success: result.success,
                title: result.success ? "Cloud Sync Complete" : "Cloud Sync Failed",
                message: result.message,
            });
        } catch (e) {
            setAchievementResult({
                isOpen: true,
                success: false,
                title: "Cloud Sync Failed",
                message: String(e),
            });
        } finally {
            setIsSyncing(false);
        }
    };

    if (!game) return null;

    return (
        <>
            <Dialog open={isOpen && !showCopyModal} onOpenChange={onClose}>
                <DialogContent className="bg-[#1b2838] border-[#0a0a0a] max-w-lg w-full p-0 overflow-hidden">
                    {/* Hero Image Background */}
                    <div className="relative h-48 overflow-hidden">
                        {heroImage ? (
                            <img
                                src={heroImage}
                                alt={game.name}
                                className="w-full h-full object-cover"
                                onError={(e) => {
                                    (e.target as HTMLImageElement).style.display = "none";
                                }}
                            />
                        ) : (
                            <div className="w-full h-full bg-gradient-to-br from-[#2a475e] to-[#1b2838]" />
                        )}
                        {/* Gradient overlay */}
                        <div className="absolute inset-0 bg-gradient-to-t from-[#1b2838] via-transparent to-transparent" />

                        {/* Game info overlay */}
                        <div className="absolute bottom-0 left-0 right-0 p-4 flex items-end gap-4">
                            {/* Poster thumbnail */}
                            {posterImage && (
                                <div className="w-20 h-30 flex-shrink-0 rounded overflow-hidden shadow-lg border border-[#0a0a0a]">
                                    <img
                                        src={posterImage}
                                        alt={game.name}
                                        className="w-full h-full object-cover"
                                        onError={(e) => {
                                            (e.target as HTMLImageElement).style.display = "none";
                                        }}
                                    />
                                </div>
                            )}
                            <div className="flex-1 min-w-0">
                                <h2 className="text-xl font-bold text-white truncate drop-shadow-lg">
                                    {game.name}
                                </h2>
                                <p className="text-sm text-gray-300 drop-shadow">
                                    AppID: {game.app_id} • {formatSize(game.size_bytes)}
                                </p>
                            </div>
                        </div>
                    </div>

                    {/* Action Buttons */}
                    <div className="p-4 space-y-3">
                        <DialogHeader>
                            <DialogTitle className="text-xl font-bold text-white flex items-center gap-3">
                                {game.name}
                                {cloudStatus?.status && cloudStatus.status !== "none" && (
                                     <div className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-[#1b2838] border border-[#2a475e] text-xs font-normal shadow-sm">
                                         <Cloud className={`w-3 h-3 ${cloudStatus.status === "synced" ? "text-[#5ba32b]" : cloudStatus.status === "conflict" ? "text-red-400" : "text-[#67c1f5]"}`} />
                                         <span className="text-gray-300 capitalize flex items-center gap-1">
                                             {cloudStatus.status}
                                             {cloudStatus.source === 'pcgamingwiki' && (
                                                 <span className="text-[10px] text-gray-400 font-mono bg-black/20 px-1 rounded" title="Save location sourced from PCGamingWiki">(PCGW)</span>
                                             )}
                                         </span>
                                     </div>
                                )}
                            </DialogTitle>
                        </DialogHeader>

                        {/* Primary Actions */}
                        <div className="grid grid-cols-2 gap-2">
                            <Button
                                onClick={handleFindUpdate}
                                variant="outline"
                                className="border-[#2a475e] text-white hover:bg-[#2a475e]/50"
                            >
                                <Search className="w-4 h-4 mr-2" />
                                Find Update
                            </Button>

                            <Button
                                onClick={handleSyncCloud}
                                disabled={isSyncing}
                                variant="outline"
                                className="border-[#2a475e] text-[#67c1f5] hover:bg-[#2a475e]/50"
                            >
                                {isSyncing ? (
                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                ) : (
                                    <Cloud className="w-4 h-4 mr-2" />
                                )}
                                Sync Cloud Saves
                            </Button>

                            {connectionMode === "local" && (
                                <Button
                                    onClick={handleCopyToRemote}
                                    variant="outline"
                                    className="border-[#2a475e] text-green-400 hover:bg-green-900/20 hover:text-green-300"
                                >
                                    <Upload className="w-4 h-4 mr-2" />
                                    Copy to Remote
                                </Button>
                            )}

                            <Button
                                onClick={handleOnlineFix}
                                disabled={isAddingOnlineFix}
                                variant="outline"
                                className="border-[#2a475e] text-[#67c1f5] hover:bg-[#2a475e]/50"
                            >
                                {isAddingOnlineFix ? (
                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                ) : (
                                    <Wifi className="w-4 h-4 mr-2" />
                                )}
                                Online-Fix
                            </Button>

                            <Button
                                onClick={handleGenerateAchievements}
                                disabled={isGeneratingAchievements}
                                variant="outline"
                                className="border-[#2a475e] text-yellow-400 hover:bg-yellow-900/20 hover:text-yellow-300"
                            >
                                {isGeneratingAchievements ? (
                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                ) : (
                                    <Trophy className="w-4 h-4 mr-2" />
                                )}
                                Achievements
                            </Button>

                            <Button
                                onClick={handleApplySteamless}
                                disabled={isApplyingSteamless || !settings.steamlessPath}
                                variant="outline"
                                className="border-[#2a475e] text-purple-400 hover:bg-purple-900/20 hover:text-purple-300"
                                title={!settings.steamlessPath ? "Configure Steamless path in Settings" : "Remove DRM using Steamless"}
                            >
                                {isApplyingSteamless ? (
                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                ) : (
                                    <Zap className="w-4 h-4 mr-2" />
                                )}
                                Steamless
                            </Button>
                        </div>

                        {/* Danger Zone */}
                        <div className="pt-2 border-t border-[#2a475e]">
                            <Button
                                onClick={handleRemoveGame}
                                disabled={isRemoving}
                                variant="outline"
                                className="w-full border-red-900/50 text-red-400 hover:bg-red-900/20 hover:text-red-300"
                            >
                                {isRemoving ? (
                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                ) : (
                                    <Trash2 className="w-4 h-4 mr-2" />
                                )}
                                Remove Game
                            </Button>
                        </div>

                        {/* Info text */}
                        <p className="text-xs text-muted-foreground text-center">
                            Path: {game.path}
                        </p>
                    </div>
                </DialogContent>
            </Dialog>

            {/* Nested Copy Modal */}
            <CopyToRemoteModal
                isOpen={showCopyModal}
                onClose={() => setShowCopyModal(false)}
                game={game}
            />

            {/* Sync Confirmation Dialog */}
            <AlertDialog open={showSyncConfirm} onOpenChange={setShowSyncConfirm}>
                <AlertDialogContent className="bg-[#1b2838] border-[#2a475e]">
                    <AlertDialogHeader>
                        <AlertDialogTitle className="text-white">Sync Cloud Saves?</AlertDialogTitle>
                        <AlertDialogDescription className="text-gray-300">
                           This will sync your local saves with the cloud.
                           {cloudStatus?.status === 'conflict' && "\n\nWARNING: Cloud conflict detected. Newer files will overwrite older ones."}
                           {cloudStatus?.status === 'synced' && "\n\nYour saves are already synced."}
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel className="bg-transparent text-gray-300 border-gray-600 hover:bg-gray-800">Cancel</AlertDialogCancel>
                        <AlertDialogAction onClick={confirmSync} className="btn-steam">Sync Now</AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>

            {/* Achievement Result Dialog */}
            <AlertDialog
                open={achievementResult.isOpen}
                onOpenChange={(open) => setAchievementResult(prev => ({ ...prev, isOpen: open }))}
            >
                <AlertDialogContent className="bg-[#1b2838] border-[#2a475e]">
                    <AlertDialogHeader>
                        <AlertDialogTitle className={achievementResult.success ? "text-green-400" : "text-red-400"}>
                            {achievementResult.success ? "✓ " : "✗ "}{achievementResult.title}
                        </AlertDialogTitle>
                        <AlertDialogDescription className="text-gray-300 whitespace-pre-line">
                            {achievementResult.message}
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogAction className="btn-steam">
                            OK
                        </AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </>
    );
}
