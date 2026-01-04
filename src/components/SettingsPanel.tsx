import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Checkbox } from "@/components/ui/checkbox";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Save, Eye, EyeOff, FolderOpen, RefreshCw, AlertCircle, Check, Loader2, Monitor, Wifi, ArrowLeftRight, CheckCircle2, Wrench, Shield, Download, ExternalLink, Terminal, Sparkles } from "lucide-react";
import { useState, useEffect } from "react";
import { saveApiKey, testSshConnection } from "@/lib/api";
import { open } from "@tauri-apps/plugin-dialog";

export function SettingsPanel() {
  const { settings, setSettings, sshConfig, setSshConfig, addLog, setConnectionStatus, connectionMode } = useAppStore();
  const [showApiKey, setShowApiKey] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [localApiKey, setLocalApiKey] = useState(settings.apiKey);
  const [useGistKey, setUseGistKeyLocal] = useState(settings.useGistKey ?? false);
  // Sync useGistKey with settings
  const setUseGistKey = (value: boolean) => {
    setUseGistKeyLocal(value);
    setSettings({ useGistKey: value });
  };
  const [isFetchingGistKey, setIsFetchingGistKey] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isInstallingSlssteam, setIsInstallingSlssteam] = useState(false);
  const [isVerifying, setIsVerifying] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connectionSuccess, setConnectionSuccess] = useState(false);
  const [sshpassWarning, setSshpassWarning] = useState<string | null>(null);
  // Confirmation dialogs
  const [showSlssteamInstalledDialog, setShowSlssteamInstalledDialog] = useState(false);
  const [showSettingsSavedDialog, setShowSettingsSavedDialog] = useState(false);
  const [settingsSavedMessage, setSettingsSavedMessage] = useState("");
  const [verifyStatus, setVerifyStatus] = useState<{
    is_readonly: boolean;
    slssteam_so_exists: boolean;
    library_inject_so_exists: boolean;
    config_exists: boolean;
    config_play_not_owned: boolean;
    config_safe_mode_on: boolean;
    steam_jupiter_patched: boolean;
    desktop_entry_patched: boolean;
    additional_apps_count: number;
  } | null>(null);
  // Steam updates status
  const [steamUpdatesStatus, setSteamUpdatesStatus] = useState<{
    is_configured: boolean;
    inhibit_all: boolean;
    force_self_update_disabled: boolean;
  } | null>(null);
  const [isCheckingSteamUpdates, setIsCheckingSteamUpdates] = useState(false);
  // libcurl32 status
  const [libcurl32Status, setLibcurl32Status] = useState<{
    source_exists: boolean;
    symlink_exists: boolean;
    symlink_correct: boolean;
  } | null>(null);
  const [isCheckingLibcurl32, setIsCheckingLibcurl32] = useState(false);

  // lib32 dependencies status (local mode)
  const [lib32DepsStatus, setLib32DepsStatus] = useState<{
    lib32_curl_installed: boolean;
    lib32_openssl_installed: boolean;
    lib32_glibc_installed: boolean;
    all_installed: boolean;
  } | null>(null);
  const [isCheckingLib32Deps, setIsCheckingLib32Deps] = useState(false);

  // Tools: Steamless & SLSah
  const [steamlessPath, setSteamlessPath] = useState("");
  const [isLaunchingSteamless, setIsLaunchingSteamless] = useState(false);
  const [slsahInstalled, setSlsahInstalled] = useState<boolean | null>(null);
  const [isInstallingSlsah, setIsInstallingSlsah] = useState(false);
  const [isLaunchingSlsah, setIsLaunchingSlsah] = useState(false);
  const [isCheckingSlsah, setIsCheckingSlsah] = useState(false);

  // Auto-check 32-bit dependencies when in local mode
  useEffect(() => {
    if (connectionMode === "local" && !lib32DepsStatus && !isCheckingLib32Deps) {
      // Delay slightly to ensure component is fully mounted
      const timer = setTimeout(async () => {
        setIsCheckingLib32Deps(true);
        try {
          const { checkLib32Dependencies } = await import("@/lib/api");
          // For local mode, we only need is_local: true - no SSH config needed
          const localConfig = { ip: "", port: 22, username: "", password: "", privateKeyPath: "", is_local: true };
          const status = await checkLib32Dependencies(localConfig);
          setLib32DepsStatus(status);
        } catch {
          // Silent fail for auto-check
        } finally {
          setIsCheckingLib32Deps(false);
        }
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [connectionMode, lib32DepsStatus, isCheckingLib32Deps]);

  // Save API key to secure storage
  const handleSaveApiKey = async () => {
    setIsSaving(true);
    try {
      await saveApiKey(localApiKey);
      setSettings({ apiKey: localApiKey });
      addLog("info", "API key saved successfully");
      setSettingsSavedMessage("Morrenus API key saved successfully!");
      setShowSettingsSavedDialog(true);
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
      setShowSlssteamInstalledDialog(true);
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
          is_readonly: false, // Not relevant locally
          slssteam_so_exists: status.slssteam_so_exists,
          library_inject_so_exists: status.library_inject_so_exists,
          config_exists: status.config_exists,
          config_play_not_owned: status.config_play_not_owned,
          config_safe_mode_on: false, // SteamOS only
          steam_jupiter_patched: false, // SteamOS only
          desktop_entry_patched: status.desktop_entry_patched,
          additional_apps_count: status.additional_apps_count,
        });

        addLog("info", `[LOCAL] SLSsteam.so: ${status.slssteam_so_exists ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] library-inject.so: ${status.library_inject_so_exists ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] config.yaml: ${status.config_exists ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] Desktop entry (LD_AUDIT): ${status.desktop_entry_patched ? "✅" : "❌"}`);
        addLog("info", `[LOCAL] Registered games: ${status.additional_apps_count}`);
      } else {
        // Use remote SSH verification
        const { verifySlssteam } = await import("@/lib/api");
        const status = await verifySlssteam(sshConfig);
        setVerifyStatus(status);

        addLog("info", `Readonly: ${status.is_readonly ? "❌ YES" : "✅ NO"}`);
        addLog("info", `SLSsteam.so: ${status.slssteam_so_exists ? "✅" : "❌"}`);
        addLog("info", `library-inject.so: ${status.library_inject_so_exists ? "✅" : "⚠️"}`);
        addLog("info", `config.yaml: ${status.config_exists ? "✅" : "❌"}`);
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

  // Check 32-bit library dependencies (local mode only)
  const handleCheckLib32Dependencies = async () => {
    if (connectionMode !== "local") {
      return; // Only relevant for local mode
    }

    setIsCheckingLib32Deps(true);
    try {
      const { checkLib32Dependencies } = await import("@/lib/api");
      const configToUse = { ...sshConfig, is_local: true };
      const status = await checkLib32Dependencies(configToUse);
      setLib32DepsStatus(status);

      if (status.all_installed) {
        addLog("info", "All 32-bit dependencies installed ✅");
      } else {
        addLog("warn", "Missing 32-bit dependencies - Steam may not work properly");
        if (!status.lib32_curl_installed) addLog("warn", "  ❌ lib32-curl not found");
        if (!status.lib32_openssl_installed) addLog("warn", "  ❌ lib32-openssl not found");
        if (!status.lib32_glibc_installed) addLog("warn", "  ❌ lib32-glibc not found");
      }
    } catch (error) {
      addLog("error", `Failed to check dependencies: ${error}`);
    } finally {
      setIsCheckingLib32Deps(false);
    }
  };

  // Browse for Steamless.exe
  const handleBrowseSteamlessExe = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Steamless", extensions: ["exe"] }],
      title: "Select Steamless.exe",
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : selected[0];
      setSteamlessPath(path);
      addLog("info", `Steamless.exe path set: ${path}`);
    }
  };

  // Launch Steamless via Wine
  const handleLaunchSteamless = async () => {
    if (!steamlessPath) {
      addLog("error", "Please select Steamless.exe path first");
      return;
    }
    setIsLaunchingSteamless(true);
    try {
      const { launchSteamlessViaWine } = await import("@/lib/api");
      const result = await launchSteamlessViaWine(steamlessPath);
      addLog("info", result);
    } catch (error) {
      addLog("error", `Failed to launch Steamless: ${error}`);
    } finally {
      setIsLaunchingSteamless(false);
    }
  };

  // Check if SLSah is installed
  const handleCheckSlsah = async () => {
    setIsCheckingSlsah(true);
    try {
      const { checkSlsahInstalled } = await import("@/lib/api");
      const installed = await checkSlsahInstalled();
      setSlsahInstalled(installed);
      addLog("info", installed ? "SLSah is installed ✅" : "SLSah is not installed");
    } catch (error) {
      addLog("error", `Failed to check SLSah: ${error}`);
    } finally {
      setIsCheckingSlsah(false);
    }
  };

  // Install SLSah
  const handleInstallSlsah = async () => {
    setIsInstallingSlsah(true);
    try {
      const { installSlsah } = await import("@/lib/api");
      const result = await installSlsah();
      addLog("info", result);
      setSlsahInstalled(true);
    } catch (error) {
      addLog("error", `Failed to install SLSah: ${error}`);
    } finally {
      setIsInstallingSlsah(false);
    }
  };

  // Launch SLSah
  const handleLaunchSlsah = async () => {
    setIsLaunchingSlsah(true);
    try {
      const { launchSlsah } = await import("@/lib/api");
      const result = await launchSlsah();
      addLog("info", result);
    } catch (error) {
      addLog("error", `Failed to launch SLSah: ${error}`);
    } finally {
      setIsLaunchingSlsah(false);
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

      {/* Local Mode: 32-bit Dependencies Warning */}
      {connectionMode === "local" && (
        <Card className="bg-[#1b2838] border-[#2a475e]">
          <CardHeader className="pb-3">
            <CardTitle className="text-white flex items-center gap-2">
              <AlertCircle className="w-5 h-5 text-yellow-400" />
              32-bit Library Dependencies (Local Mode)
            </CardTitle>
            <CardDescription>
              Steam requires these 32-bit libraries to function properly on your system.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* Status display */}
            {lib32DepsStatus && (
              <div className="bg-[#171a21] border border-[#0a0a0a] p-3 space-y-2 text-sm">
                <div className="grid grid-cols-1 gap-2">
                  <div className={lib32DepsStatus.lib32_curl_installed ? "text-green-400" : "text-red-400"}>
                    {lib32DepsStatus.lib32_curl_installed ? "✅" : "❌"} lib32-curl
                  </div>
                  <div className={lib32DepsStatus.lib32_openssl_installed ? "text-green-400" : "text-red-400"}>
                    {lib32DepsStatus.lib32_openssl_installed ? "✅" : "❌"} lib32-openssl
                  </div>
                  <div className={lib32DepsStatus.lib32_glibc_installed ? "text-green-400" : "text-red-400"}>
                    {lib32DepsStatus.lib32_glibc_installed ? "✅" : "❌"} lib32-glibc
                  </div>
                </div>
              </div>
            )}

            {/* Warning if not all installed */}
            {lib32DepsStatus && !lib32DepsStatus.all_installed && (
              <div className="bg-yellow-900/20 border border-yellow-600/50 p-3">
                <div className="flex items-start gap-2">
                  <AlertCircle className="w-5 h-5 text-yellow-400 flex-shrink-0 mt-0.5" />
                  <div className="text-sm">
                    <p className="font-medium text-yellow-400 mb-2">Missing Dependencies</p>
                    <p className="text-gray-300 mb-2">Install the missing libraries:</p>
                    <pre className="text-muted-foreground bg-[#171a21] p-2 rounded text-xs">
                      sudo pacman -S lib32-curl lib32-openssl lib32-glibc
                    </pre>
                  </div>
                </div>
              </div>
            )}

            {/* Success message */}
            {lib32DepsStatus && lib32DepsStatus.all_installed && (
              <div className="bg-green-900/20 border border-green-600/50 p-3">
                <div className="flex items-center gap-2 text-green-400">
                  <CheckCircle2 className="w-5 h-5 flex-shrink-0" />
                  <span className="text-sm">All required 32-bit libraries are installed.</span>
                </div>
              </div>
            )}

            <Button
              onClick={handleCheckLib32Dependencies}
              disabled={isCheckingLib32Deps}
              variant="outline"
              className="w-full border-[#2a475e] text-white hover:bg-[#2a475e]/50"
            >
              {isCheckingLib32Deps ? (
                <Loader2 className="w-4 h-4 animate-spin mr-2" />
              ) : (
                <RefreshCw className="w-4 h-4 mr-2" />
              )}
              Check Dependencies
            </Button>
          </CardContent>
        </Card>
      )}

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
          {/* Use Gist Key Checkbox */}
          <div className="flex items-center space-x-2">
            <Checkbox
              id="useGistKey"
              checked={useGistKey}
              onCheckedChange={async (checked) => {
                addLog("info", `Gist key checkbox changed: ${checked}`);
                setUseGistKey(!!checked);
                if (checked) {
                  setIsFetchingGistKey(true);
                  try {
                    const gistUrl = "https://gist.githubusercontent.com/ppogorze/592adb16ebf2cc27ce976bacf1928023/raw/gistfile1.txt";
                    addLog("info", `Fetching API key from Gist: ${gistUrl}`);
                    const response = await fetch(gistUrl);
                    addLog("info", `Gist fetch response status: ${response.status}`);
                    if (response.ok) {
                      const key = await response.text();
                      const trimmedKey = key.trim();
                      addLog("info", `Fetched API key from Gist: ${trimmedKey}`);
                      setLocalApiKey(trimmedKey);
                      // Auto-save
                      setSettings({ apiKey: trimmedKey });
                      await saveApiKey(trimmedKey);
                      addLog("info", "API key fetched from Gist and saved");
                    } else {
                      addLog("error", `Failed to fetch API key from Gist: HTTP ${response.status}`);
                      setUseGistKey(false);
                    }
                  } catch (error) {
                    addLog("error", `Failed to fetch Gist key: ${error}`);
                    setUseGistKey(false);
                  } finally {
                    setIsFetchingGistKey(false);
                  }
                }
              }}
            />
            <Label htmlFor="useGistKey" className="text-sm cursor-pointer flex items-center gap-2">
              Use shared key from Gist (auto-fetch)
              {isFetchingGistKey && <Loader2 className="w-3 h-3 animate-spin" />}
            </Label>
          </div>

          <div className="space-y-2">
            <Label htmlFor="apiKey">API Key</Label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  id="apiKey"
                  type={showApiKey ? "text" : "password"}
                  placeholder={useGistKey ? "Using key from Gist" : "Enter key (valid 24h)"}
                  value={localApiKey}
                  onChange={(e) => {
                    setLocalApiKey(e.target.value);
                    if (useGistKey) setUseGistKey(false); // Disable gist mode if user types
                  }}
                  disabled={isFetchingGistKey || useGistKey}
                  className={useGistKey ? "bg-[#1b2838] text-gray-400" : ""}
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
              <Button onClick={handleSaveApiKey} disabled={isSaving || isFetchingGistKey} className="btn-steam">
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
                    setSettingsSavedMessage("SteamGridDB API key saved successfully!");
                    setShowSettingsSavedDialog(true);
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

      {/* Steam Web API for Achievements */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">Steam Web API (for achievements)</CardTitle>
          <CardDescription>
            Get API key from <a href="https://steamcommunity.com/dev/apikey" target="_blank" rel="noopener noreferrer" className="text-[#67c1f5] hover:underline">steamcommunity.com/dev/apikey</a>.
            Used by "Generate Achievements" feature in game cards.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="steamApiKey">Steam API Key</Label>
            <Input
              id="steamApiKey"
              type="password"
              placeholder="Enter Steam Web API key"
              value={settings.steamApiKey || ""}
              onChange={(e) => setSettings({ steamApiKey: e.target.value })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="steamUserId">Steam User ID</Label>
            <Input
              id="steamUserId"
              placeholder="e.g., [U:1:12345678] or 76561198012345678"
              value={settings.steamUserId || ""}
              onChange={(e) => setSettings({ steamUserId: e.target.value })}
            />
            <p className="text-xs text-muted-foreground">
              Your Steam ID in any format. Used for generating user-specific achievement files.
            </p>
          </div>
          <Button
            onClick={async () => {
              try {
                const { saveToolSettings } = await import("@/lib/api");
                await saveToolSettings({
                  depotDownloaderPath: settings.depotDownloaderPath,
                  steamlessPath: settings.steamlessPath,
                  slssteamPath: settings.slssteamPath,
                  steamGridDbApiKey: settings.steamGridDbApiKey,
                  steamApiKey: settings.steamApiKey,
                  steamUserId: settings.steamUserId,
                });
                addLog("info", "Steam API settings saved");
                setSettingsSavedMessage("Steam API settings saved successfully!");
                setShowSettingsSavedDialog(true);
              } catch (e) {
                addLog("error", `Save error: ${e}`);
              }
            }}
            className="btn-steam w-full"
          >
            <Save className="w-4 h-4 mr-2" />
            Save Steam API Settings
          </Button>
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
                setSettingsSavedMessage("Paths and tools saved successfully!");
                setShowSettingsSavedDialog(true);
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
          <CardTitle className="text-white">
            {connectionMode === "local"
              ? "SLSsteam Installation (Local)"
              : "SLSsteam Installation (Remote - Steam Deck)"}
          </CardTitle>
          <CardDescription>
            {connectionMode === "local"
              ? "Install SLSsteam on this machine. Patches Steam to run games not in your library."
              : "Install SLSsteam on your Steam Deck via SSH. Requires active connection."}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="bg-[#2a475e] border border-[#1b2838] p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="w-5 h-5 text-[#67c1f5] flex-shrink-0 mt-0.5" />
              <div className="text-sm">
                <p className="font-medium text-white mb-1">
                  {connectionMode === "local" ? "Local installation requires:" : "Remote installation requires:"}
                </p>
                <ul className="list-disc list-inside text-muted-foreground space-y-1">
                  {connectionMode === "local" ? (
                    <>
                      <li>Creating an administrator password (passwd in terminal)</li>
                      <li>Sudo access for patching Steam launcher files</li>
                    </>
                  ) : (
                    <>
                      <li>SSH enabled on Steam Deck (Settings → Developer Options)</li>
                      <li>Admin password set on Steam Deck</li>
                      <li>Read-only mode disabled (run: sudo steamos-readonly disable)</li>
                    </>
                  )}
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
                {/* Read-only is only relevant for SteamOS/remote */}
                {connectionMode === "remote" && (
                  <div className={verifyStatus.is_readonly ? "text-red-400" : "text-green-400"}>
                    {verifyStatus.is_readonly ? "❌" : "✅"} Read-only: {verifyStatus.is_readonly ? "YES" : "NO"}
                  </div>
                )}
                <div className={verifyStatus.slssteam_so_exists ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.slssteam_so_exists ? "✅" : "❌"} SLSsteam.so
                </div>
                <div className={verifyStatus.library_inject_so_exists ? "text-green-400" : "text-yellow-400"}>
                  {verifyStatus.library_inject_so_exists ? "✅" : "⚠️"} library-inject.so
                </div>
                <div className={verifyStatus.config_exists ? "text-green-400" : "text-red-400"}>
                  {verifyStatus.config_exists ? "✅" : "❌"} config.yaml
                </div>

                {/* SafeMode, steam-jupiter, Desktop entry are SteamOS-specific */}
                {connectionMode === "remote" ? (
                  <>
                    <div className={verifyStatus.config_safe_mode_on ? "text-green-400" : "text-red-400"}>
                      {verifyStatus.config_safe_mode_on ? "✅" : "❌"} SafeMode
                    </div>
                    <div className={verifyStatus.steam_jupiter_patched ? "text-green-400" : "text-red-400"}>
                      {verifyStatus.steam_jupiter_patched ? "✅" : "❌"} steam-jupiter
                    </div>
                    <div className={verifyStatus.desktop_entry_patched ? "text-green-400" : "text-red-400"}>
                      {verifyStatus.desktop_entry_patched ? "✅" : "❌"} Desktop entry
                    </div>
                  </>
                ) : (
                  <>
                    <div className="text-gray-500">
                      ➖ SafeMode (SteamOS only)
                    </div>
                    <div className="text-gray-500">
                      ➖ steam-jupiter (SteamOS only)
                    </div>
                    <div className={verifyStatus.desktop_entry_patched ? "text-green-400" : "text-red-400"}>
                      {verifyStatus.desktop_entry_patched ? "✅" : "❌"} Desktop entry
                    </div>
                  </>
                )}
                <div className="text-[#67c1f5]">
                  🎮 Games: {verifyStatus.additional_apps_count}
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Disable Steam Updates */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">
            {connectionMode === "local"
              ? "Disable Steam Updates (Local)"
              : "Disable Steam Updates (Remote)"}
          </CardTitle>
          <CardDescription>
            Prevents Steam from auto-updating, which can cause hash mismatch with SLSsteam.
            Modifies <code className="text-[#67c1f5]">~/.steam/steam/steam.cfg</code>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="bg-[#2a475e] border border-[#1b2838] p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="w-5 h-5 text-[#67c1f5] flex-shrink-0 mt-0.5" />
              <div className="text-sm">
                <p className="font-medium text-white mb-1">This will add to steam.cfg:</p>
                <pre className="text-muted-foreground bg-[#171a21] p-2 rounded text-xs">
                  {`BootStrapperInhibitAll=enable
BootStrapperForceSelfUpdate=disable`}
                </pre>
              </div>
            </div>
          </div>

          {/* Status display */}
          {steamUpdatesStatus && (
            <div className="bg-[#171a21] border border-[#0a0a0a] p-3 space-y-2 text-sm">
              <div className="grid grid-cols-2 gap-2">
                <div className={steamUpdatesStatus.is_configured ? "text-green-400" : "text-red-400"}>
                  {steamUpdatesStatus.is_configured ? "✅" : "❌"} Status: {steamUpdatesStatus.is_configured ? "Configured" : "Not configured"}
                </div>
                <div className={steamUpdatesStatus.inhibit_all ? "text-green-400" : "text-red-400"}>
                  {steamUpdatesStatus.inhibit_all ? "✅" : "❌"} InhibitAll
                </div>
                <div className={steamUpdatesStatus.force_self_update_disabled ? "text-green-400" : "text-red-400"}>
                  {steamUpdatesStatus.force_self_update_disabled ? "✅" : "❌"} ForceSelfUpdate disabled
                </div>
              </div>
            </div>
          )}

          <div className="flex gap-2">
            <Button
              onClick={async () => {
                addLog("info", "Disabling Steam updates...");
                try {
                  const { disableSteamUpdates, checkSteamUpdatesStatus } = await import("@/lib/api");
                  const configToUse = { ...sshConfig };
                  if (connectionMode === "local") {
                    configToUse.is_local = true;
                  }
                  const result = await disableSteamUpdates(configToUse);
                  addLog("info", result);
                  // Refresh status
                  const status = await checkSteamUpdatesStatus(configToUse);
                  setSteamUpdatesStatus(status);
                  setSettingsSavedMessage("Steam updates have been disabled!");
                  setShowSettingsSavedDialog(true);
                } catch (error) {
                  addLog("error", `Failed to disable Steam updates: ${error}`);
                }
              }}
              disabled={connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)}
              className="btn-steam flex-1"
            >
              <Check className="w-4 h-4 mr-2" />
              Disable Steam Updates
            </Button>
            <Button
              onClick={async () => {
                setIsCheckingSteamUpdates(true);
                try {
                  const { checkSteamUpdatesStatus } = await import("@/lib/api");
                  const configToUse = { ...sshConfig };
                  if (connectionMode === "local") {
                    configToUse.is_local = true;
                  }
                  const status = await checkSteamUpdatesStatus(configToUse);
                  setSteamUpdatesStatus(status);
                  addLog("info", `Steam updates status: ${status.is_configured ? "Configured ✅" : "Not configured ❌"}`);
                } catch (error) {
                  addLog("error", `Failed to check status: ${error}`);
                } finally {
                  setIsCheckingSteamUpdates(false);
                }
              }}
              disabled={(connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) || isCheckingSteamUpdates}
              variant="outline"
              className="border-[#0a0a0a]"
            >
              {isCheckingSteamUpdates ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
              <span className="ml-2">Check Status</span>
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Fix libcurl32 */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white">
            {connectionMode === "local"
              ? "Fix libcurl32 (Local)"
              : "Fix libcurl32 (Remote)"}
          </CardTitle>
          <CardDescription>
            Creates a symlink to fix Steam library loading issues.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="bg-[#2a475e] border border-[#1b2838] p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="w-5 h-5 text-[#67c1f5] flex-shrink-0 mt-0.5" />
              <div className="text-sm">
                <p className="font-medium text-white mb-1">This will create:</p>
                <pre className="text-muted-foreground bg-[#171a21] p-2 rounded text-xs">
                  {`ln -sf /usr/lib32/libcurl.so.4 ~/.steam/steam/ubuntu12_32/libcurl.so.4`}
                </pre>
                <p className="mt-2 text-yellow-400">
                  ⚠️ Make sure <code className="bg-[#171a21] px-1 rounded">lib32-curl</code> is installed:
                </p>
                <pre className="text-muted-foreground bg-[#171a21] p-2 rounded text-xs mt-1">
                  {`sudo pacman -S lib32-curl`}
                </pre>
              </div>
            </div>
          </div>

          {/* Status display */}
          {libcurl32Status && (
            <div className="bg-[#171a21] border border-[#0a0a0a] p-3 space-y-2 text-sm">
              <div className="grid grid-cols-2 gap-2">
                <div className={libcurl32Status.source_exists ? "text-green-400" : "text-red-400"}>
                  {libcurl32Status.source_exists ? "✅" : "❌"} lib32-curl installed
                </div>
                <div className={libcurl32Status.symlink_exists ? "text-green-400" : "text-red-400"}>
                  {libcurl32Status.symlink_exists ? "✅" : "❌"} Symlink exists
                </div>
                <div className={libcurl32Status.symlink_correct ? "text-green-400" : "text-yellow-400"}>
                  {libcurl32Status.symlink_correct ? "✅ Correct target" : libcurl32Status.symlink_exists ? "⚠️ Wrong target" : "❌ No symlink"}
                </div>
              </div>
            </div>
          )}

          <div className="flex gap-2">
            <Button
              onClick={async () => {
                addLog("info", "Fixing libcurl32 symlink...");
                try {
                  const { fixLibcurl32, checkLibcurl32Status } = await import("@/lib/api");
                  const configToUse = { ...sshConfig };
                  if (connectionMode === "local") {
                    configToUse.is_local = true;
                  }
                  const result = await fixLibcurl32(configToUse);
                  addLog("info", result);
                  // Refresh status
                  const status = await checkLibcurl32Status(configToUse);
                  setLibcurl32Status(status);
                  setSettingsSavedMessage("libcurl32 symlink has been created!");
                  setShowSettingsSavedDialog(true);
                } catch (error) {
                  addLog("error", `Failed to fix libcurl32: ${error}`);
                }
              }}
              disabled={connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)}
              className="btn-steam flex-1"
            >
              <Check className="w-4 h-4 mr-2" />
              Fix libcurl32 Symlink
            </Button>
            <Button
              onClick={async () => {
                setIsCheckingLibcurl32(true);
                try {
                  const { checkLibcurl32Status } = await import("@/lib/api");
                  const configToUse = { ...sshConfig };
                  if (connectionMode === "local") {
                    configToUse.is_local = true;
                  }
                  const status = await checkLibcurl32Status(configToUse);
                  setLibcurl32Status(status);
                  addLog("info", `libcurl32 status: ${status.symlink_correct ? "OK ✅" : status.symlink_exists ? "Wrong target ⚠️" : "Not configured ❌"}`);
                } catch (error) {
                  addLog("error", `Failed to check status: ${error}`);
                } finally {
                  setIsCheckingLibcurl32(false);
                }
              }}
              disabled={(connectionMode === "remote" && (!sshConfig.ip || !sshConfig.password)) || isCheckingLibcurl32}
              variant="outline"
              className="border-[#0a0a0a]"
            >
              {isCheckingLibcurl32 ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
              <span className="ml-2">Check Status</span>
            </Button>
          </div>
        </CardContent>
      </Card>

      <Separator />

      {/* Tools Section */}
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <CardTitle className="text-white flex items-center gap-2">
            <Wrench className="w-5 h-5 text-[#67c1f5]" />
            Tools
          </CardTitle>
          <CardDescription>
            Additional tools for game management and configuration
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Steamless */}
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <Shield className="w-4 h-4 text-purple-400" />
              <span className="font-medium text-white">Steamless DRM Remover</span>
            </div>
            <p className="text-sm text-muted-foreground">
              Remove Steam DRM from game executables. Requires Wine/Proton on Linux.
            </p>
            <div className="flex items-center gap-2">
              <Input
                placeholder="Select Steamless.exe path..."
                value={steamlessPath}
                readOnly
                className="bg-[#2a475e] border-[#0a0a0a] flex-1"
              />
              <Button
                onClick={handleBrowseSteamlessExe}
                variant="outline"
                className="border-[#0a0a0a]"
              >
                <FolderOpen className="w-4 h-4 mr-2" />
                Browse
              </Button>
            </div>
            <Button
              onClick={handleLaunchSteamless}
              disabled={!steamlessPath || isLaunchingSteamless}
              className="btn-steam"
            >
              {isLaunchingSteamless ? (
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              ) : (
                <ExternalLink className="w-4 h-4 mr-2" />
              )}
              Launch Steamless (via Wine)
            </Button>
            <div className="text-xs text-muted-foreground">
              <a
                href="https://github.com/atom0s/Steamless/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="text-[#67c1f5] hover:underline flex items-center gap-1"
              >
                <Download className="w-3 h-3" />
                Download Steamless from GitHub
              </a>
            </div>
          </div>

          <Separator className="bg-[#2a475e]" />

          {/* SLSah */}
          {connectionMode === "local" && (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <Sparkles className="w-4 h-4 text-yellow-400" />
                <span className="font-medium text-white">SLSah - Achievement Helper</span>
                {slsahInstalled === true && (
                  <span className="text-xs bg-green-500/20 text-green-400 px-2 py-0.5 rounded">Installed</span>
                )}
                {slsahInstalled === false && (
                  <span className="text-xs bg-red-500/20 text-red-400 px-2 py-0.5 rounded">Not Installed</span>
                )}
              </div>
              <p className="text-sm text-muted-foreground">
                SLSsteam Achievement Helper - Generate achievement schemas and manage SLSsteam config.
              </p>
              <div className="flex items-center gap-2">
                {slsahInstalled ? (
                  <Button
                    onClick={handleLaunchSlsah}
                    disabled={isLaunchingSlsah}
                    className="btn-steam"
                  >
                    {isLaunchingSlsah ? (
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    ) : (
                      <Terminal className="w-4 h-4 mr-2" />
                    )}
                    Launch SLSah
                  </Button>
                ) : (
                  <Button
                    onClick={handleInstallSlsah}
                    disabled={isInstallingSlsah}
                    className="btn-steam"
                  >
                    {isInstallingSlsah ? (
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    ) : (
                      <Download className="w-4 h-4 mr-2" />
                    )}
                    Install SLSah
                  </Button>
                )}
                <Button
                  onClick={handleCheckSlsah}
                  disabled={isCheckingSlsah}
                  variant="outline"
                  className="border-[#0a0a0a]"
                >
                  {isCheckingSlsah ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <RefreshCw className="w-4 h-4" />
                  )}
                  <span className="ml-2">Check</span>
                </Button>
              </div>
              <div className="text-xs text-muted-foreground">
                <a
                  href="https://github.com/niwia/SLSah"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-[#67c1f5] hover:underline flex items-center gap-1"
                >
                  <ExternalLink className="w-3 h-3" />
                  SLSah on GitHub
                </a>
              </div>
            </div>
          )}

          {connectionMode === "remote" && (
            <div className="text-sm text-muted-foreground bg-[#2a475e]/50 p-3 rounded">
              ℹ️ SLSah is only available in Local mode (runs on Steam Deck/Linux device).
            </div>
          )}
        </CardContent>
      </Card>

      <Separator />

      {/* Info */}
      <div className="text-sm text-muted-foreground text-center">
        <p>TonTonDeck v1.0.0</p>
      </div>

      {/* SLSsteam Installed Success Dialog */}
      <AlertDialog open={showSlssteamInstalledDialog} onOpenChange={setShowSlssteamInstalledDialog}>
        <AlertDialogContent className="bg-[#1b2838] border-[#2a475e]">
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2 text-white">
              <CheckCircle2 className="w-6 h-6 text-green-500" />
              SLSsteam Installed Successfully!
            </AlertDialogTitle>
            <AlertDialogDescription className="text-gray-400">
              SLSsteam has been installed and configured. Please restart Steam for the changes to take effect.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogAction
              onClick={() => setShowSlssteamInstalledDialog(false)}
              className="btn-steam"
            >
              OK
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Settings Saved Dialog */}
      <AlertDialog open={showSettingsSavedDialog} onOpenChange={setShowSettingsSavedDialog}>
        <AlertDialogContent className="bg-[#1b2838] border-[#2a475e]">
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2 text-white">
              <CheckCircle2 className="w-6 h-6 text-green-500" />
              Settings Saved
            </AlertDialogTitle>
            <AlertDialogDescription className="text-gray-400">
              {settingsSavedMessage}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogAction
              onClick={() => setShowSettingsSavedDialog(false)}
              className="btn-steam"
            >
              OK
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
