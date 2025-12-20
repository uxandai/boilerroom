import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCw, Trash2, FolderOpen, AlertCircle, Loader2, Search } from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { listInstalledGames, listInstalledGamesLocal, fetchSteamGridDbArtwork, type InstalledGame } from "@/lib/api";

export function LibraryPanel() {
  const { sshConfig, addLog, connectionStatus, connectionMode, setSearchQuery, settings, setActiveTab, setTriggerSearch } = useAppStore();
  const [games, setGames] = useState<InstalledGame[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [artworkMap, setArtworkMap] = useState<Map<string, string>>(new Map());

  const refreshGames = async () => {
    if (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) {
      setError("Configure SSH connection in settings");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      let installedGames: InstalledGame[];

      if (connectionMode === "local") {
        installedGames = await listInstalledGamesLocal();
        addLog("info", `[LOCAL] Found ${installedGames.length} installed games`);
      } else {
        installedGames = await listInstalledGames(sshConfig);
        addLog("info", `[REMOTE] Found ${installedGames.length} installed games`);
      }

      setGames(installedGames);

      // Fetch artwork from SteamGridDB if API key is set
      if (settings.steamGridDbApiKey) {
        fetchArtworks(installedGames);
      }
    } catch (e) {
      const errorMsg = `Error loading library: ${e}`;
      setError(errorMsg);
      addLog("error", errorMsg);
    } finally {
      setIsLoading(false);
    }
  };

  const fetchArtworks = async (gamesList: InstalledGame[]) => {
    const newArtwork = new Map<string, string>();

    for (const game of gamesList) {
      if (game.app_id && game.app_id !== "unknown") {
        try {
          const artwork = await fetchSteamGridDbArtwork(settings.steamGridDbApiKey, game.app_id);
          if (artwork) {
            newArtwork.set(game.app_id, artwork);
          }
        } catch {
          // Ignore artwork fetch errors
        }
      }
    }

    setArtworkMap(newArtwork);
    addLog("info", `Downloaded ${newArtwork.size} covers from SteamGridDB`);
  };

  // Track if games have been loaded
  const hasLoadedRef = useRef(false);

  // Reset loaded state when disconnected (only for remote mode)
  useEffect(() => {
    if (connectionMode === "remote" && connectionStatus !== "ssh_ok") {
      hasLoadedRef.current = false;
      setGames([]); // Clear games when disconnected in remote mode
    }
  }, [connectionStatus, connectionMode]);

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  return (
    <div className="space-y-6">
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-white">Installed Games</CardTitle>
              <CardDescription>List of Steam games managed by TonTonDeck</CardDescription>
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
              {games.map((game) => (
                <div
                  key={game.app_id}
                  className="bg-[#171a21] border border-[#0a0a0a] p-3 flex items-center justify-between hover:bg-[#1b2838] transition-colors"
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
                      onClick={() => {
                        setSearchQuery(game.app_id !== "unknown" ? game.app_id : game.name);
                        setActiveTab("search");
                        setTriggerSearch(true);
                      }}
                    >
                      <Search className="w-4 h-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-red-400 hover:text-red-300 hover:bg-red-900/20"
                      title="Uninstall"
                      onClick={async () => {
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
    </div>
  );
}
