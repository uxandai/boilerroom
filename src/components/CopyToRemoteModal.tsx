import { useState, useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Loader2, Upload, HardDrive, FolderOpen, Wifi, Check, AlertCircle } from "lucide-react";
import { getSteamLibraries, testSshConnection, copyGameToRemote, type InstalledGame } from "@/lib/api";

interface CopyToRemoteModalProps {
    isOpen: boolean;
    onClose: () => void;
    game: InstalledGame | null;
}

export function CopyToRemoteModal({ isOpen, onClose, game }: CopyToRemoteModalProps) {
    const { sshConfig, setSshConfig, addLog, setInstallProgress } = useAppStore();

    const [libraries, setLibraries] = useState<string[]>([]);
    const [selectedLibrary, setSelectedLibrary] = useState<string>("");
    const [isLoadingLibraries, setIsLoadingLibraries] = useState(false);

    // Local SSH config for modal (in case global config is empty)
    const [localIp, setLocalIp] = useState("");
    const [localUsername, setLocalUsername] = useState("deck");
    const [localPassword, setLocalPassword] = useState("");
    const [isConnecting, setIsConnecting] = useState(false);
    const [isConnected, setIsConnected] = useState(false);
    const [connectionError, setConnectionError] = useState<string | null>(null);

    // Check if we have SSH credentials
    const hasSshCredentials = Boolean(sshConfig.ip && sshConfig.password);

    // Reset state when modal opens
    useEffect(() => {
        if (isOpen) {
            setLocalIp(sshConfig.ip || "");
            setLocalUsername(sshConfig.username || "deck");
            setLocalPassword(sshConfig.password || "");
            setIsConnected(hasSshCredentials);
            setConnectionError(null);
            setLibraries([]);
            setSelectedLibrary("");

            // If already have credentials, load libraries
            if (hasSshCredentials) {
                loadRemoteLibraries();
            }
        }
    }, [isOpen]);

    const handleConnect = async () => {
        if (!localIp || !localPassword) {
            setConnectionError("IP and password are required");
            return;
        }

        setIsConnecting(true);
        setConnectionError(null);

        try {
            const tempConfig = {
                ...sshConfig,
                ip: localIp,
                username: localUsername,
                password: localPassword,
            };

            await testSshConnection(tempConfig);

            // Save the config globally
            setSshConfig({
                ip: localIp,
                username: localUsername,
                password: localPassword,
            });

            setIsConnected(true);
            addLog("info", `Connected to ${localIp}`);

            // Load libraries after successful connection
            loadRemoteLibraries(tempConfig);
        } catch (error) {
            setConnectionError(`Connection failed: ${error}`);
            addLog("error", `SSH connection failed: ${error}`);
        } finally {
            setIsConnecting(false);
        }
    };

    // Load remote Steam libraries
    const loadRemoteLibraries = async (configOverride?: typeof sshConfig) => {
        const config = configOverride || sshConfig;
        if (!config.ip || !config.password) return;

        setIsLoadingLibraries(true);
        try {
            const libs = await getSteamLibraries(config);
            // Sort: internal first (paths containing .steam), then SD cards
            const sorted = libs.sort((a, b) => {
                const aIsInternal = a.includes('.steam') || (!a.includes('mmcblk') && !a.includes('media'));
                const bIsInternal = b.includes('.steam') || (!b.includes('mmcblk') && !b.includes('media'));
                if (aIsInternal && !bIsInternal) return -1;
                if (!aIsInternal && bIsInternal) return 1;
                return 0;
            });
            setLibraries(sorted);
            if (sorted.length > 0) {
                setSelectedLibrary(sorted[0]);
            }
        } catch (error) {
            addLog("error", `Failed to load remote Steam libraries: ${error}`);
        } finally {
            setIsLoadingLibraries(false);
        }
    };

    const formatSize = (bytes: number): string => {
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
        if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
        return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    };

    const handleCopy = async () => {
        if (!game || !selectedLibrary) return;

        // Close dialog immediately - copy continues in background
        onClose();

        addLog("info", `Starting copy of ${game.name} to remote: ${selectedLibrary}`);

        try {
            const targetPath = `${selectedLibrary}/steamapps/common`;

            // Set install progress to show the progress banner
            setInstallProgress({
                step: "transferring",
                appId: game.app_id,
                gameName: game.name,
                heroImage: `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/library_hero.jpg`,
                downloadPercent: 0,
                downloadSpeed: "",
                eta: "calculating...",
                filesTotal: 0,
                filesTransferred: 0,
                bytesTotal: game.size_bytes,
                bytesTransferred: 0,
                transferSpeed: "",
                message: "Copying to Steam Deck..."
            });

            await copyGameToRemote(sshConfig, game.path, targetPath, game.app_id, game.name);

            addLog("info", `Successfully copied ${game.name} to Steam Deck`);
        } catch (error) {
            addLog("error", `Copy failed: ${error}`);
            setInstallProgress({
                step: "error",
                appId: game.app_id,
                gameName: game.name,
                downloadPercent: 0,
                downloadSpeed: "",
                eta: "",
                filesTotal: 0,
                filesTransferred: 0,
                bytesTotal: 0,
                bytesTransferred: 0,
                transferSpeed: "",
                error: `Copy failed: ${error}`
            });
        }
    };

    if (!game) return null;

    return (
        <Dialog open={isOpen} onOpenChange={onClose}>
            <DialogContent className="bg-[#1b2838] border-[#0a0a0a] max-w-md w-full overflow-hidden">
                <DialogHeader>
                    <DialogTitle className="text-white flex items-center gap-2 pr-6">
                        <Upload className="w-5 h-5 flex-shrink-0" />
                        <span className="truncate">Copy to Steam Deck: {game.name}</span>
                    </DialogTitle>
                </DialogHeader>

                <div className="space-y-4 py-4 overflow-hidden">
                    {/* Game Info */}
                    <div className="bg-[#171a21] border border-[#0a0a0a] p-3 flex items-center gap-3 overflow-hidden">
                        <div className="w-14 h-14 bg-[#2a475e] rounded flex items-center justify-center flex-shrink-0">
                            <FolderOpen className="w-6 h-6 text-muted-foreground" />
                        </div>
                        <div className="min-w-0 flex-1">
                            <p className="font-medium text-white truncate">{game.name}</p>
                            <p className="text-sm text-muted-foreground">
                                AppID: {game.app_id} • {formatSize(game.size_bytes)}
                            </p>
                            <p className="text-xs text-muted-foreground truncate" title={game.path}>
                                From: {game.path}
                            </p>
                        </div>
                    </div>

                    {/* SSH Connection Section - show if not connected */}
                    {!isConnected && (
                        <div className="space-y-3 bg-[#2a475e] border border-[#1b2838] p-4 rounded">
                            <div className="flex items-center gap-2 text-white font-medium">
                                <Wifi className="w-4 h-4" />
                                Connect to Steam Deck
                            </div>

                            <div className="space-y-2">
                                <div>
                                    <Label className="text-sm text-muted-foreground">IP Address</Label>
                                    <Input
                                        value={localIp}
                                        onChange={(e) => setLocalIp(e.target.value)}
                                        placeholder="192.168.1.100"
                                        className="bg-[#171a21] border-[#0a0a0a]"
                                    />
                                </div>
                                <div>
                                    <Label className="text-sm text-muted-foreground">Username</Label>
                                    <Input
                                        value={localUsername}
                                        onChange={(e) => setLocalUsername(e.target.value)}
                                        placeholder="deck"
                                        className="bg-[#171a21] border-[#0a0a0a]"
                                    />
                                </div>
                                <div>
                                    <Label className="text-sm text-muted-foreground">Password</Label>
                                    <Input
                                        type="password"
                                        value={localPassword}
                                        onChange={(e) => setLocalPassword(e.target.value)}
                                        placeholder="SSH password"
                                        className="bg-[#171a21] border-[#0a0a0a]"
                                    />
                                </div>
                            </div>

                            {connectionError && (
                                <div className="flex items-center gap-2 text-red-400 text-sm">
                                    <AlertCircle className="w-4 h-4" />
                                    {connectionError}
                                </div>
                            )}

                            <Button
                                onClick={handleConnect}
                                disabled={isConnecting || !localIp || !localPassword}
                                className="w-full btn-steam"
                            >
                                {isConnecting ? (
                                    <>
                                        <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                        Connecting...
                                    </>
                                ) : (
                                    <>
                                        <Wifi className="w-4 h-4 mr-2" />
                                        Connect
                                    </>
                                )}
                            </Button>
                        </div>
                    )}

                    {/* Connected Status */}
                    {isConnected && (
                        <div className="flex items-center gap-2 text-green-400 text-sm">
                            <Check className="w-4 h-4" />
                            Connected to {sshConfig.ip || localIp}
                        </div>
                    )}

                    {/* Destination Selection - only show when connected */}
                    {isConnected && (
                        <div className="space-y-2">
                            <Label className="text-sm font-medium text-white flex items-center gap-2">
                                <HardDrive className="w-4 h-4" />
                                Destination on Steam Deck
                            </Label>

                            {isLoadingLibraries ? (
                                <div className="flex items-center gap-2 text-muted-foreground text-sm">
                                    <Loader2 className="w-4 h-4 animate-spin" />
                                    Loading Steam libraries...
                                </div>
                            ) : (
                                <Select value={selectedLibrary} onValueChange={setSelectedLibrary}>
                                    <SelectTrigger className="bg-[#2a475e] border-[#1b2838]">
                                        <SelectValue placeholder="Select library" />
                                    </SelectTrigger>
                                    <SelectContent className="bg-[#2a475e] border-[#1b2838]">
                                        {libraries.map((lib) => (
                                            <SelectItem key={lib} value={lib} className="text-sm">
                                                {lib.includes("mmcblk") || lib.includes("media")
                                                    ? `📁 SD Card (${lib})`
                                                    : `💾 Internal Storage (${lib})`}
                                            </SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            )}
                        </div>
                    )}

                    {/* Info about what will happen - only show when connected */}
                    {isConnected && (
                        <div className="bg-[#2a475e] border border-[#1b2838] p-3 text-sm text-muted-foreground">
                            <p>This will:</p>
                            <ul className="list-disc list-inside mt-1 space-y-1">
                                <li>Copy the game folder via rsync</li>
                                <li>Add the AppID to SLSsteam config on Steam Deck</li>
                                <li>Create an ACF file for Steam to recognize the game</li>
                            </ul>
                        </div>
                    )}
                </div>

                <DialogFooter>
                    <Button
                        variant="outline"
                        onClick={onClose}
                        className="border-[#0a0a0a]"
                    >
                        Cancel
                    </Button>
                    <Button
                        onClick={handleCopy}
                        disabled={!isConnected || !selectedLibrary}
                        className="btn-steam"
                    >
                        <Upload className="w-4 h-4 mr-2" />
                        Copy to Steam Deck
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
