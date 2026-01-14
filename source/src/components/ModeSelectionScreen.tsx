import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Monitor, Wifi, Loader2, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";

interface ModeSelectionScreenProps {
  onModeSelected: (mode: "local" | "remote") => void;
}

export function ModeSelectionScreen({ onModeSelected }: ModeSelectionScreenProps) {
  const { setConnectionMode } = useAppStore();
  const [isDetecting, setIsDetecting] = useState(true);
  const [detectedAsSteamDeck, setDetectedAsSteamDeck] = useState(false);
  const [osName, setOsName] = useState<string>("");

  // Auto-detect Steam Deck on mount
  useEffect(() => {
    const detect = async () => {
      try {
        const { detectSteamDeck } = await import("@/lib/api");
        const result = await detectSteamDeck();
        setDetectedAsSteamDeck(result.is_steam_deck);
        setOsName(result.os_name);

        // If detected as Steam Deck, auto-select local mode after a short delay
        if (result.is_steam_deck) {
          setTimeout(() => {
            handleSelectMode("local");
          }, 2000); // Give user 2s to see the detection message
        }
      } catch (error) {
        console.log("Platform detection failed:", error);
      } finally {
        setIsDetecting(false);
      }
    };
    detect();
  }, []);

  const handleSelectMode = async (mode: "local" | "remote") => {
    setConnectionMode(mode);

    // Persist the selection
    const { Store } = await import("@tauri-apps/plugin-store");
    const store = await Store.load("settings.json");
    await store.set("connectionMode", mode);
    await store.save();

    onModeSelected(mode);
  };

  // Show loading while detecting
  if (isDetecting) {
    return (
      <div className="min-h-screen bg-background flex flex-col items-center justify-center p-8">
        <img src="/logo.png" alt="BoilerRoom" className="h-64 w-auto mb-2 mix-blend-screen" />
        <Loader2 className="w-8 h-8 text-[#67c1f5] animate-spin mb-4" />
        <p className="text-muted-foreground">Detecting platform...</p>
      </div>
    );
  }

  // Auto-detected as Steam Deck - showing confirmation
  if (detectedAsSteamDeck) {
    return (
      <div className="min-h-screen bg-background flex flex-col items-center justify-center p-8">
        <img src="/logo.png" alt="BoilerRoom" className="h-64 w-auto mb-2 mix-blend-screen" />
        <div className="bg-[#2a4c28] border border-[#408f40] rounded-lg p-6 max-w-md text-center">
          <Sparkles className="w-12 h-12 text-green-400 mx-auto mb-4" />
          <h2 className="text-xl font-bold text-white mb-2">Steam Deck Detected! 🎮</h2>
          <p className="text-green-300/80 mb-4">
            Detected system: <strong>{osName}</strong>
          </p>
          <p className="text-sm text-muted-foreground">
            Automatically enabling "On this device" mode...
          </p>
        </div>
        <button
          onClick={() => setDetectedAsSteamDeck(false)}
          className="mt-6 text-sm text-muted-foreground hover:text-white underline"
        >
          I want to choose a different mode
        </button>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background flex flex-col items-center justify-center p-8 relative">
      {/* Logo */}
      <div className="mb-0">
        <img src="/logo.png" alt="BoilerRoom" className="h-64 w-auto mix-blend-screen" />
      </div>

      <h1 className="text-2xl font-bold text-white mb-2">Select Operating Mode</h1>
      <p className="text-muted-foreground mb-8 text-center max-w-md">
        {osName && <span className="text-xs">Detected system: {osName}</span>}
      </p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 max-w-2xl w-full">
        {/* Local Mode - This Machine */}
        <Card
          className="bg-[#1b2838] border-[#2a475e] hover:border-[#67c1f5] cursor-pointer transition-all hover:scale-[1.02] group"
          onClick={() => handleSelectMode("local")}
        >
          <CardHeader className="pb-3">
            <div className="w-16 h-16 rounded-full bg-[#2a475e] flex items-center justify-center mb-4 group-hover:bg-[#67c1f5]/20 transition-colors">
              <Monitor className="w-8 h-8 text-[#67c1f5]" />
            </div>
            <CardTitle className="text-white text-xl">On This Device</CardTitle>
            <CardDescription className="text-gray-400">
              Steam Deck / Machine / Linux
            </CardDescription>
          </CardHeader>
          <CardContent>
            <ul className="text-sm text-muted-foreground space-y-2">
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                Install directly to disk
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                No SSH configuration required
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                Supports Arch-based Linux
              </li>
            </ul>
          </CardContent>
        </Card>

        {/* Remote Mode - Upload from PC */}
        <Card
          className="bg-[#1b2838] border-[#2a475e] hover:border-[#67c1f5] cursor-pointer transition-all hover:scale-[1.02] group"
          onClick={() => handleSelectMode("remote")}
        >
          <CardHeader className="pb-3">
            <div className="w-16 h-16 rounded-full bg-[#2a475e] flex items-center justify-center mb-4 group-hover:bg-[#67c1f5]/20 transition-colors">
              <Wifi className="w-8 h-8 text-[#67c1f5]" />
            </div>
            <CardTitle className="text-white text-xl">Remote Transfer</CardTitle>
            <CardDescription className="text-gray-400">
              Send remotely from PC to Steam Deck
            </CardDescription>
          </CardHeader>
          <CardContent>
            <ul className="text-sm text-muted-foreground space-y-2">
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                Download on PC, transfer to Deck
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                Requires SSH connection
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-400">✓</span>
                Faster downloads via PC
              </li>
            </ul>
          </CardContent>
        </Card>
      </div>

      <p className="text-xs text-muted-foreground mt-8 text-center">
        BoilerRoom is an independent project. Not affiliated with or supported by Valve/Steam, SLSsteam, Steamless :wink:<br />Used for copying files you have legal rights to. Do not use for license/rights violations or bypassing protections.
      </p>
    </div>
  );
}
