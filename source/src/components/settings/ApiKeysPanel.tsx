/**
 * API Keys Panel - manages Depot Provider and SteamGridDB API keys
 */
import { useState, useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Save, Eye, EyeOff, Loader2, Check, RefreshCw, Trophy, LogIn } from "lucide-react";

export function ApiKeysPanel() {
    const { settings, setSettings, addLog, steamCmLoggedIn, setSteamCmLoggedIn } = useAppStore();

    // Local state
    const [showApiKey, setShowApiKey] = useState(false);
    const [localApiKey, setLocalApiKey] = useState(settings.apiKey);
    const [isSaving, setIsSaving] = useState(false);
    const [isFetchingFromUrl, setIsFetchingFromUrl] = useState(false);
    const [showUrlPassword, setShowUrlPassword] = useState(false);
    const [justSaved, setJustSaved] = useState(false);
    const [justSavedGrid, setJustSavedGrid] = useState(false);
    const [justSavedSteam, setJustSavedSteam] = useState(false); // For Steam Web API

    // Batch achievement generation state
    const [isGeneratingAll, setIsGeneratingAll] = useState(false);
    const [batchResult, setBatchResult] = useState<{
        processed: number;
        skipped: number;
        errors: number;
    } | null>(null);

    // Steam CM login state
    const [isLoggingIn, setIsLoggingIn] = useState(false);
    const [loginError, setLoginError] = useState<string | null>(null);

    // Sync local key when settings change externally (e.g., on app load from backend)
    useEffect(() => setLocalApiKey(settings.apiKey), [settings.apiKey]);

    const handleSaveApiKey = async () => {
        setIsSaving(true);
        try {
            const { saveApiKey } = await import("@/lib/api");
            await saveApiKey(localApiKey);
            setSettings({ apiKey: localApiKey });
            addLog("info", "Depot Provider API key saved successfully");

            setJustSaved(true);
            setTimeout(() => setJustSaved(false), 2000);
        } catch (error) {
            addLog("error", `Failed to save API key: ${error}`);
        } finally {
            setIsSaving(false);
        }
    };

    const handleSaveGridDbKey = async () => {
        try {
            const { saveToolSettings } = await import("@/lib/api");
            await saveToolSettings({
                depotDownloaderPath: settings.depotDownloaderPath,
                steamlessPath: settings.steamlessPath,
                slssteamPath: settings.slssteamPath,
                steamGridDbApiKey: settings.steamGridDbApiKey,
            });
            addLog("info", "SteamGridDB API key saved");

            setJustSavedGrid(true);
            setTimeout(() => setJustSavedGrid(false), 2000);
        } catch (e) {
            addLog("error", `Save error: ${e}`);
        }
    };

    const handleSaveSteamWebApiKey = async () => {
        try {
            const { saveToolSettings } = await import("@/lib/api");
            await saveToolSettings({
                depotDownloaderPath: settings.depotDownloaderPath,
                steamlessPath: settings.steamlessPath,
                slssteamPath: settings.slssteamPath,
                steamGridDbApiKey: settings.steamGridDbApiKey,
                steamApiKey: settings.steamApiKey,
                steamUserId: settings.steamUserId,
                steamUsername: settings.steamUsername,
                steamPassword: settings.steamPassword,
                achievementMethod: settings.achievementMethod,
            });
            addLog("info", "Achievement settings saved");

            setJustSavedSteam(true);
            setTimeout(() => setJustSavedSteam(false), 2000);
        } catch (e) {
            addLog("error", `Save error: ${e}`);
        }
    };

    // Batch generate achievements for all AdditionalApps
    const handleGenerateAll = async () => {
        setIsGeneratingAll(true);
        setBatchResult(null);
        try {
            const { generateAllAchievements } = await import("@/lib/api/misc");

            if (!settings.steamApiKey) {
                addLog("error", "Steam Web API Key is required");
                return;
            }
            if (!settings.steamUserId) {
                addLog("error", "Steam User ID is required");
                return;
            }

            const result = await generateAllAchievements(settings.steamApiKey, settings.steamUserId);
            setBatchResult({
                processed: result.processed,
                skipped: result.skipped,
                errors: result.errors
            });

            if (result.processed > 0) {
                addLog("info", `Generated achievements for ${result.processed} games`);
            }
            if (result.errors > 0) {
                addLog("warn", `Failed to generate achievements for ${result.errors} games`);
            }
            result.messages.forEach(msg => addLog("info", msg));
        } catch (e) {
            const errorMsg = e instanceof Error ? e.message : String(e);
            addLog("error", `Achievement generation failed: ${errorMsg}`);
        } finally {
            setIsGeneratingAll(false);
        }
    };

    // Handle Steam CM login
    const handleSteamLogin = async () => {
        if (!settings.steamUsername || !settings.steamPassword) {
            addLog("error", "Steam username and password required");
            return;
        }

        setIsLoggingIn(true);
        setLoginError(null);
        addLog("info", "Logging in to Steam... Check your Steam mobile app to approve.");

        try {
            const { invoke } = await import("@tauri-apps/api/core");
            await invoke("steam_cm_login", {
                steamUsername: settings.steamUsername,
                steamPassword: settings.steamPassword
            });
            setSteamCmLoggedIn(true);
            addLog("info", "Steam login successful! You can now generate achievements.");
        } catch (e) {
            const errorMsg = e instanceof Error ? e.message : String(e);
            setSteamCmLoggedIn(false);
            setLoginError(errorMsg);
            addLog("error", `Steam login failed: ${errorMsg}`);
        } finally {
            setIsLoggingIn(false);
        }
    };

    return (
        <div className="space-y-6">
            {/* Depot Provider API */}
            <Card className="bg-[#1b2838] border-[#2a475e]">
                <CardHeader className="pb-3">
                    <CardTitle className="text-white">Depot Provider API</CardTitle>
                    <CardDescription>
                        Log in via Discord at <a href="https://manifest.morrenus.xyz" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">manifest.morrenus.xyz</a> and create an API key.<br />
                        Free key: 25 manifests per day, must be regenerated every 24 hours.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Use API Key from URL Checkbox */}
                    <div className="flex items-center space-x-2">
                        <Checkbox
                            id="useApiKeyUrl"
                            checked={settings.useApiKeyUrl}
                            onCheckedChange={(checked) => {
                                setSettings({ useApiKeyUrl: !!checked });
                            }}
                        />
                        <Label htmlFor="useApiKeyUrl" className="text-sm cursor-pointer">
                            Fetch API key from txt file URL
                        </Label>
                    </div>

                    {/* URL and Auth fields (shown when checkbox is checked) */}
                    {settings.useApiKeyUrl && (
                        <div className="space-y-3 pl-6 border-l-2 border-[#2a475e]">
                            <div className="space-y-2">
                                <Label htmlFor="apiKeyUrl">URL to txt file (http/https)</Label>
                                <Input
                                    id="apiKeyUrl"
                                    type="url"
                                    placeholder="https://example.com/api-key.txt"
                                    value={settings.apiKeyUrl || ""}
                                    onChange={(e) => setSettings({ apiKeyUrl: e.target.value })}
                                />
                            </div>
                            <div className="grid grid-cols-2 gap-3">
                                <div className="space-y-2">
                                    <Label htmlFor="apiKeyUrlUsername">HTTP Auth Username (optional)</Label>
                                    <Input
                                        id="apiKeyUrlUsername"
                                        placeholder="username"
                                        value={settings.apiKeyUrlUsername || ""}
                                        onChange={(e) => setSettings({ apiKeyUrlUsername: e.target.value })}
                                    />
                                </div>
                                <div className="space-y-2">
                                    <Label htmlFor="apiKeyUrlPassword">HTTP Auth Password (optional)</Label>
                                    <div className="relative">
                                        <Input
                                            id="apiKeyUrlPassword"
                                            type={showUrlPassword ? "text" : "password"}
                                            placeholder="password"
                                            value={settings.apiKeyUrlPassword || ""}
                                            onChange={(e) => setSettings({ apiKeyUrlPassword: e.target.value })}
                                        />
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="absolute right-0 top-0 h-full hover:bg-transparent"
                                            onClick={() => setShowUrlPassword(!showUrlPassword)}
                                        >
                                            {showUrlPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                        </Button>
                                    </div>
                                </div>
                            </div>
                            <Button
                                onClick={async () => {
                                    if (!settings.apiKeyUrl) {
                                        addLog("error", "Please enter a URL to fetch the API key from");
                                        return;
                                    }
                                    setIsFetchingFromUrl(true);
                                    try {
                                        addLog("info", `Fetching API key from URL...`);
                                        const headers: HeadersInit = {};
                                        if (settings.apiKeyUrlUsername && settings.apiKeyUrlPassword) {
                                            const credentials = btoa(`${settings.apiKeyUrlUsername}:${settings.apiKeyUrlPassword}`);
                                            headers["Authorization"] = `Basic ${credentials}`;
                                        }
                                        const response = await fetch(settings.apiKeyUrl, { headers });
                                        if (response.ok) {
                                            const key = await response.text();
                                            const trimmedKey = key.trim();
                                            setLocalApiKey(trimmedKey);
                                            setSettings({ apiKey: trimmedKey });
                                            const { saveApiKey } = await import("@/lib/api");
                                            await saveApiKey(trimmedKey);
                                            addLog("info", "API key fetched from URL and saved");
                                        } else {
                                            addLog("error", `Failed to fetch API key: HTTP ${response.status}`);
                                        }
                                    } catch (error) {
                                        addLog("error", `Failed to fetch API key: ${error}`);
                                    } finally {
                                        setIsFetchingFromUrl(false);
                                    }
                                }}
                                disabled={isFetchingFromUrl || !settings.apiKeyUrl}
                                variant="outline"
                                className="w-full border-[#2a475e] text-white hover:bg-[#2a475e]/50"
                            >
                                {isFetchingFromUrl ? (
                                    <>
                                        <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                        Fetching...
                                    </>
                                ) : (
                                    "Fetch API Key from URL"
                                )}
                            </Button>
                        </div>
                    )}

                    <div className="space-y-2">
                        <Label htmlFor="apiKey">API Key</Label>
                        <div className="flex gap-2">
                            <div className="relative flex-1">
                                <Input
                                    id="apiKey"
                                    type={showApiKey ? "text" : "password"}
                                    placeholder={settings.useApiKeyUrl ? "Using key from URL" : "Enter key (valid 24h)"}
                                    value={localApiKey}
                                    onChange={(e) => {
                                        setLocalApiKey(e.target.value);
                                        if (settings.useApiKeyUrl) setSettings({ useApiKeyUrl: false });
                                    }}
                                    disabled={isFetchingFromUrl || settings.useApiKeyUrl}
                                    className={settings.useApiKeyUrl ? "bg-[#1b2838] text-gray-400" : ""}
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
                            <Button
                                onClick={handleSaveApiKey}
                                disabled={isSaving || isFetchingFromUrl}
                                className="btn-steam min-w-[100px]"
                            >
                                {justSaved ? (
                                    <>
                                        <Check className="w-4 h-4 mr-2" />
                                        Saved
                                    </>
                                ) : (
                                    <>
                                        <Save className="w-4 h-4 mr-2" />
                                        Save
                                    </>
                                )}
                            </Button>
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* SteamGridDB API */}
            <Card className="bg-[#1b2838] border-[#2a475e]">
                <CardHeader className="pb-3">
                    <CardTitle className="text-white">SteamGridDB API (optional for covers)</CardTitle>
                    <CardDescription>Get API key from <a href="https://www.steamgriddb.com/profile/preferences/api" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">steamgriddb.com</a></CardDescription>
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
                                onClick={handleSaveGridDbKey}
                                className="btn-steam min-w-[100px]"
                            >
                                {justSavedGrid ? (
                                    <>
                                        <Check className="w-4 h-4 mr-2" />
                                        Saved
                                    </>
                                ) : (
                                    <>
                                        <Save className="w-4 h-4 mr-2" />
                                        Save
                                    </>
                                )}
                            </Button>
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Steam Achievements */}
            <Card className="bg-[#1b2838] border-[#2a475e]">
                <CardHeader className="pb-3">
                    <CardTitle className="text-white">Steam Achievements</CardTitle>
                    <CardDescription>
                        Choose method for generating achievement schema files.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Method Toggle */}
                    <div className="space-y-2">
                        <Label>Generation Method</Label>
                        <div className="grid grid-cols-2 gap-2">
                            <Button
                                variant={settings.achievementMethod === "web_api" ? "default" : "outline"}
                                className={settings.achievementMethod === "web_api"
                                    ? "btn-steam"
                                    : "border-[#2a475e] text-white hover:bg-[#2a475e]/50"}
                                onClick={async () => {
                                    const { saveAchievementMethod } = await import("@/lib/api");
                                    await saveAchievementMethod("web_api");
                                    setSettings({ achievementMethod: "web_api" });
                                    addLog("info", "Achievement method: Web API (SLSah)");
                                }}
                            >
                                Web API (SLSah)
                            </Button>
                            <Button
                                variant={settings.achievementMethod === "steam_cm" ? "default" : "outline"}
                                className={settings.achievementMethod === "steam_cm"
                                    ? "btn-steam"
                                    : "border-[#2a475e] text-white hover:bg-[#2a475e]/50"}
                                onClick={async () => {
                                    const { saveAchievementMethod } = await import("@/lib/api");
                                    await saveAchievementMethod("steam_cm");
                                    setSettings({ achievementMethod: "steam_cm" });
                                    addLog("info", "Achievement method: Steam CM (SLScheevo)");
                                }}
                            >
                                Steam CM (SLScheevo)
                            </Button>
                        </div>
                        <p className="text-xs text-muted-foreground">
                            {settings.achievementMethod === "steam_cm"
                                ? "Requires Steam login. Approve on Steam mobile app when prompted."
                                : "Uses Steam Web API. Requires API key."}
                        </p>
                    </div>

                    {/* Web API fields (only shown when web_api selected) */}
                    {settings.achievementMethod === "web_api" && (
                        <>
                            <div className="space-y-2">
                                <Label htmlFor="steamApiKey">Steam Web API Key</Label>
                                <p className="text-xs text-muted-foreground">
                                    Get from <a href="https://steamcommunity.com/dev/apikey" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">steamcommunity.com/dev/apikey</a>
                                </p>
                                <Input
                                    id="steamApiKey"
                                    type="password"
                                    placeholder="Enter Steam Web API key"
                                    value={settings.steamApiKey || ""}
                                    onChange={(e) => setSettings({ steamApiKey: e.target.value })}
                                />
                            </div>
                        </>
                    )}

                    {/* Steam CM fields (only shown when steam_cm selected) */}
                    {settings.achievementMethod === "steam_cm" && (
                        <>
                            <div className="space-y-2">
                                <Label htmlFor="steamUsername">Steam Username</Label>
                                <Input
                                    id="steamUsername"
                                    placeholder="Your Steam account name"
                                    value={settings.steamUsername || ""}
                                    onChange={(e) => setSettings({ steamUsername: e.target.value })}
                                />
                            </div>
                            <div className="space-y-2">
                                <Label htmlFor="steamPassword">Steam Password</Label>
                                <Input
                                    id="steamPassword"
                                    type="password"
                                    placeholder="Your Steam password"
                                    value={settings.steamPassword || ""}
                                    onChange={(e) => setSettings({ steamPassword: e.target.value })}
                                />
                                <p className="text-xs text-yellow-400">
                                    ⚠️ Password is used only during generation. Approve on Steam mobile app.
                                </p>
                            </div>

                            {/* Login to Steam button */}
                            <Button
                                onClick={handleSteamLogin}
                                disabled={isLoggingIn || !settings.steamUsername || !settings.steamPassword}
                                className={steamCmLoggedIn ? "w-full bg-green-700 hover:bg-green-600" : "btn-steam w-full"}
                            >
                                {isLoggingIn ? (
                                    <>
                                        <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                        Waiting for mobile app...
                                    </>
                                ) : steamCmLoggedIn ? (
                                    <>
                                        <Check className="w-4 h-4 mr-2" />
                                        Logged In
                                    </>
                                ) : (
                                    <>
                                        <LogIn className="w-4 h-4 mr-2" />
                                        Login to Steam
                                    </>
                                )}
                            </Button>
                            {loginError && (
                                <p className="text-xs text-red-400">
                                    {loginError}
                                </p>
                            )}
                        </>
                    )}

                    {/* Steam User ID (only needed for Web API method) */}
                    {settings.achievementMethod === "web_api" && (
                        <div className="space-y-2">
                            <Label htmlFor="steamUserId">Steam User ID (steamID3)</Label>
                            <p className="text-xs text-muted-foreground">
                                Find at <a href="https://steamid.io/lookup" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">steamid.io/lookup</a>
                            </p>
                            <Input
                                id="steamUserId"
                                placeholder="e.g. 12345678"
                                value={settings.steamUserId || ""}
                                onChange={(e) => setSettings({ steamUserId: e.target.value })}
                            />
                        </div>
                    )}

                    <Button
                        onClick={handleSaveSteamWebApiKey}
                        className="btn-steam w-full"
                    >
                        {justSavedSteam ? (
                            <>
                                <Check className="w-4 h-4 mr-2" />
                                Saved Achievement Settings
                            </>
                        ) : (
                            <>
                                <Save className="w-4 h-4 mr-2" />
                                Save Achievement Settings
                            </>
                        )}
                    </Button>

                    {/* Batch generate for all games */}
                    <div className="pt-4 border-t border-[#2a475e]">
                        <div className="flex items-center gap-2 mb-2">
                            <Trophy className="w-4 h-4 text-yellow-500" />
                            <span className="text-sm font-medium text-gray-300">Batch Generation</span>
                        </div>
                        <p className="text-xs text-muted-foreground mb-3">
                            Generate achievement schemas for all games in AdditionalApps that don't have them yet.
                        </p>

                        {batchResult && (
                            <div className="text-xs mb-3 space-y-0.5">
                                <div className="text-green-400">✓ {batchResult.processed} generated</div>
                                {batchResult.skipped > 0 && (
                                    <div className="text-gray-500">• {batchResult.skipped} skipped (already exist)</div>
                                )}
                                {batchResult.errors > 0 && (
                                    <div className="text-red-400">✗ {batchResult.errors} errors</div>
                                )}
                            </div>
                        )}

                        <Button
                            onClick={handleGenerateAll}
                            disabled={isGeneratingAll || !settings.steamApiKey || !settings.steamUserId}
                            variant="secondary"
                            className="w-full"
                        >
                            {isGeneratingAll ? (
                                <>
                                    <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                                    Generating...
                                </>
                            ) : (
                                <>
                                    <Trophy className="w-4 h-4 mr-2" />
                                    Generate All Achievements
                                </>
                            )}
                        </Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
