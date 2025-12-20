import { useEffect, useCallback, useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { checkDeckStatus, testSshConnection } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Wifi, WifiOff, Check, RefreshCw, Loader2, HardDrive } from "lucide-react";

export function DeckStatus() {
  const {
    sshConfig,
    connectionStatus,
    setConnectionStatus,
    connectionMode,
    setConnectionMode,
    addLog,
  } = useAppStore();

  const [isReconnecting, setIsReconnecting] = useState(false);

  // Initial check on mount
  useEffect(() => {
    if (connectionMode === "remote" && sshConfig.ip) {
      handleReconnect();
    }
  }, [connectionMode]); // Trigger when mode changes

  // Reconnect function
  const handleReconnect = useCallback(async () => {
    if (connectionMode === "local") return;
    
    if (!sshConfig.ip) {
      setConnectionStatus("offline");
      return;
    }

    setIsReconnecting(true);
    addLog("info", `Checking connection to ${sshConfig.ip}...`);

    try {
      // First check if online
      const status = await checkDeckStatus(sshConfig.ip, sshConfig.port);
      
      if (status === "online") {
        // Try SSH if credentials are set
        if (sshConfig.password || sshConfig.privateKeyPath) {
          try {
            await testSshConnection(sshConfig);
            setConnectionStatus("ssh_ok");
            addLog("info", "SSH connection established");
          } catch {
            setConnectionStatus("online");
            addLog("warn", "Device online but SSH auth failed");
          }
        } else {
          setConnectionStatus("online");
        }
      } else {
        setConnectionStatus("offline");
        addLog("warn", "Device offline");
      }
    } catch (error) {
      setConnectionStatus("offline");
      addLog("error", `Connection check failed: ${error}`);
    } finally {
      setIsReconnecting(false);
    }
  }, [sshConfig, setConnectionStatus, addLog, connectionMode]);

  return (
    <div className="flex items-center gap-3">
      {/* Mode Toggle */}
      <div className="flex bg-[#0a0a0a] rounded-lg p-1 border border-[#67c1f5]/20">
        <button
          onClick={() => {
            addLog("info", "Switched to LOCAL mode");
            setConnectionMode("local");
          }}
          className={`px-3 py-1 text-xs rounded-md transition-colors ${
            connectionMode === "local"
              ? "bg-[#67c1f5] text-[#1b2838] font-medium"
              : "text-gray-400 hover:text-white"
          }`}
        >
          Local Mode
        </button>
        <button
          onClick={() => {
            addLog("info", "Switched to REMOTE mode");
            setConnectionMode("remote");
          }}
          className={`px-3 py-1 text-xs rounded-md transition-colors ${
            connectionMode === "remote"
              ? "bg-[#67c1f5] text-[#1b2838] font-medium"
              : "text-gray-400 hover:text-white"
          }`}
        >
          Remote Mode
        </button>
      </div>

      {/* Status Icons */}
      {connectionMode === "local" ? (
        <>
          <span title="Local Storage">
            <HardDrive className="w-4 h-4 text-green-500" />
          </span>
          <Button
            variant="ghost"
            size="sm"
            disabled={true}
            className="h-8 w-8 p-0 text-gray-600 cursor-not-allowed"
            title="Local Mode"
          >
           <RefreshCw className="w-4 h-4" />
          </Button>
        </>
      ) : (
        <>
          {/* WiFi status icon - always visible */}
          {connectionStatus === "ssh_ok" && !isReconnecting && (
            <span title="Connected">
              <Check className="w-4 h-4 text-green-500" />
            </span>
          )}
          {connectionStatus === "online" && !isReconnecting && (
            <span title="Online (No SSH)">
              <Wifi className="w-4 h-4 text-yellow-500" />
            </span>
          )}
          {connectionStatus === "offline" && !isReconnecting && (
            <span title="Offline">
              <WifiOff className="w-4 h-4 text-red-500" />
            </span>
          )}
          {/* Greyed out when reconnecting or status unknown */}
          {(isReconnecting || (connectionStatus !== "ssh_ok" && connectionStatus !== "online" && connectionStatus !== "offline")) && (
            <span title={isReconnecting ? "Checking connection..." : "Connection not checked"}>
              <Wifi className="w-4 h-4 text-gray-500" />
            </span>
          )}
          
          <Button
            variant="ghost"
            size="sm"
            onClick={handleReconnect}
            disabled={isReconnecting || !sshConfig.ip}
            className="h-8 w-8 p-0 text-[#67c1f5] hover:bg-[#2a475e]"
            title="Reconnect"
          >
            {isReconnecting ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <RefreshCw className="w-4 h-4" />
            )}
          </Button>
        </>
      )}
    </div>
  );
}
