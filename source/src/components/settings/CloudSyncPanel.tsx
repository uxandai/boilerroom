import { useState, useEffect } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Cloud, Eye, EyeOff, Loader2, Check, AlertCircle, RefreshCw } from "lucide-react";
import { useAppStore } from "@/store/useAppStore";
import { invoke } from "@tauri-apps/api/core";

// Types matching backend
interface CloudSyncConfig {
  enabled: boolean;
  provider: string; // "webdav", "gdrive", "dropbox", "onedrive"
  webdav_url: string;
  username: string;
  password: string;
}

interface GlobalCloudStatus {
  enabled: boolean;
  is_syncing: boolean;
  games_synced: number;
  games_pending: number;
  games_with_conflicts: number;
  last_sync: string | null;
}

export function CloudSyncPanel() {
  const { addLog, connectionMode } = useAppStore();
  
  const [config, setConfig] = useState<CloudSyncConfig>({
    enabled: false,
    provider: "webdav",
    webdav_url: "",
    username: "",
    password: "",
  });
  
  const [showPassword, setShowPassword] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [globalStatus, setGlobalStatus] = useState<GlobalCloudStatus | null>(null);

  // Load config on mount
  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const savedConfig = await invoke<CloudSyncConfig | null>("get_cloudsync_config");
      if (savedConfig) {
        setConfig(savedConfig);
      }
      
      // Also load global status
      const status = await invoke<GlobalCloudStatus>("get_global_cloud_status");
      setGlobalStatus(status);
    } catch (e) {
      addLog("error", `Failed to load CloudSync config: ${e}`);
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    setTestResult(null);
    
    try {
      await invoke("save_cloudsync_config", { config });
      addLog("info", "CloudSync configuration saved");
      setTestResult({ success: true, message: "Configuration saved" });
      
      // Refresh status
      const status = await invoke<GlobalCloudStatus>("get_global_cloud_status");
      setGlobalStatus(status);
    } catch (e) {
      addLog("error", `Failed to save CloudSync config: ${e}`);
      setTestResult({ success: false, message: String(e) });
    } finally {
      setIsSaving(false);
    }
  };

  const handleTestConnection = async () => {
    if (!config.webdav_url) {
      setTestResult({ success: false, message: "WebDAV URL is required" });
      return;
    }

    setIsTesting(true);
    setTestResult(null);

    try {
      // Save first, then test
      await invoke("save_cloudsync_config", { config });
      const result = await invoke<string>("test_cloudsync_connection", { config });
      setTestResult({ success: true, message: result });
      addLog("info", `CloudSync test: ${result}`);
    } catch (e) {
      const errorMsg = String(e);
      setTestResult({ success: false, message: errorMsg });
      addLog("error", `CloudSync test failed: ${errorMsg}`);
    } finally {
      setIsTesting(false);
    }
  };

  const handleToggleEnabled = async (enabled: boolean) => {
    const newConfig = { ...config, enabled };
    setConfig(newConfig);
    
    // Auto-save when toggling
    try {
      await invoke("save_cloudsync_config", { config: newConfig });
      addLog("info", `CloudSync ${enabled ? "enabled" : "disabled"}`);
    } catch (e) {
      addLog("error", `Failed to update CloudSync: ${e}`);
    }
  };

  // Only show for local mode
  if (connectionMode !== "local") {
    return null;
  }

  return (
    <Card className="bg-[#1b2838] border-[#2a475e]">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Cloud className="w-5 h-5 text-[#67c1f5]" />
            <CardTitle className="text-white">CloudSync</CardTitle>
          </div>
          <Switch
            checked={config.enabled}
            onCheckedChange={handleToggleEnabled}
          />
        </div>
        <CardDescription>
          Sync game saves to your own WebDAV server (Nextcloud, ownCloud, etc.)
        </CardDescription>
      </CardHeader>

      {config.enabled && (
        <CardContent className="space-y-4">
          {/* Status indicator */}
          {globalStatus && (
            <div className={`p-3 rounded-md border ${
              globalStatus.games_pending > 0 
                ? "bg-[#4c4428] border-[#8f8040]" 
                : "bg-[#2a4c28] border-[#408f40]"
            }`}>
              <div className="flex items-center gap-2 text-sm">
                {globalStatus.is_syncing ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin text-[#67c1f5]" />
                    <span className="text-[#67c1f5]">Syncing...</span>
                  </>
                ) : globalStatus.games_pending > 0 ? (
                  <>
                    <Cloud className="w-4 h-4 text-[#ffcc6b]" />
                    <span className="text-[#ffcc6b]">
                      {globalStatus.games_pending} game(s) pending sync
                    </span>
                  </>
                ) : (
                  <>
                    <Check className="w-4 h-4 text-[#6bff6b]" />
                    <span className="text-[#6bff6b]">Cloud saves synced</span>
                  </>
                )}
              </div>
            </div>
          )}

          {/* Help text */}
          <div className="p-3 rounded-md border bg-[#0d1821] border-[#2a475e] text-xs text-muted-foreground">
            <p className="font-medium text-white mb-2">☁️ WebDAV Setup:</p>
            <ol className="list-decimal list-inside space-y-1">
              <li>Get your WebDAV URL from your cloud provider</li>
              <li>For Nextcloud: <code className="bg-[#1b2838] px-1 rounded">https://your-server/remote.php/dav/files/username/</code></li>
              <li>Enter your cloud account credentials below</li>
              <li>Test the connection to verify settings</li>
            </ol>
          </div>



          {/* Provider Selection */}
          <div className="space-y-3 pt-2">
            <Label>Cloud Provider</Label>
            <RadioGroup
              value={config.provider || "webdav"}
              onValueChange={(val) => setConfig({ ...config, provider: val })}
              className="flex gap-2"
            >
              <div className="flex-1">
                <RadioGroupItem value="webdav" id="prov-webdav" className="peer sr-only" />
                <Label
                  htmlFor="prov-webdav"
                  className="flex flex-col items-center justify-between rounded-md border-2 border-muted bg-popover p-4 hover:bg-accent hover:text-accent-foreground peer-data-[state=checked]:border-[#67c1f5] peer-data-[state=checked]:bg-[#2a475e] cursor-pointer"
                >
                  WebDAV
                </Label>
              </div>
              <div>
                <RadioGroupItem value="gdrive" id="prov-gdrive" className="peer sr-only" />
                <Label
                  htmlFor="prov-gdrive"
                  className="flex flex-col items-center justify-between rounded-md border-2 border-muted bg-popover p-4 hover:bg-accent hover:text-accent-foreground peer-data-[state=checked]:border-[#67c1f5] peer-data-[state=checked]:bg-[#2a475e] cursor-pointer"
                >
                  Google Drive
                </Label>
              </div>
              <div className="flex-1">
                <RadioGroupItem value="dropbox" id="prov-dropbox" className="peer sr-only" />
                <Label
                  htmlFor="prov-dropbox"
                  className="flex flex-col items-center justify-between rounded-md border-2 border-muted bg-popover p-4 hover:bg-accent hover:text-accent-foreground peer-data-[state=checked]:border-[#67c1f5] peer-data-[state=checked]:bg-[#2a475e] cursor-pointer"
                >
                  Dropbox
                </Label>
              </div>
              <div>
                 <RadioGroupItem value="other" id="prov-other" className="peer sr-only" />
                 <Label
                   htmlFor="prov-other"
                   className="flex flex-col items-center justify-between rounded-md border-2 border-muted bg-popover p-4 hover:bg-accent hover:text-accent-foreground peer-data-[state=checked]:border-[#67c1f5] peer-data-[state=checked]:bg-[#2a475e] cursor-pointer"
                 >
                   Other
                 </Label>
               </div>
            </RadioGroup>
          </div>

          {/* WebDAV URL - Only show for WebDAV or Other */}
          {(config.provider === 'webdav' || config.provider === 'other' || !config.provider) && (
          <div className="space-y-2">
            <Label htmlFor="webdav-url">WebDAV URL</Label>
            <Input
              id="webdav-url"
              placeholder="https://cloud.example.com/remote.php/dav/files/user/"
              value={config.webdav_url}
              onChange={(e) => setConfig({ ...config, webdav_url: e.target.value })}
            />
          </div>
          )}

          {/* Coming Soon message for others */}
          {config.provider !== 'webdav' && config.provider !== 'other' && config.provider && (
              <div className="p-4 rounded-md bg-[#1b2838] border border-[#2a475e] text-center text-gray-400">
                  <p className="font-medium">Direct integration coming soon!</p>
                  <p className="text-sm">For now, please use WebDAV if your provider supports it, or use the provider's official desktop client to sync a local folder and point BoilerRoom logic to it (future feature).</p>
              </div>
          )}

          {/* Username */}
          <div className="space-y-2">
            <Label htmlFor="webdav-username">Username</Label>
            <Input
              id="webdav-username"
              placeholder="your-username"
              value={config.username}
              onChange={(e) => setConfig({ ...config, username: e.target.value })}
            />
          </div>

          {/* Password */}
          <div className="space-y-2">
            <Label htmlFor="webdav-password">Password / App Token</Label>
            <div className="relative">
              <Input
                id="webdav-password"
                type={showPassword ? "text" : "password"}
                placeholder="your-password"
                value={config.password}
                onChange={(e) => setConfig({ ...config, password: e.target.value })}
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

          {/* Buttons */}
          <div className="flex gap-2">
            <Button
              onClick={handleTestConnection}
              disabled={isTesting || !config.webdav_url}
              variant="outline"
              className="flex-1 border-[#2a475e] text-white hover:bg-[#2a475e]/50"
            >
              {isTesting ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Testing...
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Test Connection
                </>
              )}
            </Button>
            <Button
              onClick={handleSave}
              disabled={isSaving}
              className="flex-1 btn-steam"
            >
              {isSaving ? (
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              ) : (
                "Save Settings"
              )}
            </Button>
          </div>

          {/* Test result */}
          {testResult && (
            <div className={`p-3 rounded-md border ${
              testResult.success 
                ? "bg-[#2a4c28] border-[#408f40]" 
                : "bg-[#4c2828] border-[#8f4040]"
            }`}>
              <div className={`flex items-center gap-2 text-sm ${
                testResult.success ? "text-[#6bff6b]" : "text-[#ff6b6b]"
              }`}>
                {testResult.success ? (
                  <Check className="w-4 h-4 flex-shrink-0" />
                ) : (
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                )}
                <span>{testResult.message}</span>
              </div>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
}
