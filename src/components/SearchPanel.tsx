import { useState, useCallback, useEffect } from "react";
import { useAppStore, type SearchResult } from "@/store/useAppStore";
import { searchBundles } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

import { Search, Download, Loader2, Package, AlertTriangle, FolderUp } from "lucide-react";
import { InstallModal } from "@/components/InstallModal";
import { SlssteamStatusBanner } from "@/components/SlssteamStatusBanner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

export function SearchPanel() {
  const {
    searchQuery,
    setSearchQuery,
    searchResults,
    setSearchResults,
    isSearching,
    setIsSearching,
    connectionStatus,
    connectionMode,
    settings,
    addLog,
    triggerSearch,
    setTriggerSearch,
  } = useAppStore();

  const [localQuery, setLocalQuery] = useState(searchQuery);
  const [installModalOpen, setInstallModalOpen] = useState(false);
  const [selectedGame, setSelectedGame] = useState<SearchResult | null>(null);
  const [showApiKeyAlert, setShowApiKeyAlert] = useState(false);
  const [isUploadingManifest, setIsUploadingManifest] = useState(false);
  const [uploadedZipPath, setUploadedZipPath] = useState<string | null>(null);

  // Sync local query when store query changes (e.g. from LibraryPanel)
  useEffect(() => {
    if (searchQuery !== localQuery) {
      setLocalQuery(searchQuery);
    }
  }, [searchQuery]);

  // Handle auto-trigger search
  useEffect(() => {
    if (triggerSearch && searchQuery) {
      const performMobileSearch = async () => {
        setIsSearching(true);
        addLog("info", `Auto-searching for: ${searchQuery}`);
        try {
          const results = await searchBundles(searchQuery);
          setSearchResults(results);
          addLog("info", `Found ${results.length} results`);
        } catch (error) {
          addLog("error", `Search failed: ${error}`);
          setSearchResults([]);
        } finally {
          setIsSearching(false);
          setTriggerSearch(false);
        }
      };
      performMobileSearch();
    }
  }, [triggerSearch, searchQuery, setTriggerSearch, setIsSearching, setSearchResults, addLog]);

  // Generate Steam header image URL
  const getHeaderImageUrl = (appId: string) => {
    return `https://cdn.akamai.steamstatic.com/steam/apps/${appId}/header.jpg`;
  };

  // Perform search
  const handleSearch = useCallback(async () => {
    if (!localQuery.trim()) return;

    setSearchQuery(localQuery);
    setIsSearching(true);
    addLog("info", `Searching for: ${localQuery}`);

    try {
      const results = await searchBundles(localQuery);
      setSearchResults(results);
      addLog("info", `Found ${results.length} results`);
    } catch (error) {
      const errorMsg = String(error);
      addLog("error", `Search failed: ${errorMsg}`);
      setSearchResults([]);

      // Show alert if API key expired
      if (errorMsg.includes("expired") || errorMsg.includes("invalid") || errorMsg.includes("401")) {
        setShowApiKeyAlert(true);
      }
    } finally {
      setIsSearching(false);
    }
  }, [localQuery, setSearchQuery, setIsSearching, setSearchResults, addLog]);

  // Handle keyboard events
  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleSearch();
    }
  };

  // Handle manifest ZIP upload
  const handleUploadManifest = async () => {
    try {
      setIsUploadingManifest(true);
      addLog("info", "Opening file picker for manifest ZIP...");

      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [{ name: "Manifest ZIP", extensions: ["zip"] }],
        title: "Select manifest .zip",
      });

      if (!selected) {
        addLog("info", "File picker cancelled");
        return;
      }

      const zipPath = typeof selected === "string" ? selected : selected[0];
      addLog("info", `Processing manifest: ${zipPath}`);

      const { extractManifestZip } = await import("@/lib/api");
      const gameData = await extractManifestZip(zipPath);

      addLog("info", `Extracted manifest for: ${gameData.game_name} (${gameData.app_id})`);

      // Create minimal SearchResult for InstallModal
      // InstallModal will use the game_id to load depots via its own flow
      const searchResult: SearchResult = {
        game_id: gameData.app_id,
        game_name: gameData.game_name,
        manifest_available: true,
        header_image: `https://cdn.cloudflare.steamstatic.com/steam/apps/${gameData.app_id}/header.jpg`,
      };

      // Open InstallModal with the uploaded manifest data
      setSelectedGame(searchResult);
      setUploadedZipPath(zipPath); // Pass the zip path so InstallModal skips downloadBundle
      setInstallModalOpen(true);

    } catch (error) {
      addLog("error", `Failed to process manifest: ${error}`);
    } finally {
      setIsUploadingManifest(false);
    }
  };

  // Start installation
  const handleInstall = (gameId: string) => {
    addLog("info", `[DEBUG] handleInstall called for ${gameId}, connectionMode=${connectionMode}, connectionStatus=${connectionStatus}`);

    // LOCAL mode doesn't require SSH connection
    if (connectionMode !== "local" && connectionStatus !== "ssh_ok") {
      addLog("error", "Cannot install: SSH connection not established. Configure in Settings or switch to Local mode.");
      return;
    }

    // Find the result object to get details
    const result = searchResults.find(r => r.game_id === gameId);
    if (!result) return;

    addLog("info", `[DEBUG] Opening InstallModal for ${result.game_name}`);
    // Open install modal with selected game
    setSelectedGame(result);
    setInstallModalOpen(true);
  };

  // Format bytes
  const formatBytes = (bytes: number | undefined): string => {
    if (!bytes) return "";
    const units = ["B", "KB", "MB", "GB"];
    let size = bytes;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  return (
    <div className="space-y-4">
      {/* SLSsteam Status Banner */}
      <SlssteamStatusBanner />

      {/* Search Card */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">Search Games</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Search input */}
          <div className="flex gap-2">
            <Input
              placeholder="Search by game name or AppID from SteamDB..."
              value={localQuery}
              onChange={(e) => setLocalQuery(e.target.value)}
              onKeyPress={handleKeyPress}
              className="flex-1 bg-[#316282] border-none text-white placeholder:text-[#7a9cb0]"
            />
            <Button
              variant="outline"
              onClick={handleUploadManifest}
              disabled={isUploadingManifest}
              className="border-[#2a475e] text-white hover:bg-[#2a475e]/50"
              title="Upload manifest .zip from disk"
            >
              {isUploadingManifest ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <>
                  <FolderUp className="w-4 h-4 mr-2" />
                  Upload manifest.zip
                </>
              )}
            </Button>
            <Button
              onClick={handleSearch}
              disabled={isSearching || !localQuery.trim()}
              className="btn-steam px-6"
            >
              {isSearching ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <>
                  <Search className="w-4 h-4 mr-2" />
                  Search
                </>
              )}
            </Button>
          </div>

          {!settings.apiKey && (
            <div className="bg-[#4c2828] border border-[#8f4040] p-3 text-[#ddd] rounded">
              ⚠️ API key is invalid or not configured. Go to settings.
            </div>
          )}
        </CardContent>
      </Card>

      {/* Results */}
      <div>
        {searchResults.length === 0 ? (
          <div className="flex items-center justify-center h-64 text-muted-foreground">
            {isSearching ? (
              <Loader2 className="w-8 h-8 animate-spin" />
            ) : (
              <div className="text-center">
                <Package className="w-16 h-16 mx-auto mb-4 opacity-30" />
                <p className="text-center text-muted-foreground">Search or upload a manifest to install a game on your device.</p>
              </div>
            )}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4" style={{ contentVisibility: 'auto' }}>
            {searchResults.map((result) => (
              <Card
                key={result.game_id}
                className="game-card bg-[#1b2838] border-[#0a0a0a] overflow-hidden"
                style={{ contain: 'layout style paint' }}
              >
                {/* Game header image */}
                <div className="relative">
                  <img
                    src={getHeaderImageUrl(result.game_id)}
                    alt={result.game_name}
                    className="w-full steam-header-image bg-[#0a0a0a]"
                    loading="lazy"
                    decoding="async"
                    onError={(e) => {
                      (e.target as HTMLImageElement).src = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='460' height='215' viewBox='0 0 460 215'%3E%3Crect fill='%231b2838' width='460' height='215'/%3E%3Ctext x='230' y='107' fill='%23666' font-family='Arial' font-size='14' text-anchor='middle'%3ENo Image%3C/text%3E%3C/svg%3E";
                    }}
                  />
                  {!result.manifest_available && (
                    <div className="absolute inset-0 bg-black/70 flex items-center justify-center">
                      <span className="text-yellow-500 font-medium">Not Available</span>
                    </div>
                  )}
                </div>

                <CardContent className="p-3">
                  <h3 className="font-medium text-white truncate mb-1" title={result.game_name}>
                    {result.game_name}
                  </h3>
                  <div className="flex items-center justify-between">
                    <div className="text-xs text-muted-foreground">
                      <span>AppID: {result.game_id}</span>
                      {result.manifest_size && (
                        <span className="ml-2">• {formatBytes(result.manifest_size)}</span>
                      )}
                    </div>
                    <Button
                      size="sm"
                      disabled={!result.manifest_available || (connectionMode !== "local" && connectionStatus !== "ssh_ok")}
                      onClick={() => handleInstall(result.game_id)}
                      className="btn-steam text-xs py-1 px-3 h-auto"
                    >
                      <Download className="w-3 h-3 mr-1" />
                      Install
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>

      {/* Install Modal */}
      <InstallModal
        isOpen={installModalOpen}
        onClose={() => {
          setInstallModalOpen(false);
          setUploadedZipPath(null); // Clear uploaded path on close
        }}
        game={selectedGame}
        preExtractedZipPath={uploadedZipPath || undefined}
      />

      {/* API Key Expired Alert */}
      <AlertDialog open={showApiKeyAlert} onOpenChange={setShowApiKeyAlert}>
        <AlertDialogContent className="bg-[#1b2838] border-[#0a0a0a]">
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2 text-white">
              <AlertTriangle className="w-5 h-5 text-yellow-500" />
              API Key Expired
            </AlertDialogTitle>
            <AlertDialogDescription className="text-gray-400">
              Your Morrenus API key has expired or is invalid.
              Generate a new key and paste it in Settings.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogAction
              onClick={() => {
                setShowApiKeyAlert(false);
                useAppStore.getState().setActiveTab("settings");
              }}
              className="btn-steam"
            >
              Go to Settings
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
