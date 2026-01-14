import { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { FolderOpen, Save, Check, Wrench } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

export function ToolPathsPanel() {
    const { settings, setSettings, addLog } = useAppStore();
    const [isSaving, setIsSaving] = useState(false);
    const [justSaved, setJustSaved] = useState(false);

    const handleBrowseDepotDownloader = async () => {
        try {
            const selected = await open({
                multiple: false,
                directory: false,
                title: "Select DepotDownloaderMod binary"
            });
            if (selected) {
                const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
                setSettings({ depotDownloaderPath: path });
            }
        } catch (error) {
            addLog("error", `Failed to select file: ${error}`);
        }
    };

    const handleBrowseSteamless = async () => {
        try {
            const selected = await open({
                multiple: false,
                directory: false,
                filters: [{ name: "Steamless", extensions: ["exe"] }],
                title: "Select Steamless.exe"
            });
            if (selected) {
                const path = typeof selected === 'string' ? selected : (selected as { path?: string })?.path || String(selected);
                setSettings({ steamlessPath: path });
            }
        } catch (error) {
            addLog("error", `Failed to select file: ${error}`);
        }
    };

    const handleSave = async () => {
        setIsSaving(true);
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
            addLog("info", "Tool settings saved successfully");
            setJustSaved(true);
            setTimeout(() => setJustSaved(false), 2000);
        } catch (e) {
            addLog("error", `Failed to save tool settings: ${e}`);
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <Card className="bg-[#1b2838] border-[#2a475e]">
            <CardHeader className="pb-3">
                <CardTitle className="text-white flex items-center gap-2">
                    <Wrench className="w-5 h-5 text-[#67c1f5]" />
                    External Tools
                </CardTitle>
                <CardDescription>
                    Configure paths for required external tools.
                </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="space-y-2">
                    <Label htmlFor="depotDownloader">DepotDownloaderMod Path</Label>
                    <div className="flex gap-2">
                        <Input
                            id="depotDownloader"
                            value={settings.depotDownloaderPath || ""}
                            placeholder="/path/to/DepotDownloaderMod"
                            onChange={(e) => setSettings({ depotDownloaderPath: e.target.value })}
                            className="bg-[#0a0a0a] border-[#2a475e]"
                        />
                        <Button onClick={handleBrowseDepotDownloader} variant="secondary" size="icon" className="border-[#2a475e]">
                            <FolderOpen className="w-4 h-4" />
                        </Button>
                    </div>
                </div>

                <div className="space-y-2">
                    <Label htmlFor="steamless">Steamless.exe Path (Optional)</Label>
                    <div className="flex gap-2">
                        <Input
                            id="steamless"
                            value={settings.steamlessPath || ""}
                            placeholder="/path/to/Steamless.exe"
                            onChange={(e) => setSettings({ steamlessPath: e.target.value })}
                            className="bg-[#0a0a0a] border-[#2a475e]"
                        />
                        <Button onClick={handleBrowseSteamless} variant="secondary" size="icon" className="border-[#2a475e]">
                            <FolderOpen className="w-4 h-4" />
                        </Button>
                    </div>
                </div>

                <div className="pt-2">
                    <Button
                        onClick={handleSave}
                        disabled={isSaving}
                        className="btn-steam w-full"
                    >
                        {justSaved ? (
                            <>
                                <Check className="w-4 h-4 mr-2" />
                                Saved
                            </>
                        ) : (
                            <>
                                <Save className="w-4 h-4 mr-2" />
                                Save Tool Paths
                            </>
                        )}
                    </Button>
                </div>
            </CardContent>
        </Card>
    );
}
