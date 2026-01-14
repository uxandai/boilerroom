import { useEffect, useCallback, useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { checkDeckStatus, testSshConnection } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Wifi, WifiOff, Check, Pause, Play, Loader2 } from "lucide-react";

export function ConnectionPanel() {
  const {
    sshConfig,
    setSshConfig,
    connectionStatus,
    setConnectionStatus,
    connectionCheckPaused,
    toggleConnectionCheck,
    addLog,
  } = useAppStore();

  const [isTesting, setIsTesting] = useState(false);

  // Status check function
  const checkConnection = useCallback(async () => {
    if (!sshConfig.ip || connectionCheckPaused) return;

    try {
      const status = await checkDeckStatus(sshConfig.ip, sshConfig.port);
      setConnectionStatus(status as "offline" | "online" | "ssh_ok");
    } catch (error) {
      setConnectionStatus("offline");
      addLog("error", `Connection check failed: ${error}`);
    }
  }, [sshConfig.ip, sshConfig.port, connectionCheckPaused, setConnectionStatus, addLog]);

  // Auto-refresh every 5 seconds
  useEffect(() => {
    if (connectionCheckPaused) return;

    const interval = setInterval(checkConnection, 5000);
    // Initial check
    checkConnection();

    return () => clearInterval(interval);
  }, [checkConnection, connectionCheckPaused]);

  // Test SSH connection
  const handleTestConnection = async () => {
    setIsTesting(true);
    addLog("info", `Testing SSH connection to ${sshConfig.ip}:${sshConfig.port}...`);

    try {
      const result = await testSshConnection(sshConfig);
      setConnectionStatus("ssh_ok");
      addLog("info", `SSH connection successful: ${result}`);
    } catch (error) {
      addLog("error", `SSH test failed: ${error}`);
    } finally {
      setIsTesting(false);
    }
  };

  // Status indicator
  const StatusIndicator = () => {
    switch (connectionStatus) {
      case "ssh_ok":
        return (
          <div className="flex items-center gap-2 text-green-500">
            <Check className="w-5 h-5" />
            <span className="font-medium">SSH OK</span>
          </div>
        );
      case "online":
        return (
          <div className="flex items-center gap-2 text-yellow-500">
            <Wifi className="w-5 h-5" />
            <span className="font-medium">ONLINE (SSH not tested)</span>
          </div>
        );
      default:
        return (
          <div className="flex items-center gap-2 text-red-500">
            <WifiOff className="w-5 h-5" />
            <span className="font-medium">OFFLINE</span>
          </div>
        );
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          Steam Deck Connection
          <div className="flex items-center gap-4">
            <StatusIndicator />
            <Button
              variant="ghost"
              size="icon"
              onClick={toggleConnectionCheck}
              title={connectionCheckPaused ? "Resume auto-check" : "Pause auto-check"}
            >
              {connectionCheckPaused ? (
                <Play className="w-4 h-4" />
              ) : (
                <Pause className="w-4 h-4" />
              )}
            </Button>
          </div>
        </CardTitle>
        <CardDescription>
          Configure SSH connection to your Steam Deck
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <Label htmlFor="ip">IP Address</Label>
            <Input
              id="ip"
              placeholder="192.168.1.100"
              value={sshConfig.ip}
              onChange={(e) => setSshConfig({ ip: e.target.value })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="port">Port</Label>
            <Input
              id="port"
              type="number"
              placeholder="22"
              value={sshConfig.port}
              onChange={(e) => setSshConfig({ port: parseInt(e.target.value) || 22 })}
            />
          </div>
        </div>

        <div className="space-y-2">
          <Label htmlFor="username">Username</Label>
          <Input
            id="username"
            placeholder="deck"
            value={sshConfig.username}
            onChange={(e) => setSshConfig({ username: e.target.value })}
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="password">Password (optional)</Label>
          <Input
            id="password"
            type="password"
            placeholder="Leave empty if using SSH key"
            value={sshConfig.password}
            onChange={(e) => setSshConfig({ password: e.target.value })}
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="keyPath">SSH Private Key Path (optional)</Label>
          <Input
            id="keyPath"
            placeholder="/path/to/id_rsa"
            value={sshConfig.privateKeyPath}
            onChange={(e) => setSshConfig({ privateKeyPath: e.target.value })}
          />
        </div>

        <Button
          onClick={handleTestConnection}
          disabled={!sshConfig.ip || isTesting}
          className="w-full"
        >
          {isTesting ? (
            <>
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              Testing Connection...
            </>
          ) : (
            "Test Connection"
          )}
        </Button>
      </CardContent>
    </Card>
  );
}
