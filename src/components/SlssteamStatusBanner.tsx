import { useAppStore } from "@/store/useAppStore";
import { AlertCircle, CheckCircle2, Settings, Loader2, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEffect, useState, useRef, useCallback } from "react";
import type { SlssteamStatus } from "@/lib/api";

interface StatusInfo {
  isLoading: boolean;
  isConfigured: boolean;
  message: string;
  details?: string;
}

const REFRESH_INTERVAL_MS = 15000; // 15 seconds

export function SlssteamStatusBanner() {
  const { connectionMode, connectionStatus, sshConfig, setActiveTab, addLog } = useAppStore();
  const [status, setStatus] = useState<StatusInfo>({
    isLoading: true,
    isConfigured: false,
    message: "Checking configuration...",
  });
  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  const checkStatus = useCallback(async (silent = false) => {
    if (!silent) {
      setStatus(prev => ({ ...prev, isLoading: true, message: "Checking configuration..." }));
    }

    if (connectionMode === "local") {
      // Local mode - check if SLSsteam is installed locally
      try {
        const { verifySlssteamLocal } = await import("@/lib/api");
        const localStatus = await verifySlssteamLocal();
        
        if (localStatus.slssteam_so_exists && localStatus.config_exists) {
          setStatus({
            isLoading: false,
            isConfigured: true,
            message: "SLSsteam configured correctly",
            details: `${localStatus.additional_apps_count} games registered`,
          });
        } else {
          const missing: string[] = [];
          if (!localStatus.slssteam_so_exists) missing.push("SLSsteam.so");
          if (!localStatus.config_exists) missing.push("config.yaml");
          
          setStatus({
            isLoading: false,
            isConfigured: false,
            message: "SLSsteam is not installed",
            details: `Missing: ${missing.join(", ")}`,
          });
        }
      } catch (error) {
        addLog("warn", `Local SLSsteam check failed: ${error}`);
        setStatus({
          isLoading: false,
          isConfigured: false,
          message: "Unable to check SLSsteam",
          details: "Configure in settings",
        });
      }
    } else {
      // Remote mode - check SSH connection and SLSsteam status
      if (!sshConfig.ip || !sshConfig.password) {
        setStatus({
          isLoading: false,
          isConfigured: false,
          message: "No SSH configuration",
          details: "Configure Steam Deck connection",
        });
        return;
      }

      if (connectionStatus !== "ssh_ok") {
        setStatus({
          isLoading: false,
          isConfigured: false,
          message: "No SSH connection",
          details: "Check connection in settings",
        });
        return;
      }

      // SSH is connected - verify SLSsteam on remote
      try {
        const { verifySlssteam } = await import("@/lib/api");
        const remoteStatus: SlssteamStatus = await verifySlssteam(sshConfig);

        if (remoteStatus.slssteam_so_exists && remoteStatus.config_exists && remoteStatus.config_play_not_owned) {
          setStatus({
            isLoading: false,
            isConfigured: true,
            message: "SLSsteam configured on Deck",
            details: `${remoteStatus.additional_apps_count} games registered`,
          });
        } else {
          const missing: string[] = [];
          if (!remoteStatus.slssteam_so_exists) missing.push("SLSsteam.so");
          if (!remoteStatus.config_exists) missing.push("config.yaml");
          if (!remoteStatus.config_play_not_owned) missing.push("PlayNotOwnedGames");
          
          setStatus({
            isLoading: false,
            isConfigured: false,
            message: "SLSsteam requires configuration",
            details: `Missing: ${missing.join(", ")}`,
          });
        }
      } catch (error) {
        addLog("warn", `Remote SLSsteam check failed: ${error}`);
        setStatus({
          isLoading: false,
          isConfigured: false,
          message: "Unable to check SLSsteam",
          details: "SSH verification error",
        });
      }
    }
  }, [connectionMode, connectionStatus, sshConfig, addLog]);

  // Initial check
  useEffect(() => {
    checkStatus();
  }, [checkStatus]);

  // Auto-refresh every 15s ONLY when not configured
  useEffect(() => {
    // Clear existing interval
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }

    // Only set up auto-refresh if not configured
    if (!status.isConfigured && !status.isLoading) {
      intervalRef.current = setInterval(() => {
        checkStatus(true); // Silent refresh
      }, REFRESH_INTERVAL_MS);
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [status.isConfigured, status.isLoading, checkStatus]);

  const goToSettings = () => {
    setActiveTab("settings");
  };

  const handleManualRefresh = () => {
    checkStatus();
  };

  if (status.isLoading) {
    return (
      <div className="bg-[#2a475e] border border-[#1b2838] rounded-md p-3 mb-4 flex items-center gap-3">
        <Loader2 className="w-5 h-5 text-[#67c1f5] animate-spin flex-shrink-0" />
        <div className="flex-1">
          <p className="text-sm text-white">{status.message}</p>
        </div>
      </div>
    );
  }

  if (status.isConfigured) {
    return (
      <div className="bg-[#2a4c28] border border-[#408f40] rounded-md p-3 mb-4 flex items-center gap-3">
        <CheckCircle2 className="w-5 h-5 text-green-400 flex-shrink-0" />
        <div className="flex-1">
          <p className="text-sm text-white">{status.message}</p>
          {status.details && (
            <p className="text-xs text-green-300/70">{status.details}</p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-[#4c2828] border border-[#8f4040] rounded-md p-3 mb-4 flex items-center gap-3">
      <AlertCircle className="w-5 h-5 text-yellow-500 flex-shrink-0" />
      <div className="flex-1">
        <p className="text-sm text-white">{status.message}</p>
        <div className="flex items-center gap-2">
          {status.details && (
            <p className="text-xs text-yellow-300/70">{status.details}</p>
          )}
        </div>
      </div>
      <Button 
        size="sm" 
        variant="ghost" 
        onClick={handleManualRefresh}
        className="text-yellow-400 hover:text-yellow-300 hover:bg-[#8f4040]/20 flex-shrink-0 px-2"
        title="Refresh now"
      >
        <RefreshCw className="w-4 h-4" />
      </Button>
      <Button 
        size="sm" 
        variant="outline" 
        onClick={goToSettings}
        className="border-[#8f4040] text-white hover:bg-[#8f4040]/20 flex-shrink-0"
      >
        <Settings className="w-4 h-4 mr-2" />
        Settings
      </Button>
    </div>
  );
}
