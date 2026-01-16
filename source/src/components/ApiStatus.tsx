import { useState, useEffect, useCallback } from "react";
import { useAppStore } from "@/store/useAppStore";
import { checkDepotProviderApiStatus, type DepotProviderUserStats, getGlobalCloudStatus, type GlobalCloudStatus } from "@/lib/api";
import { Cloud, CloudOff, AlertTriangle } from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

type ApiHealthStatus = "healthy" | "limited" | "error" | "loading";

export function ApiStatus() {
  const { settings } = useAppStore();
  const [status, setStatus] = useState<ApiHealthStatus>("loading");
  const [stats, setStats] = useState<DepotProviderUserStats | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [cloudStatus, setCloudStatus] = useState<GlobalCloudStatus | null>(null);

  const checkApiStatus = useCallback(async () => {
    setStatus("loading");
    setErrorMessage(null);

    try {
      const result = await checkDepotProviderApiStatus(settings.apiKey || "");

      if (!result.health_ok) {
        setStatus("error");
        setErrorMessage(result.error || "API server unavailable");
        setStats(null);
        return;
      }

      if (result.user_stats) {
        setStats(result.user_stats);
        if (result.user_stats.can_make_requests) {
          setStatus("healthy");
        } else {
          setStatus("limited");
          setErrorMessage("Daily limit reached");
        }
      } else {
        setStats(null);
        setStatus("limited");
        setErrorMessage(result.error || "No API key");
      }
    } catch (error) {
      setStatus("error");
      setErrorMessage(`Connection error: ${error}`);
      setStats(null);
    }
    
    // Check cloud status
    try {
      const cStatus = await getGlobalCloudStatus();
      setCloudStatus(cStatus);
    } catch {
      // Ignore cloud status errors
    }
  }, [settings.apiKey]);


  // Check on mount and when API key changes
  useEffect(() => {
    checkApiStatus();
  }, [checkApiStatus]);

  const getStatusIcon = () => {
    switch (status) {
      case "healthy":
        return <Cloud className="w-4 h-4 text-[#5ba32b]" />; // Steam green
      case "limited":
        return <AlertTriangle className="w-4 h-4 text-[#ffc82c]" />; // Steam yellow
      case "error":
        return <CloudOff className="w-4 h-4 text-[#cd3838]" />; // Steam red
      case "loading":
        return <Cloud className="w-4 h-4 text-gray-500 animate-pulse" />;
    }
  };

  const getStatusColor = () => {
    switch (status) {
      case "healthy":
        return "text-[#5ba32b]";
      case "limited":
        return "text-[#ffc82c]";
      case "error":
        return "text-[#cd3838]";
      default:
        return "text-gray-500";
    }
  };

  const getTooltipContent = () => {
    if (status === "loading") {
      return "Checking API status...";
    }

    if (status === "error" || (status === "limited" && !stats)) {
      return errorMessage || "API Error";
    }

    if (stats) {
      return (
        <div className="text-xs space-y-1">
          <div className="font-medium">{stats.username}</div>
          <div>Today: {stats.daily_usage} / {stats.daily_limit}</div>
          {!stats.can_make_requests && (
            <div className="text-[#ffc82c]">Daily limit reached</div>
          )}
        </div>
      );
    }

    return "No API key";
  };

  return (
    <>
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={checkApiStatus}
            className="flex items-center gap-1.5 px-1.5 py-1 rounded hover:bg-[#2a475e]/50 transition-colors"
          >
            {getStatusIcon()}
            {stats && (
              <span className={`text-xs font-medium ${getStatusColor()}`}>
                {stats.daily_usage}/{stats.daily_limit}
              </span>
            )}
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom" className="bg-[#1b2838] border-[#2a475e]">
          {getTooltipContent()}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>

      {/* Cloud Sync Status */}
      {cloudStatus?.enabled && (
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="flex items-center gap-1.5 px-1.5 py-1 rounded hover:bg-[#2a475e]/50 transition-colors cursor-help">
                {cloudStatus.is_syncing ? (
                  <Cloud className="w-4 h-4 text-[#67c1f5] animate-pulse" />
                ) : cloudStatus.games_pending > 0 ? (
                  <Cloud className="w-4 h-4 text-[#ffcc6b]" />
                ) : cloudStatus.games_with_conflicts > 0 ? (
                  <AlertTriangle className="w-4 h-4 text-[#ffc82c]" />
                ) : (
                  <Cloud className="w-4 h-4 text-[#5ba32b]" />
                )}
              </div>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="bg-[#1b2838] border-[#2a475e]">
              <div className="text-xs space-y-1">
                <div className="font-medium">Cloud Saves</div>
                {cloudStatus.is_syncing ? (
                  <div className="text-[#67c1f5]">Syncing...</div>
                ) : (
                  <>
                    <div className="text-gray-300">Synced: {cloudStatus.games_synced}</div>
                    {cloudStatus.games_pending > 0 && (
                      <div className="text-[#ffcc6b]">Pending: {cloudStatus.games_pending}</div>
                    )}
                    {cloudStatus.games_with_conflicts > 0 && (
                      <div className="text-[#ffc82c]">Conflicts: {cloudStatus.games_with_conflicts}</div>
                    )}
                    {cloudStatus.games_pending === 0 && cloudStatus.games_with_conflicts === 0 && (
                      <div className="text-[#5ba32b]">All synced</div>
                    )}
                    {cloudStatus.last_sync && (
                      <div className="text-gray-500 pt-1">Last: {new Date(cloudStatus.last_sync).toLocaleTimeString()}</div>
                    )}
                  </>
                )}
              </div>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      )}
    </>
  );
}
