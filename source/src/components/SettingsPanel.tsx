import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";

import { Eye, EyeOff, RefreshCw, AlertCircle, Check, Loader2, Monitor, Wifi, ArrowLeftRight } from "lucide-react";
import { useState, useEffect } from "react";
import { testSshConnection } from "@/lib/api";

import { RelaunchSetupButton } from "@/components/SetupWizard";
import { StatusDashboard } from "@/components/settings/StatusDashboard";
import { ApiKeysPanel } from "@/components/settings/ApiKeysPanel";
import { SystemHealthPanel } from "@/components/settings/SystemHealthPanel";
import { ToolPathsPanel } from "@/components/settings/ToolPathsPanel";

export function SettingsPanel() {
  const { sshConfig, setSshConfig, addLog, setConnectionStatus, connectionMode } = useAppStore();
  const [showPassword, setShowPassword] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [appVersion, setAppVersion] = useState("...");

  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connectionSuccess, setConnectionSuccess] = useState(false);
  const [sshpassWarning, setSshpassWarning] = useState<string | null>(null);

  // Load app version on mount
  useEffect(() => {
    const loadVersion = async () => {
      try {
        const { getVersion } = await import("@tauri-apps/api/app");
        const version = await getVersion();
        setAppVersion(version);
      } catch {
        setAppVersion("1.4.0");
      }
    };
    loadVersion();
  }, []);

  // Test SSH connection AND save settings
  const handleTestAndSave = async () => {
    if (!sshConfig.ip) {
      addLog("error", "Please enter Steam Deck IP address");
      setConnectionError("Please enter Steam Deck IP address");
      return;
    }

    setIsTesting(true);
    setConnectionError(null);
    setConnectionSuccess(false);
    setSshpassWarning(null);
    addLog("info", `Testing SSH connection to ${sshConfig.ip}:${sshConfig.port}...`);

    try {
      await testSshConnection(sshConfig);
      setConnectionStatus("ssh_ok");
      setConnectionSuccess(true);
      addLog("info", "SSH connection successful!");

      // Auto-save on successful connection
      try {
        const { saveSshConfig, checkSshpassAvailable } = await import("@/lib/api");
        await saveSshConfig(sshConfig);
        addLog("info", "SSH settings saved automatically");

        // Check if sshpass is available (needed for rsync password auth)
        if (sshConfig.password && !sshConfig.privateKeyPath) {
          const hasSshpass = await checkSshpassAvailable();
          if (!hasSshpass) {
            setSshpassWarning("sshpass is not installed. Rsync will prompt for password on each transfer. Install: sudo pacman -S sshpass");
            addLog("warn", "sshpass not found - rsync will prompt for password");
          }
        }
      } catch (saveError) {
        addLog("error", `Failed to save settings: ${saveError}`);
      }
    } catch (error) {
      setConnectionStatus("offline");
      const errorMsg = String(error);
      setConnectionError(errorMsg);
      addLog("error", `SSH test failed: ${errorMsg}`);
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Status Dashboard - Quick overview at top */}
      <StatusDashboard />

      {/* Mode Selection - Always at top */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white flex items-center gap-2">
            {connectionMode === "local" ? (
              <Monitor className="w-5 h-5 text-[#67c1f5]" />
            ) : (
              <Wifi className="w-5 h-5 text-[#67c1f5]" />
            )}
            Operating Mode
          </CardTitle>
          <CardDescription>
            Current: <strong className="text-white">{connectionMode === "local" ? "On this device" : "Remote transfer"}</strong>
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button
            onClick={async () => {
              try {
                const { clearConnectionMode } = await import("@/lib/api");
                await clearConnectionMode();
                addLog("info", "Operating mode reset. Restart the application.");
                // Reload the app
                window.location.reload();
              } catch (e) {
                addLog("error", `Failed to reset mode: ${e}`);
              }
            }}
            variant="outline"
            className="w-full border-[#2a475e] text-white hover:bg-[#2a475e]/50"
          >
            <ArrowLeftRight className="w-4 h-4 mr-2" />
            Change Operating Mode
          </Button>
          <RelaunchSetupButton />
        </CardContent>
      </Card>

      {/* Steam Deck Connection (Remote Only) */}
      {connectionMode !== "local" && (
        <Card className="bg-[#1b2838] border-[#2a475e]">
          <CardHeader className="pb-3">
            <CardTitle className="text-white">Steam Deck Connection</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-3 gap-4">
              <div className="col-span-2 space-y-2">
                <Label htmlFor="ip">Steam Deck IP Address (from network settings)</Label>
                <Input
                  id="ip"
                  placeholder="192.168.0.100"
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
              <Label htmlFor="username">SSH Username</Label>
              <Input
                id="username"
                placeholder="deck"
                value={sshConfig.username}
                onChange={(e) => setSshConfig({ username: e.target.value })}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="password">SSH Password</Label>
              <div className="relative">
                <Input
                  id="password"
                  type={showPassword ? "text" : "password"}
                  placeholder="deck"
                  value={sshConfig.password}
                  onChange={(e) => setSshConfig({ password: e.target.value })}
                />
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute right-0 top-0 h-full hover:bg-transparent"
                  onClick={() => setShowPassword(!showPassword)}
                >
                  {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </Button>
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="keyPath">SSH Key Path (optional)</Label>
              <Input
                id="keyPath"
                placeholder="/path/to/id_rsa"
                value={sshConfig.privateKeyPath}
                onChange={(e) => setSshConfig({ privateKeyPath: e.target.value })}
              />
            </div>

            <Button
              onClick={handleTestAndSave}
              disabled={!sshConfig.ip || isTesting}
              className="btn-steam w-full"
            >
              {isTesting ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Testing and saving...
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Test Connection & Save
                </>
              )}
            </Button>

            {/* Connection result message */}
            {connectionError && (
              <div className="bg-[#4c2828] border border-[#8f4040] p-3 text-sm">
                <div className="flex items-center gap-2 text-[#ff6b6b]">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  <span>{connectionError}</span>
                </div>
              </div>
            )}
            {connectionSuccess && !connectionError && (
              <div className="bg-[#2a4c28] border border-[#408f40] p-3 text-sm">
                <div className="flex items-center gap-2 text-[#6bff6b]">
                  <Check className="w-4 h-4 flex-shrink-0" />
                  <span>SSH connection successful! Settings saved.</span>
                </div>
              </div>
            )}
            {sshpassWarning && (
              <div className="bg-[#4c4428] border border-[#8f8040] p-3 text-sm">
                <div className="flex items-start gap-2 text-[#ffcc6b]">
                  <AlertCircle className="w-4 h-4 flex-shrink-0 mt-0.5" />
                  <span>{sshpassWarning}</span>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}




      {/* System Health - Consolidated Panel */}
      <SystemHealthPanel />

      {/* Tool Paths */}
      <ToolPathsPanel />

      {/* API Keys - Consolidated Panel */}
      <ApiKeysPanel />

      {/* Info */}
      <div className="text-sm text-muted-foreground text-center">
        <p>BoilerRoom v{appVersion}</p>
      </div>

    </div>
  );
}

