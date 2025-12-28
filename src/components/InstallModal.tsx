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
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Loader2, Download, HardDrive, RefreshCcw, Key } from "lucide-react";
import { getSteamLibraries, startPipelinedInstall, downloadBundle, extractManifestZip, checkGameInstalled, installDepotKeysOnly, type DepotInfo, type InstalledDepot, type DepotKeyInfo } from "@/lib/api";
import type { SearchResult } from "@/store/useAppStore";

interface Depot {
  depot_id: string;
  manifest_id: string;
  name: string;
  size?: number;
  manifest_path: string; // From backend DepotInfo
  key: string;
  selected: boolean;
  isOutdated?: boolean; // true if installed manifest differs from new
}

interface InstallModalProps {
  isOpen: boolean;
  onClose: () => void;
  game: SearchResult | null;
  preExtractedZipPath?: string; // When set, skip downloadBundle and use this path directly
}

export function InstallModal({ isOpen, onClose, game, preExtractedZipPath }: InstallModalProps) {
  const { sshConfig, settings, addLog, setInstallProgress, connectionMode } = useAppStore();

  const [depots, setDepots] = useState<Depot[]>([]);
  const [libraries, setLibraries] = useState<string[]>([]);
  const [selectedLibrary, setSelectedLibrary] = useState<string>("");
  const [isLoadingLibraries, setIsLoadingLibraries] = useState(false);
  const [isLoadingDepots, setIsLoadingDepots] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [isGameInstalled, setIsGameInstalled] = useState(false);
  const [hasUpdates, setHasUpdates] = useState(false);
  const [isDepotKeysOnly, setIsDepotKeysOnly] = useState(false);

  // Load Steam libraries when modal opens
  useEffect(() => {
    if (isOpen) {
      console.log("[DEBUG] Modal opened, connectionMode=", connectionMode);
      addLog("info", `[DEBUG] Modal opened, connectionMode=${connectionMode}`);
      if (connectionMode === "local") {
        // LOCAL mode: use default local Steam library paths
        console.log("[DEBUG] Calling loadLocalLibraries...");
        addLog("info", "[DEBUG] Calling loadLocalLibraries...");
        loadLocalLibraries();
      } else if (sshConfig.ip && sshConfig.password) {
        // REMOTE mode: load from Deck via SSH
        console.log("[DEBUG] Calling loadLibraries (remote)...");
        addLog("info", "[DEBUG] Calling loadLibraries (remote)...");
        loadLibraries();
      } else {
        console.log("[DEBUG] Neither LOCAL mode nor SSH configured!");
        addLog("warn", "[DEBUG] Neither LOCAL mode nor SSH configured!");
      }
    }
  }, [isOpen, sshConfig, connectionMode]);

  // Download and parse manifest when modal opens
  useEffect(() => {
    if (isOpen && game) {
      loadDepots();
    }
  }, [isOpen, game]);

  const loadDepots = async () => {
    if (!game) return;

    setIsLoadingDepots(true);
    setDepots([]);
    setIsGameInstalled(false);
    setHasUpdates(false);
    addLog("info", `Downloading manifest for ${game.game_name} (${game.game_id})...`);

    try {
      // Step 1: Check what's already installed
      let installed: InstalledDepot[] = [];
      try {
        // Use correct config based on connection mode
        const configForCheck = { ...sshConfig };
        if (connectionMode === "local") {
          configForCheck.is_local = true;
        }
        installed = await checkGameInstalled(configForCheck, game.game_id);
        if (installed.length > 0) {
          setIsGameInstalled(true);
          addLog("info", `Game is installed with ${installed.length} depots`);
        }
      } catch {
        // Not installed or check failed - proceed as new install
      }

      // Step 2: Get manifest - either from pre-extracted path or download from API
      let zipPath: string;
      if (preExtractedZipPath) {
        // Use uploaded manifest directly - skip API download
        zipPath = preExtractedZipPath;
        addLog("info", `Using uploaded manifest: ${zipPath}`);
      } else {
        // Download from Morrenus API
        zipPath = await downloadBundle(game.game_id);
        addLog("info", `Manifest downloaded to: ${zipPath}`);
      }

      // Step 3: Extract and parse the ZIP to get depot info
      const data = await extractManifestZip(zipPath);
      addLog("info", `Found ${data.depots.length} depots for ${data.game_name}`);

      // Step 4: Convert to our Depot format and check for updates
      let anyOutdated = false;
      const parsedDepots: Depot[] = data.depots.map((d: DepotInfo) => {
        const installedManifest = installed.find(i => i.depot_id === d.depot_id)?.manifest_id;
        const isOutdated = installedManifest ? installedManifest !== d.manifest_id : false;
        if (isOutdated) {
          anyOutdated = true;
          addLog("info", `Depot ${d.depot_id}: Update available (${installedManifest} → ${d.manifest_id})`);
        }
        return {
          depot_id: d.depot_id,
          manifest_id: d.manifest_id,
          name: d.name || `Depot ${d.depot_id}`,
          size: d.size,
          manifest_path: d.manifest_path,
          key: d.key,
          selected: isOutdated, // Auto-select outdated depots
          isOutdated,
        };
      });

      setHasUpdates(anyOutdated);
      setDepots(parsedDepots);
      addLog("info", `Depots: ${parsedDepots.map(d => d.depot_id).join(", ")}`);
    } catch (error) {
      addLog("error", `Failed to load depot info: ${error}`);
    } finally {
      setIsLoadingDepots(false);
    }
  };

  const loadLibraries = async () => {
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
        setSelectedLibrary(sorted[0]); // Default to internal (first after sort)
      }
    } catch (error) {
      addLog("error", `Failed to load Steam libraries: ${error}`);
    } finally {
      setIsLoadingLibraries(false);
    }
  };

  // LOCAL mode: detect local Steam libraries
  const loadLocalLibraries = async () => {
    setIsLoadingLibraries(true);
    try {
      const { homeDir } = await import("@tauri-apps/api/path");
      let home = await homeDir();
      // Ensure home has trailing slash
      if (!home.endsWith("/") && !home.endsWith("\\")) {
        home = home + "/";
      }
      const platform = navigator.platform.toLowerCase();
      let localLibraries: string[] = [];

      addLog("info", `[DEBUG] Home directory: ${home}, Platform: ${platform}`);

      if (platform.includes('linux')) {
        // Linux/Steam Deck
        localLibraries = [
          `${home}.local/share/Steam`,
          `${home}.steam/steam`,
        ];
      } else if (platform.includes('mac') || platform.includes('darwin')) {
        // macOS
        localLibraries = [
          `${home}Library/Application Support/Steam`,
          `${home}Games`, // Fallback
        ];
      } else {
        // Windows
        localLibraries = [
          "C:\\Program Files (x86)\\Steam",
          "C:\\Program Files\\Steam",
        ];
      }

      setLibraries(localLibraries);
      if (localLibraries.length > 0) {
        setSelectedLibrary(localLibraries[0]);
      }
    } catch (error) {
      console.error("Failed to load local libraries:", error);
      addLog("error", `[DEBUG] loadLocalLibraries failed: ${error}`);
      // Fallback to reasonable defaults
      setLibraries(["/tmp/Games"]);
      setSelectedLibrary("/tmp/Games");
    } finally {
      setIsLoadingLibraries(false);
      addLog("info", `[DEBUG] Libraries loaded: ${libraries.length}, selectedLibrary=${selectedLibrary}`);
    }
  };

  const toggleDepot = (depotId: string) => {
    setDepots(depots.map(d =>
      d.depot_id === depotId ? { ...d, selected: !d.selected } : d
    ));
  };

  const selectAllDepots = () => {
    setDepots(depots.map(d => ({ ...d, selected: true })));
  };

  const deselectAllDepots = () => {
    setDepots(depots.map(d => ({ ...d, selected: false })));
  };

  const selectedCount = depots.filter(d => d.selected).length;
  const totalSelectedSize = depots
    .filter(d => d.selected)
    .reduce((acc, d) => acc + (d.size || 0), 0);

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const handleInstall = async () => {
    if (!game || selectedCount === 0 || !selectedLibrary) return;

    setIsInstalling(true);
    addLog("info", `Starting installation of ${game.game_name} to ${selectedLibrary}`);

    try {
      const selectedDepotsFiltered = depots.filter(d => d.selected);
      const selectedDepotIds = selectedDepotsFiltered.map(d => d.depot_id);
      const selectedManifestIds = selectedDepotsFiltered.map(d => d.manifest_id);
      const selectedManifestFiles = selectedDepotsFiltered.map(d => d.manifest_path);
      const selectedDepotKeys: [string, string][] = selectedDepotsFiltered.map(d => [d.depot_id, d.key]);

      if (selectedDepotIds.length === 0) return;

      const targetPath = `${selectedLibrary}/steamapps/common`;

      // Inject is_local flag if needed
      const configToUse = { ...sshConfig };
      const { connectionMode } = useAppStore.getState();
      if (connectionMode === "local") {
        configToUse.is_local = true;
      }

      await startPipelinedInstall(
        game.game_id,
        game.game_name,
        selectedDepotIds,
        selectedManifestIds,
        selectedManifestFiles,
        selectedDepotKeys,
        settings.depotDownloaderPath,
        settings.steamlessPath,
        configToUse,
        targetPath
      );

      setInstallProgress({
        step: "downloading",
        appId: game.game_id,
        gameName: game.game_name,
        heroImage: `https://cdn.akamai.steamstatic.com/steam/apps/${game.game_id}/library_hero.jpg`,
        downloadPercent: 0,
        downloadSpeed: "",
        eta: "calculating...",
        filesTotal: 0,
        filesTransferred: 0,
        bytesTotal: 0,
        bytesTransferred: 0,
        transferSpeed: "",
        message: "Initializing..."
      });

      onClose();
    } catch (error) {
      addLog("error", `Install failed: ${error}`);
    } finally {
      setIsInstalling(false);
    }
  };

  // Handle "Only Depot & Keys" mode - configure Steam without downloading
  const handleDepotKeysOnly = async () => {
    if (!game || selectedCount === 0 || !selectedLibrary) return;

    setIsDepotKeysOnly(true);
    addLog("info", `Configuring depot keys for ${game.game_name} (no download)`);

    try {
      const selectedDepotsFiltered = depots.filter(d => d.selected);

      // Convert to DepotKeyInfo format
      const depotKeyInfos: DepotKeyInfo[] = selectedDepotsFiltered.map(d => ({
        depot_id: d.depot_id,
        manifest_id: d.manifest_id,
        manifest_path: d.manifest_path,
        key: d.key,
      }));

      // Inject is_local flag if needed
      const configToUse = { ...sshConfig };
      const { connectionMode } = useAppStore.getState();
      if (connectionMode === "local") {
        configToUse.is_local = true;
      }

      const result = await installDepotKeysOnly(
        game.game_id,
        game.game_name,
        depotKeyInfos,
        configToUse,
        selectedLibrary,
        false // Don't trigger steam://install - user should restart Steam manually
      );

      addLog("info", result);
      addLog("info", `✅ Depot keys configured! Restart Steam to start downloading.`);

      onClose();
    } catch (error) {
      addLog("error", `Depot keys config failed: ${error}`);
    } finally {
      setIsDepotKeysOnly(false);
    }
  };

  if (!game) return null;

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="bg-[#1b2838] border-[#0a0a0a] max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-white flex items-center gap-2">
            <Download className="w-5 h-5" />
            Install: {game.game_name}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* Depot Selection */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-sm font-medium text-white">Select Depots</Label>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={selectAllDepots}
                  className="text-xs h-6 px-2"
                >
                  Select All
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={deselectAllDepots}
                  className="text-xs h-6 px-2"
                >
                  Deselect All
                </Button>
              </div>
            </div>

            <div className="bg-[#2a475e] border border-[#1b2838] rounded max-h-48 overflow-y-auto">
              {isLoadingDepots ? (
                <div className="p-4 text-center text-muted-foreground text-sm flex items-center justify-center gap-2">
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Loading depot information...
                </div>
              ) : depots.length === 0 ? (
                <div className="p-4 text-center text-muted-foreground text-sm">
                  No depot information available
                </div>
              ) : (
                depots.map((depot) => (
                  <div
                    key={depot.depot_id}
                    className="flex items-center gap-3 p-3 border-b border-[#1b2838] last:border-b-0 hover:bg-[#1b2838]/50"
                  >
                    <Checkbox
                      id={depot.depot_id}
                      checked={depot.selected}
                      onCheckedChange={() => toggleDepot(depot.depot_id)}
                    />
                    <Label
                      htmlFor={depot.depot_id}
                      className="flex-1 cursor-pointer text-sm"
                    >
                      <span className="text-muted-foreground">{depot.depot_id}</span>
                      <span className="mx-2">-</span>
                      <span className="text-white" title={depot.name}>
                        {depot.name.length > 40 ? depot.name.substring(0, 40) + "..." : depot.name}
                      </span>
                    </Label>
                    {depot.size && (
                      <span className="text-xs text-muted-foreground">
                        {formatSize(depot.size)}
                      </span>
                    )}
                  </div>
                ))
              )}
            </div>

            {selectedCount > 0 && (
              <div className="text-xs text-[#67c1f5]">
                {selectedCount} depot(s) selected
                {totalSelectedSize > 0 && ` • ~${formatSize(totalSelectedSize)}`}
              </div>
            )}
          </div>

          {/* Library Selection */}
          <div className="space-y-2">
            <Label className="text-sm font-medium text-white flex items-center gap-2">
              <HardDrive className="w-4 h-4" />
              Install Location
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
        </div>

        <DialogFooter>
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={onClose}
              className="border-[#0a0a0a]"
            >
              Cancel
            </Button>
            <Button
              onClick={handleDepotKeysOnly}
              disabled={selectedCount === 0 || !selectedLibrary || isInstalling || isDepotKeysOnly}
              variant="secondary"
              title="Add depot keys and manifests only (no download) - use steam://install/{appid} afterwards"
            >
              {isDepotKeysOnly ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Configuring...
                </>
              ) : (
                <>
                  <Key className="w-4 h-4 mr-2" />
                  Only Depot & Keys
                </>
              )}
            </Button>
            <Button
              onClick={handleInstall}
              disabled={selectedCount === 0 || !selectedLibrary || isInstalling || isDepotKeysOnly}
              className="btn-steam"
            >
              {isInstalling ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  {isGameInstalled ? "Updating..." : "Installing..."}
                </>
              ) : isGameInstalled && hasUpdates ? (
                <>
                  <RefreshCcw className="w-4 h-4 mr-2" />
                  Update ({selectedCount})
                </>
              ) : isGameInstalled ? (
                <>
                  <Download className="w-4 h-4 mr-2" />
                  Reinstall ({selectedCount})
                </>
              ) : (
                <>
                  <Download className="w-4 h-4 mr-2" />
                  Install ({selectedCount})
                </>
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
