import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Save, Eye, EyeOff, FolderOpen, RefreshCw, AlertCircle, Check, Loader2, Monitor, Wifi, ArrowLeftRight } from "lucide-react";
import { useState } from "react";
import { saveApiKey, testSshConnection } from "@/lib/api";
import { open } from "@tauri-apps/plugin-dialog";

export function SettingsPanel() {
  const { settings, setSettings, sshConfig, setSshConfig, addLog, setConnectionStatus, connectionMode } = useAppStore();
  const [showApiKey, setShowApiKey] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [localApiKey, setLocalApiKey] = useState(settings.apiKey);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isInstallingSlssteam, setIsInstallingSlssteam] = useState(false);
  const [isVerifying, setIsVerifying] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connectionSuccess, setConnectionSuccess] = useState(false);
  const [sshpassWarning, setSshpassWarning] = useState<string | null>(null);
  const [verifyStatus, setVerifyStatus] = useState<{
    is_readonly: boolean;
    slssteam_so_exists: boolean;
    config_exists: boolean;
    config_play_not_owned: boolean;
    config_safe_mode_on: boolean;
    steam_jupiter_patched: boolean;
    desktop_entry_patched: boolean;
    additional_apps_count: number;
  } | null>(null);

  // Save API key to secure storage
  const handleSaveApiKey = async () => {
    setIsSaving(true);
    try {
      await saveApiKey(localApiKey);
      setSettings({ apiKey: localApiKey });
      addLog("info", "API key saved successfully");
    } catch (error) {
      addLog("error", `Failed to save API key: ${error}`);
    } finally {
      setIsSaving(false);
    }
  };

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

  // Browse for DepotDownloaderMod binary
  const handleBrowseDepotDownloader = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: "Select DepotDownloaderMod binary",
      });
      if (selected) {
        // Handle both string and object return types
        const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
        setSettings({ depotDownloaderPath: path });
        addLog("info", `DepotDownloaderMod path set to: ${path}`);
      }
    } catch (error) {
      console.error("File picker error:", error);
      addLog("error", `Failed to select file: ${error}`);
    }
  };

  // Browse for Steamless binary
  const handleBrowseSteamless = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: "Select Steamless binary",
      });
      if (selected) {
        // Handle both string and object return types
        const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
        setSettings({ steamlessPath: path });
        addLog("info", `Steamless path set to: ${path}`);
      }
    } catch (error) {
      console.error("File picker error:", error);
      addLog("error", `Failed to select file: ${error}`);
    }
  };

  // Install SLSsteam
  const handleInstallSlssteam = async () => {
    if (!settings.slssteamPath) {
      addLog("error", "Please select a path to SLSsteam first");
      return;
    }

    // Check SSH config only in remote mode
    if (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) {
      addLog("error", "Configure SSH connection first");
      return;
    }

    // For local mode, we don't need root password typically (or use sudo dialog)
    // For remote mode, reuse SSH password for sudo
    const rootPassword = connectionMode === "local" ? "" : sshConfig.password;

    setIsInstallingSlssteam(true);
    addLog("info", "Starting SLSsteam installation...");

    try {
      // Import API functions
      const { checkReadonlyStatus, installSlssteam } = await import("@/lib/api");

      // Build config with is_local flag
      const configToUse = { ...sshConfig };
      if (connectionMode === "local") {
        configToUse.is_local = true;
      }

      // Check readonly status (only relevant for SteamOS)
      if (connectionMode === "remote") {
        addLog("info", "Checking read-only mode...");
        const isReadonly = await checkReadonlyStatus(configToUse);

        if (isReadonly) {
          addLog("error", "SteamOS is in read-only mode! Run: sudo steamos-readonly disable");
          setIsInstallingSlssteam(false);
          return;
        }
        addLog("info", "Read-only mode disabled ✓");
      } else {
        addLog("info", "Local mode - skipping readonly check");
      }

      // Install SLSsteam
      const result = await installSlssteam(configToUse, settings.slssteamPath, rootPassword);
      addLog("info", result);
      addLog("info", "SLSsteam installed successfully! Restart Steam.");
    } catch (error) {
      addLog("error", `SLSsteam installation failed: ${error}`);
    } finally {
      setIsInstallingSlssteam(false);
    }
  };

  // Verify SLSsteam installation
  const handleVerifySlssteam = async () => {
    // Remote mode requires SSH config
    if (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) {
      addLog("error", "Configure SSH connection first");
      return;
    }

    setIsVerifying(true);
    setVerifyStatus(null);
    addLog("info", "Checking SLSsteam installation...");

    try {
      if (connectionMode === "local") {
        // Use local verification
        const { verifySlssteamLocal } = await import("@/lib/api");
        const status = await verifySlssteamLocal();

        // Adapt local status to verifyStatus format (some fields not available locally)
        setVerifyStatus({
          is_readonly: false, // Can't check remotely, assume false locally
          slssteam_so_exists: status.slssteam_so_exists,
          config_exists: status.config_exists,
          config_play_not_owned: status.config_play_not_owned,
          config_safe_mode_on: false, // Not checked locally
          steam_jupiter_patched: false, // Not checked locally
          desktop_entry_patched: false, // Not checked locally
          additional_apps_count: status.additional_apps_count,
        });

        addLog("info", `[LOCAL] SLSsteam.so: ${status.slssteam_so_exists ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] config.yaml: ${status.config_exists ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] PlayNotOwnedGames: ${status.config_play_not_owned ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] Registered games: ${status.additional_apps_count}`);
      } else {
        // Use remote SSH verification
        const { verifySlssteam } = await import("@/lib/api");
        const status = await verifySlssteam(sshConfig);
        setVerifyStatus(status);

        addLog("info", `Readonly: ${status.is_readonly ? "❌ YES" : "✅ NO"}`);
        addLog("info", `SLSsteam.so: ${status.slssteam_so_exists ? "✅" : "❌"}`);
        addLog("info", `config.yaml: ${status.config_exists ? "✅" : "❌"}`);
        addLog("info", `PlayNotOwnedGames: ${status.config_play_not_owned ? "✅" : "❌"}`);
        addLog("info", `SafeMode: ${status.config_safe_mode_on ? "✅" : "❌"}`);
        addLog("info", `steam-jupiter patched: ${status.steam_jupiter_patched ? "✅" : "❌"}`);
        addLog("info", `Desktop entry patched: ${status.desktop_entry_patched ? "✅" : "❌"}`);
        addLog("info", `Registered games: ${status.additional_apps_count}`);
      }
    } catch (error) {
      addLog("error", `Verification failed: ${error}`);
    } finally {
      setIsVerifying(false);
    }
  };

  return (
    <div className="space-y-6">
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

      {/* Morrenus API */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">Morrenus API</CardTitle>
          <CardDescription>Log in via Discord at https://manifest.morrenus.xyz and create an API key in account settings.<br />Free key: 25 manifests per day, must be regenerated every 24 hours.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="apiKey">API Key</Label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  id="apiKey"
                  type={showApiKey ? "text" : "password"}
                  placeholder="Enter key (valid 24h)"
                  value={localApiKey}
                  onChange={(e) => setLocalApiKey(e.target.value)}
                />
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute right-0 top-0 h-full hover:bg-transparent"
                  onClick={() => setShowApiKey(!showApiKey)}
                >
                  {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </Button>
              </div>
              <Button onClick={handleSaveApiKey} disabled={isSaving} className="btn-steam">
                <Save className="w-4 h-4 mr-2" />
                Save
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* SteamGridDB API */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">SteamGridDB API (optional for covers)</CardTitle>
          <CardDescription>Get API key from https://www.steamgriddb.com/profile/preferences/api</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="steamGridDbApiKey">SteamGridDB API Key</Label>
            <div className="flex gap-2">
              <Input
                id="steamGridDbApiKey"
                type="password"
                placeholder="Enter API key for game covers"
                value={settings.steamGridDbApiKey}
                onChange={(e) => setSettings({ steamGridDbApiKey: e.target.value })}
                className="flex-1"
              />
              <Button
                onClick={async () => {
                  try {
                    const { saveToolSettings } = await import("@/lib/api");
                    await saveToolSettings({
                      depotDownloaderPath: settings.depotDownloaderPath,
                      steamlessPath: settings.steamlessPath,
                      slssteamPath: settings.slssteamPath,
                      steamGridDbApiKey: settings.steamGridDbApiKey,
                    });
                    addLog("info", "SteamGridDB API key saved");
                  } catch (e) {
                    addLog("error", `Save error: ${e}`);
                  }
                }}
                className="btn-steam"
              >
                <Save className="w-4 h-4 mr-2" />
                Save
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Paths & Tools */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">Paths and Tools</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="depotPath">DepotDownloaderMod Path</Label>
            <div className="flex gap-2">
              <Input
                id="depotPath"
                placeholder="/path/to/DepotDownloaderMod"
                value={settings.depotDownloaderPath}
                onChange={(e) => setSettings({ depotDownloaderPath: e.target.value })}
                className="flex-1"
              />
              <Button variant="outline" onClick={handleBrowseDepotDownloader} className="border-[#0a0a0a]">
                <FolderOpen className="w-4 h-4" />
              </Button>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="steamlessPath">Steamless.CLI.exe Path (optionally required for some DRM games)</Label>
            <div className="flex gap-2">
              <Input
                id="steamlessPath"
                placeholder="/path/to/Steamless.CLI.exe"
                value={settings.steamlessPath}
                onChange={(e) => setSettings({ steamlessPath: e.target.value })}
                className="flex-1"
              />
              <Button variant="outline" onClick={handleBrowseSteamless} className="border-[#0a0a0a]">
                <FolderOpen className="w-4 h-4" />
              </Button>
            </div>
          </div>

          <Separator className="my-4" />

          <Button
            onClick={async () => {
              try {
                const { saveToolSettings } = await import("@/lib/api");
                await saveToolSettings({
                  depotDownloaderPath: settings.depotDownloaderPath,
                  steamlessPath: settings.steamlessPath,
                  slssteamPath: settings.slssteamPath,
                  steamGridDbApiKey: settings.steamGridDbApiKey,
                });
                addLog("info", "Tool settings saved");
              } catch (e) {
                addLog("error", `Failed to save tool settings: ${e}`);
              }
            }}
            className="w-full btn-steam"
          >
            <Save className="w-4 h-4 mr-2" />
            Save
          </Button>
        </CardContent>
      </Card>

      {/* SLSsteam Installation */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">SLSsteam Installation on Steam Deck / Linux</CardTitle>
          <CardDescription>SLSsteam is a library that allows running games not from the Steam store.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="bg-[#2a475e] border border-[#1b2838] p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="w-5 h-5 text-[#67c1f5] flex-shrink-0 mt-0.5" />
              <div className="text-sm">
                <p className="font-medium text-white mb-1">Installation requires:</p>
                <ul className="list-disc list-inside text-muted-foreground space-y-1">
                  <li>Creating an administrator password (does not affect normal usage)</li>
                  <li>Enabling SSH service (only for remote connections)</li>
                  <li>Disabling read-only mode (only for SteamOS in game mode)</li>
                </ul>
              </div>
            </div>
          </div>

          <div className="space-y-2">
            <Label>SLSsteam Update</Label>
            <div className="flex gap-2 items-center">
              <div className="flex-1 bg-[#0a0a0a] border border-[#2a475e] rounded-md px-3 py-2 text-sm">
                {settings.slssteamVersion ? (
                  <span className="text-green-400">SLSsteam {settings.slssteamVersion}</span>
                ) : (
                  <span className="text-muted-foreground">Not downloaded</span>
                )}
              </div>
              <Button
                variant="outline"
                className="border-[#0a0a0a]"
                onClick={async () => {
                  setSettings({ slssteamPath: "downloading..." });
                  addLog("info", "Fetching latest SLSsteam from GitHub...");
                  try {
                    const { fetchLatestSlssteam, getCachedSlssteamVersion, getCachedSlssteamPath } = await import("@/lib/api");
                    const result = await fetchLatestSlssteam();
                    addLog("info", result);
                    const version = await getCachedSlssteamVersion();
                    const path = await getCachedSlssteamPath();
                    setSettings({ slssteamVersion: version || undefined, slssteamPath: path || "" });
                  } catch (e) {
                    addLog("error", `Failed to fetch SLSsteam: ${e}`);
                    setSettings({ slssteamPath: "" });
                  }
                }}
              >
                <RefreshCw className="w-4 h-4 mr-2" />
                Fetch Update
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Downloads SLSsteam-Any.7z from GitHub and extracts bin/SLSsteam.so
            </p>
          </div>

          <div className="flex gap-2">
            <Button
              onClick={handleInstallSlssteam}
              disabled={isInstallingSlssteam || (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) || !settings.slssteamPath || settings.slssteamPath === "downloading..."}
              className="btn-steam flex-1"
            >
              {isInstallingSlssteam ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Installing...
                </>
              ) : (
                <>
                  <Check className="w-4 h-4 mr-2" />
                  {connectionMode === "local" ? "Install Locally" : "Install on Deck"}
                </>
              )}
            </Button>
            <Button
              onClick={handleVerifySlssteam}
              disabled={isVerifying || (connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password))}
              variant="outline"
              className="border-[#0a0a0a]"
            >
              {isVerifying ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
              <span className="ml-2">Verify</span>
            </Button>
          </div>

          {verifyStatus && (
            <div className="bg-[#171a21] border border-[#0a0a0a] p-3 space-y-2 text-sm">
              <div className="grid grid-cols-2 gap-2">
                <div className={verifyStatus.is_readonly ? "text-red-400" : "text-green-400"}>
                  {verifyStatus.is_readonly ? "❌" : "✅"} Read-only: {verifyStatus.is_readonly ? "YES" : "NO"}
                </div>
                <div className={verifyStatus.slssteam_so_exists ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.slssteam_so_exists ? "✅" : "❌"} SLSsteam.so
                </div>
                <div className={verifyStatus.config_exists ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.config_exists ? "✅" : "❌"} config.yaml
                </div>
                <div className={verifyStatus.config_play_not_owned ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.config_play_not_owned ? "✅" : "❌"} PlayNotOwnedGames
                </div>
                <div className={verifyStatus.config_safe_mode_on ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.config_safe_mode_on ? "✅" : "❌"} SafeMode
                </div>
                <div className={verifyStatus.steam_jupiter_patched ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.steam_jupiter_patched ? "✅" : "❌"} steam-jupiter
                </div>
                <div className={verifyStatus.desktop_entry_patched ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.desktop_entry_patched ? "✅" : "❌"} Desktop entry
                </div>
                <div className="text-[#67c1f5]">
                  🎮 Games: {verifyStatus.additional_apps_count}
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Separator />

      {/* Info */}
      <div className="text-sm text-muted-foreground text-center">
        <p>TonTonDeck v1.0.0</p>
      </div>
    </div>
  );
}
