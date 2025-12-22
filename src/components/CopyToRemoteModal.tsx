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
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Loader2, Upload, HardDrive, FolderOpen } from "lucide-react";
import { getSteamLibraries, type InstalledGame } from "@/lib/api";

interface CopyToRemoteModalProps {
    isOpen: boolean;
    onClose: () => void;
    game: InstalledGame | null;
}

export function CopyToRemoteModal({ isOpen, onClose, game }: CopyToRemoteModalProps) {
    const { sshConfig, addLog, setInstallProgress } = useAppStore();

    const [libraries, setLibraries] = useState<string[]>([]);
    const [selectedLibrary, setSelectedLibrary] = useState<string>("");
    const [isLoadingLibraries, setIsLoadingLibraries] = useState(false);
    const [isCopying, setIsCopying] = useState(false);

    // Load remote Steam libraries when modal opens
    useEffect(() => {
        if (isOpen && sshConfig.ip && sshConfig.password) {
            loadRemoteLibraries();
        }
    }, [isOpen, sshConfig]);

    const loadRemoteLibraries = async () => {
        setIsLoadingLibraries(true);
        try {
            const libs = await getSteamLibraries(sshConfig);
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

        setIsCopying(true);
        addLog("info", `Starting copy of ${game.name} to remote: ${selectedLibrary}`);

        try {
            const targetPath = `${selectedLibrary}/steamapps/common`;

            // Import the copy function
            const { copyGameToRemote } = await import("@/lib/api");

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
            onClose();
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
        } finally {
            setIsCopying(false);
        }
    };

    if (!game) return null;

    return (
        <Dialog open={isOpen} onOpenChange={onClose}>
            <DialogContent className="bg-[#1b2838] border-[#0a0a0a] max-w-lg">
                <DialogHeader>
                    <DialogTitle className="text-white flex items-center gap-2">
                        <Upload className="w-5 h-5" />
                        Copy to Steam Deck: {game.name}
                    </DialogTitle>
                </DialogHeader>

                <div className="space-y-4 py-4">
                    {/* Game Info */}
                    <div className="bg-[#171a21] border border-[#0a0a0a] p-3 flex items-center gap-3">
                        <div className="w-16 h-16 bg-[#2a475e] rounded flex items-center justify-center">
                            <FolderOpen className="w-8 h-8 text-muted-foreground" />
                        </div>
                        <div>
                            <p className="font-medium text-white">{game.name}</p>
                            <p className="text-sm text-muted-foreground">
                                AppID: {game.app_id} • {formatSize(game.size_bytes)}
                            </p>
                            <p className="text-xs text-muted-foreground truncate max-w-sm" title={game.path}>
                                From: {game.path}
                            </p>
                        </div>
                    </div>

                    {/* Destination Selection */}
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

                    {/* Info about what will happen */}
                    <div className="bg-[#2a475e] border border-[#1b2838] p-3 text-sm text-muted-foreground">
                        <p>This will:</p>
                        <ul className="list-disc list-inside mt-1 space-y-1">
                            <li>Copy the game folder via rsync</li>
                            <li>Add the AppID to SLSsteam config on Steam Deck</li>
                            <li>Create an ACF file for Steam to recognize the game</li>
                        </ul>
                    </div>
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
                        disabled={!selectedLibrary || isCopying}
                        className="btn-steam"
                    >
                        {isCopying ? (
                            <>
                                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                Copying...
                            </>
                        ) : (
                            <>
                                <Upload className="w-4 h-4 mr-2" />
                                Copy to Steam Deck
                            </>
                        )}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
