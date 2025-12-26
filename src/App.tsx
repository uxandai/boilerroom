import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { SearchPanel } from "@/components/SearchPanel";
import { SettingsPanel } from "@/components/SettingsPanel";
import { LogsPanel } from "@/components/LogsPanel";
import { LibraryPanel } from "@/components/LibraryPanel";
import { InstallProgress } from "@/components/InstallProgress";
import { DeckStatus } from "@/components/DeckStatus";
import { ModeSelectionScreen } from "@/components/ModeSelectionScreen";
import { useAppStore } from "@/store/useAppStore";
import { getApiKey, loadConnectionMode } from "@/lib/api";
import { Power } from "lucide-react";
import { useEffect, useState, useRef } from "react";
import "./index.css";

function App() {
  const { setSettings, setSshConfig, activeTab, setActiveTab, setConnectionMode, triggerLibraryRefresh } = useAppStore();
  const [showModeSelection, setShowModeSelection] = useState<boolean | null>(null);

  // Track if initialization has occurred (prevents StrictMode double-call)
  const hasInitializedRef = useRef(false);

  // Load saved settings on startup
  useEffect(() => {
    // Prevent duplicate initialization from React StrictMode
    if (hasInitializedRef.current) return;
    hasInitializedRef.current = true;

    const loadSettings = async () => {
      try {
        // Check if mode was already selected
        const savedMode = await loadConnectionMode();
        if (savedMode) {
          setConnectionMode(savedMode);
          setShowModeSelection(false);
        } else {
          setShowModeSelection(true);
        }

        // Load API key
        const savedApiKey = await getApiKey();
        if (savedApiKey) {
          setSettings({ apiKey: savedApiKey });
        }

        // Load SSH config
        const { loadSshConfig, loadToolSettings } = await import("@/lib/api");
        const savedSshConfig = await loadSshConfig();
        if (savedSshConfig) {
          setSshConfig(savedSshConfig);

          // Auto-connect if SSH config was saved
          if (savedSshConfig.ip && savedSshConfig.password) {
            try {
              const { checkDeckStatus, testSshConnection } = await import("@/lib/api");
              const status = await checkDeckStatus(savedSshConfig.ip, savedSshConfig.port);
              if (status === "online") {
                await testSshConnection(savedSshConfig);
                useAppStore.getState().setConnectionStatus("ssh_ok");
                useAppStore.getState().triggerLibraryRefresh(); // Auto-load library on connect
                console.log("Auto-connected to Steam Deck");
              }
            } catch {
              console.log("Auto-connect failed, Deck may be offline");
            }
          }
        }

        // Load tool settings
        const savedToolSettings = await loadToolSettings();
        if (savedToolSettings) {
          setSettings({
            depotDownloaderPath: savedToolSettings.depotDownloaderPath,
            steamlessPath: savedToolSettings.steamlessPath,
            slssteamPath: savedToolSettings.slssteamPath,
            steamGridDbApiKey: savedToolSettings.steamGridDbApiKey || "",
          });
        }

        // Load cached SLSsteam version from disk
        const { getCachedSlssteamVersion, getCachedSlssteamPath } = await import("@/lib/api");
        const cachedVersion = await getCachedSlssteamVersion();
        const cachedPath = await getCachedSlssteamPath();
        if (cachedVersion || cachedPath) {
          setSettings({
            slssteamVersion: cachedVersion || undefined,
            slssteamPath: cachedPath || "",
          });
        }
      } catch (error) {
        console.error("Failed to load settings:", error);
        setShowModeSelection(true);
      }
    };
    loadSettings();
  }, [setSettings, setSshConfig, setConnectionMode]);

  // Listen for install progress
  useEffect(() => {
    import("@tauri-apps/api/event").then(({ listen }) => {
      const unlisten = listen("install-progress", (event: any) => {
        const payload = event.payload;
        const currentProgress = useAppStore.getState().installProgress;
        useAppStore.getState().setInstallProgress({
          step: payload.state,
          appId: currentProgress?.appId || "unknown",
          gameName: currentProgress?.gameName || "Unknown Game",
          heroImage: currentProgress?.heroImage,
          downloadPercent: payload.download_percent || 0,
          downloadSpeed: payload.download_speed || "",
          eta: payload.eta || "",
          filesTotal: payload.files_total || 0,
          filesTransferred: payload.files_transferred || 0,
          bytesTotal: payload.bytes_total || 0,
          bytesTransferred: payload.bytes_transferred || 0,
          transferSpeed: payload.transfer_speed || "",
          message: payload.message,
          error: payload.state === "error" ? payload.message : undefined,
        });
      });

      return () => {
        unlisten.then(f => f());
      };
    });
  }, []);

  // Show loading state while checking mode
  if (showModeSelection === null) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="animate-pulse">
          <img src="/logo.png" alt="TonTonDeck" className="h-24 w-auto opacity-50 mix-blend-screen" />
        </div>
      </div>
    );
  }

  // Show mode selection screen if not yet selected
  if (showModeSelection) {
    return <ModeSelectionScreen onModeSelected={() => {
      setShowModeSelection(false);
      triggerLibraryRefresh(); // Auto-load library when mode is selected
    }} />;
  }


  return (
    <div className="min-h-screen bg-background text-foreground">
      {/* Header - draggable for window movement */}
      <header data-tauri-drag-region className="bg-[#171a21] border-b border-[#0a0a0a] px-4 py-3 select-none cursor-move">
        <div className="flex items-center justify-between pointer-events-none">
          <div className="flex items-center gap-3 pointer-events-auto">
            <img src="/logo.png" alt="TonTonDeck" className="h-12 w-auto mix-blend-screen" />
          </div>
          <div className="pointer-events-auto">
            <DeckStatus />
          </div>
        </div>
      </header>

      {/* Install Progress (shows when active) */}
      <InstallProgress />

      {/* Main Content */}
      <div className="px-4">
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList className="w-full justify-start bg-transparent border-b border-[#2a475e] p-0 h-auto gap-0 rounded-none mb-0">
            <TabsTrigger
              value="search"
              className="px-6 py-3 text-sm font-bold uppercase tracking-wide rounded-none border-b-2 border-transparent data-[state=active]:border-[#67c1f5] data-[state=active]:text-[#67c1f5] data-[state=active]:bg-transparent text-gray-400 hover:text-white bg-transparent"
            >
              Download
            </TabsTrigger>
            <TabsTrigger
              value="library"
              className="px-6 py-3 text-sm font-bold uppercase tracking-wide rounded-none border-b-2 border-transparent data-[state=active]:border-[#67c1f5] data-[state=active]:text-[#67c1f5] data-[state=active]:bg-transparent text-gray-400 hover:text-white bg-transparent"
            >
              Library
            </TabsTrigger>
            <TabsTrigger
              value="settings"
              className="px-6 py-3 text-sm font-bold uppercase tracking-wide rounded-none border-b-2 border-transparent data-[state=active]:border-[#67c1f5] data-[state=active]:text-[#67c1f5] data-[state=active]:bg-transparent text-gray-400 hover:text-white bg-transparent"
            >
              Settings
            </TabsTrigger>
            <TabsTrigger
              value="logs"
              className="px-6 py-3 text-sm font-bold uppercase tracking-wide rounded-none border-b-2 border-transparent data-[state=active]:border-[#67c1f5] data-[state=active]:text-[#67c1f5] data-[state=active]:bg-transparent text-gray-400 hover:text-white bg-transparent"
            >
              Logs
            </TabsTrigger>

            <button
              onClick={() => {
                import('@tauri-apps/plugin-process').then(({ exit }) => exit(0));
              }}
              className="ml-auto flex items-center gap-2 px-4 py-3 text-sm font-bold uppercase tracking-wide text-red-400 hover:text-red-300"
            >
              <Power className="w-4 h-4" />
              Exit
            </button>
          </TabsList>

          <TabsContent value="search" className="mt-4">
            <SearchPanel />
          </TabsContent>

          <TabsContent value="settings" className="mt-4">
            <SettingsPanel />
          </TabsContent>

          <TabsContent value="logs" className="mt-4">
            <LogsPanel />
          </TabsContent>

          <TabsContent value="library" className="mt-4">
            <LibraryPanel />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}

export default App;
