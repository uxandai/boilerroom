/**
 * API Keys Panel - manages Morrenus and SteamGridDB API keys
 */
import { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Save, Eye, EyeOff, Loader2, Check } from "lucide-react";

export function ApiKeysPanel() {
    const { settings, setSettings, addLog } = useAppStore();

    // Local state
    const [showApiKey, setShowApiKey] = useState(false);
    const [localApiKey, setLocalApiKey] = useState(settings.apiKey);
    const [isSaving, setIsSaving] = useState(false);
    const [isFetchingGistKey, setIsFetchingGistKey] = useState(false);
    const [justSaved, setJustSaved] = useState(false);
    const [justSavedGrid, setJustSavedGrid] = useState(false);
    const [justSavedSteam, setJustSavedSteam] = useState(false); // For Steam Web API

    // Sync local key when settings change externally
    // useEffect(() => setLocalApiKey(settings.apiKey), [settings.apiKey]);

    const handleSaveApiKey = async () => {
        setIsSaving(true);
        try {
            const { saveApiKey } = await import("@/lib/api");
            await saveApiKey(localApiKey);
            setSettings({ apiKey: localApiKey });
            addLog("info", "Morrenus API key saved successfully");

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
            });
            addLog("info", "Steam Web API settings saved");

            setJustSavedSteam(true);
            setTimeout(() => setJustSavedSteam(false), 2000);
        } catch (e) {
            addLog("error", `Save error: ${e}`);
        }
    };

    return (
        <div className="space-y-6">
            {/* Morrenus API */}
            <Card className="bg-[#1b2838] border-[#2a475e]">
                <CardHeader className="pb-3">
                    <CardTitle className="text-white">Morrenus API</CardTitle>
                    <CardDescription>
                        Log in via Discord at <a href="https://manifest.morrenus.xyz" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">manifest.morrenus.xyz</a> and create an API key.<br />
                        Free key: 25 manifests per day, must be regenerated every 24 hours.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Use Gist Key Checkbox */}
                    <div className="flex items-center space-x-2">
                        <Checkbox
                            id="useGistKey"
                            checked={settings.useGistKey}
                            onCheckedChange={async (checked) => {
                                setSettings({ useGistKey: !!checked });
                                if (checked) {
                                    setIsFetchingGistKey(true);
                                    try {
                                        const gistUrl = "https://gist.githubusercontent.com/ppogorze/592adb16ebf2cc27ce976bacf1928023/raw/gistfile1.txt";
                                        addLog("info", `Fetching API key from Gist...`);
                                        const response = await fetch(gistUrl);
                                        if (response.ok) {
                                            const key = await response.text();
                                            const trimmedKey = key.trim();
                                            setLocalApiKey(trimmedKey);
                                            setSettings({ apiKey: trimmedKey });
                                            const { saveApiKey } = await import("@/lib/api");
                                            await saveApiKey(trimmedKey);
                                            addLog("info", "API key fetched from Gist and saved");
                                        } else {
                                            addLog("error", `Failed to fetch API key from Gist: HTTP ${response.status}`);
                                            setSettings({ useGistKey: false });
                                        }
                                    } catch (error) {
                                        addLog("error", `Failed to fetch Gist key: ${error}`);
                                        setSettings({ useGistKey: false });
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
                                    placeholder={settings.useGistKey ? "Using key from Gist" : "Enter key (valid 24h)"}
                                    value={localApiKey}
                                    onChange={(e) => {
                                        setLocalApiKey(e.target.value);
                                        if (settings.useGistKey) setSettings({ useGistKey: false });
                                    }}
                                    disabled={isFetchingGistKey || settings.useGistKey}
                                    className={settings.useGistKey ? "bg-[#1b2838] text-gray-400" : ""}
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
                                disabled={isSaving || isFetchingGistKey}
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

            {/* Steam Web API */}
            <Card className="bg-[#1b2838] border-[#2a475e]">
                <CardHeader className="pb-3">
                    <CardTitle className="text-white">Steam Web API (Achivements)</CardTitle>
                    <CardDescription>
                        Get API key from <a href="https://steamcommunity.com/dev/apikey" target="_blank" rel="noreferrer" className="text-[#67c1f5] hover:underline">steamcommunity.com/dev/apikey</a>.
                        Used for generating game achievement files.
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="space-y-2">
                        <Label htmlFor="steamApiKey">Steam Web API Key</Label>
                        <Input
                            id="steamApiKey"
                            type="password"
                            placeholder="Enter Steam Web API key"
                            value={settings.steamApiKey || ""}
                            onChange={(e) => setSettings({ steamApiKey: e.target.value })}
                        />
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="steamUserId">Steam User ID (64-bit)</Label>
                        <Input
                            id="steamUserId"
                            placeholder="e.g. 76561198012345678"
                            value={settings.steamUserId || ""}
                            onChange={(e) => setSettings({ steamUserId: e.target.value })}
                        />
                        <p className="text-xs text-muted-foreground">
                            Required for creating user-specific achievement data.
                        </p>
                    </div>
                    <Button
                        onClick={handleSaveSteamWebApiKey}
                        className="btn-steam w-full"
                    >
                        {justSavedSteam ? (
                            <>
                                <Check className="w-4 h-4 mr-2" />
                                Saved Steam Web API Settings
                            </>
                        ) : (
                            <>
                                <Save className="w-4 h-4 mr-2" />
                                Save Steam Web API Settings
                            </>
                        )}
                    </Button>
                </CardContent>
            </Card>
        </div>
    );
}
