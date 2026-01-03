import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { RefreshCw, Trash2, FolderOpen, AlertCircle, Loader2, Search, Upload } from "lucide-react";
import { useState, useEffect, useRef, useCallback } from "react";
import { listInstalledGames, listInstalledGamesLocal, fetchSteamGridDbArtwork, type InstalledGame } from "@/lib/api";
import { formatSize } from "@/lib/utils";
import { CopyToRemoteModal } from "@/components/CopyToRemoteModal";
import { GameCardModal } from "@/components/GameCardModal";

export function LibraryPanel() {
  const { sshConfig, addLog, connectionStatus, connectionMode, setSearchQuery, settings, setActiveTab, setTriggerSearch, libraryNeedsRefresh } = useAppStore();
  const [games, setGames] = useState<InstalledGame[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [artworkMap, setArtworkMap] = useState<Map<string, string>>(new Map());
  const [showOnlyTonTonDeck, setShowOnlyTonTonDeck] = useState<boolean>(() => {
    // Load saved preference from localStorage
    const saved = localStorage.getItem("libraryFilterTonTonDeck");
    return saved !== null ? saved === "true" : true; // Default to true
  });
  // State for Copy to Remote modal
  const [copyToRemoteGame, setCopyToRemoteGame] = useState<InstalledGame | null>(null);
  // State for Game Card modal
  const [selectedGameForCard, setSelectedGameForCard] = useState<InstalledGame | null>(null);

  // Refs for preventing duplicate refreshes
  const isRefreshingRef = useRef(false);
  const lastRefreshTimeRef = useRef<number>(0);
  const prevConnectionModeRef = useRef(connectionMode);

  // Memoized refreshGames to avoid stale closures in useEffect
  const refreshGames = useCallback(async () => {
    if (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) {
      setError("Configure SSH connection in settings");
      return;
    }

    setIsLoading(true);
    setError(null);
    setGames([]); // Clear list first to prevent duplicates

    try {
      let installedGames: InstalledGame[];

      if (connectionMode === "local") {
        installedGames = await listInstalledGamesLocal();
        addLog("info", `[LOCAL] Found ${installedGames.length} installed games`);
      } else {
        installedGames = await listInstalledGames(sshConfig);
        addLog("info", `[REMOTE] Found ${installedGames.length} installed games`);
      }

      // Deduplicate games by app_id (Steam paths may be symlinked)
      const uniqueGames = installedGames.reduce((acc, game) => {
        // Use app_id as key, but fallback to path for unknown app_ids
        const key = game.app_id !== "unknown" ? game.app_id : game.path;
        if (!acc.has(key)) {
          acc.set(key, game);
        }
        return acc;
      }, new Map<string, InstalledGame>());

      const deduped = Array.from(uniqueGames.values());
      if (deduped.length < installedGames.length) {
        addLog("info", `Deduplicated ${installedGames.length - deduped.length} duplicate entries`);
      }

      setGames(deduped);

      // Fetch artwork in batches for better performance
      fetchArtworksWithCache(deduped);
    } catch (e) {
      const errorMsg = `Error loading library: ${e}`;
      setError(errorMsg);
      addLog("error", errorMsg);
    } finally {
      setIsLoading(false);
    }
  }, [connectionMode, sshConfig, addLog, settings.steamGridDbApiKey]);

  // Load artworks with disk caching - batched for performance
  const fetchArtworksWithCache = useCallback(async (gamesList: InstalledGame[]) => {
    const { getCachedArtworkPath, cacheArtwork } = await import("@/lib/api");
    const BATCH_SIZE = 5;
    const artworkUpdates = new Map<string, string>();

    // Filter games that need artwork
    const gamesToFetch = gamesList.filter(
      game => game.app_id && game.app_id !== "unknown"
    );

    // Process in batches for better performance
    for (let i = 0; i < gamesToFetch.length; i += BATCH_SIZE) {
      const batch = gamesToFetch.slice(i, i + BATCH_SIZE);

      await Promise.allSettled(
        batch.map(async (game) => {
          // Skip if already fetched in this batch run
          if (artworkUpdates.has(game.app_id)) return;

          try {
            // 1. Check disk cache first
            const cachedPath = await getCachedArtworkPath(game.app_id);
            if (cachedPath) {
              const assetUrl = `asset://localhost/${cachedPath}`;
              artworkUpdates.set(game.app_id, assetUrl);
              return;
            }

            // 2. No cache - fetch from SteamGridDB if API key available
            if (settings.steamGridDbApiKey) {
              const artwork = await fetchSteamGridDbArtwork(settings.steamGridDbApiKey, game.app_id);
              if (artwork) {
                const localPath = await cacheArtwork(game.app_id, artwork);
                const assetUrl = `asset://localhost/${localPath}`;
                artworkUpdates.set(game.app_id, assetUrl);
              }
            }
          } catch {
            // Ignore artwork errors
          }
        })
      );

      // Batch update state after each batch to show progress
      if (artworkUpdates.size > 0) {
        setArtworkMap(prev => {
          const newMap = new Map(prev);
          artworkUpdates.forEach((url, appId) => newMap.set(appId, url));
          return newMap;
        });
      }
    }
  }, [settings.steamGridDbApiKey]);

  // Clear games immediately when connectionMode changes to prevent stale data
  useEffect(() => {
    if (prevConnectionModeRef.current !== connectionMode) {
      prevConnectionModeRef.current = connectionMode;
      // Immediately clear stale data from previous mode
      setGames([]);
      setError(null);
      setArtworkMap(new Map()); // Also clear artwork cache on mode change
      addLog("info", `Mode changed to ${connectionMode}, clearing library...`);
    }
  }, [connectionMode, addLog]);

  // Auto-refresh when libraryNeedsRefresh flag is set
  useEffect(() => {
    if (libraryNeedsRefresh && !isRefreshingRef.current) {
      // Debounce: prevent rapid refreshes (500ms minimum between refreshes)
      const now = Date.now();
      if (now - lastRefreshTimeRef.current < 500) {
        useAppStore.setState({ libraryNeedsRefresh: false });
        return;
      }

      useAppStore.setState({ libraryNeedsRefresh: false });
      lastRefreshTimeRef.current = now;
      isRefreshingRef.current = true;
      refreshGames().finally(() => {
        isRefreshingRef.current = false;
      });
    }
  }, [libraryNeedsRefresh, refreshGames]);

  // Reset state when disconnected (only for remote mode)
  useEffect(() => {
    if (connectionMode === "remote" && connectionStatus !== "ssh_ok") {
      setGames([]);
    }
  }, [connectionStatus, connectionMode]);

  return (
    <div className="space-y-6">
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-white">Installed Games</CardTitle>
              <CardDescription>List of Steam games {showOnlyTonTonDeck ? "installed by TonTonDeck" : "in your library"}</CardDescription>
            </div>
            <Button
              variant="outline"
              onClick={refreshGames}
              disabled={isLoading || (connectionMode === "remote" && connectionStatus !== "ssh_ok")}
              className="border-[#0a0a0a]"
            >
              {isLoading ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
            </Button>
          </div>
          {/* Filter checkbox */}
          <div className="flex items-center gap-2 mt-2">
            <Checkbox
              id="filter-tontondeck"
              checked={showOnlyTonTonDeck}
              disabled={isLoading}
              onCheckedChange={(checked) => {
                const value = Boolean(checked);
                setShowOnlyTonTonDeck(value);
                localStorage.setItem("libraryFilterTonTonDeck", String(value));
              }}
            />
            <Label htmlFor="filter-tontondeck" className="text-sm text-muted-foreground cursor-pointer">
              Show only TonTonDeck-installed games
            </Label>
          </div>
        </CardHeader>
        <CardContent>
          {connectionMode === "remote" && connectionStatus !== "ssh_ok" && (
            <div className="bg-[#2a475e] border border-[#1b2838] p-4 flex items-center gap-3">
              <AlertCircle className="w-5 h-5 text-[#67c1f5]" />
              <p className="text-muted-foreground">Connect to device to view library</p>
            </div>
          )}

          {error && (
            <div className="bg-[#4c2828] border border-[#8f4040] p-4 flex items-center gap-3">
              <AlertCircle className="w-5 h-5 text-red-400" />
              <p className="text-red-300">{error}</p>
            </div>
          )}

          {/* Empty state - show when ready but no games */}
          {((connectionMode === "local") || (connectionMode === "remote" && connectionStatus === "ssh_ok")) && !error && games.length === 0 && !isLoading && (
            <div className="text-center py-8 text-muted-foreground">
              <p>No installed games or click refresh</p>
            </div>
          )}

          {games.length > 0 && (
            <div className="space-y-2">
              {games
                .filter(game => !showOnlyTonTonDeck || game.has_depotdownloader_marker)
                .map((game) => (
                  <div
                    key={game.app_id !== "unknown" ? game.app_id : game.path}
                    className="bg-[#171a21] border border-[#0a0a0a] p-3 flex items-center justify-between hover:bg-[#1b2838] transition-colors cursor-pointer"
                    onClick={() => setSelectedGameForCard(game)}
                  >
                    <div className="flex items-center gap-4">
                      {artworkMap.get(game.app_id) || game.header_image ? (
                        <img
                          src={artworkMap.get(game.app_id) || game.header_image}
                          alt={game.name}
                          className="w-24 h-9 object-cover rounded"
                        />
                      ) : (
                        <div className="w-24 h-9 bg-[#2a475e] rounded flex items-center justify-center">
                          <FolderOpen className="w-4 h-4 text-muted-foreground" />
                        </div>
                      )}
                      <div>
                        <p className="font-medium text-white">{game.name}</p>
                        <p className="text-xs text-muted-foreground">
                          AppID: {game.app_id} • {formatSize(game.size_bytes)}
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-[#67c1f5] hover:text-[#8ed0f8] hover:bg-[#2a475e]"
                        title="Search for updates"
                        onClick={(e) => {
                          e.stopPropagation();
                          setSearchQuery(game.app_id !== "unknown" ? game.app_id : game.name);
                          setActiveTab("search");
                          setTriggerSearch(true);
                        }}
                      >
                        <Search className="w-4 h-4" />
                      </Button>
                      {/* Copy to Remote - only in local mode */}
                      {connectionMode === "local" && (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-green-400 hover:text-green-300 hover:bg-green-900/20"
                          title="Copy to Steam Deck"
                          onClick={(e) => {
                            e.stopPropagation();
                            setCopyToRemoteGame(game);
                          }}
                        >
                          <Upload className="w-4 h-4" />
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-red-400 hover:text-red-300 hover:bg-red-900/20"
                        title="Uninstall"
                        onClick={async (e) => {
                          e.stopPropagation();
                          if (!confirm(`Are you sure you want to uninstall ${game.name}?`)) return;
                          try {
                            const { uninstallGame } = await import("@/lib/api");
                            // Use correct config based on connection mode
                            const configForUninstall = { ...sshConfig };
                            if (connectionMode === "local") {
                              configForUninstall.is_local = true;
                            }
                            await uninstallGame(configForUninstall, game.path, game.app_id);
                            addLog("info", `Uninstalled: ${game.name}`);
                            refreshGames(); // Refresh list
                          } catch (e) {
                            addLog("error", `Uninstall error: ${e}`);
                          }
                        }}
                      >
                        <Trash2 className="w-4 h-4" />
                      </Button>
                    </div>
                  </div>
                ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Copy to Remote Modal */}
      <CopyToRemoteModal
        isOpen={copyToRemoteGame !== null}
        onClose={() => setCopyToRemoteGame(null)}
        game={copyToRemoteGame}
      />

      {/* Game Card Modal */}
      <GameCardModal
        isOpen={selectedGameForCard !== null}
        onClose={() => setSelectedGameForCard(null)}
        game={selectedGameForCard}
        onGameRemoved={refreshGames}
      />
    </div>
  );
}
