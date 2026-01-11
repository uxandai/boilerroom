import { create } from "zustand";

// Types
export type ConnectionStatus = "offline" | "online" | "ssh_ok";
export type InstallStep = "idle" | "downloading" | "uploading" | "extracting" | "configuring" | "done" | "error";

export interface SshConfig {
  ip: string;
  port: number;
  username: string;
  password: string;
  privateKeyPath: string;
  is_local?: boolean;
}

export interface SearchResult {
  game_id: string;
  game_name: string;
  manifest_available: boolean;
  manifest_size?: number;
  header_image?: string;
  depot_id?: number;
  manifest_id?: number;
}

export interface LogEntry {
  timestamp: Date;
  level: "info" | "warn" | "error";
  message: string;
}

export interface InstallProgress {
  step: string; // "downloading", "steamless", "transferring", "configuring", "finished", "error", "cancelled"
  appId: string;
  gameName: string;
  heroImage?: string;
  downloadPercent: number;
  downloadSpeed: string;  // e.g. "12.5 MB/s"
  eta: string;            // e.g. "2m 30s"
  filesTotal: number;
  filesTransferred: number;
  bytesTotal: number;
  bytesTransferred: number;
  transferSpeed: string; // e.g. "45.2 MB/s"
  currentFile?: string;  // Current file being transferred (truncated for display)
  currentFilePercent?: number; // Per-file progress 0-100
  error?: string;
  message?: string;
}

export interface Settings {
  apiKey: string;
  defaultSshConfig: SshConfig;
  targetDirectory: string;
  depotDownloaderPath: string;
  steamlessPath: string;
  slssteamPath: string;
  slssteamVersion?: string; // Cached SLSsteam version from GitHub
  steamGridDbApiKey: string; // Optional SteamGridDB API key for game artwork
  steamApiKey: string; // Steam Web API key for achievements
  steamUserId: string; // Steam User ID for achievements (format: [U:1:xxxxxxxxx])
  useGistKey?: boolean; // Whether to use shared API key from Gist
}

export interface QueueItem {
  game: SearchResult;
  depotIds: string[];
  manifestIds: string[];
  manifestFiles: string[];
  depotKeys: [string, string][];
  targetPath: string;
  config: SshConfig; // Config needed for this specific install
  depotDownloaderPath: string;
  steamlessPath: string;
  appToken?: string;
}

interface AppState {
  // Connection
  sshConfig: SshConfig;
  connectionStatus: ConnectionStatus;
  connectionMode: "remote" | "local"; // New mode
  connectionCheckPaused: boolean;

  // Search
  searchQuery: string;
  searchResults: SearchResult[];
  isSearching: boolean;
  triggerSearch: boolean; // Flag to auto-trigger search
  libraryNeedsRefresh: boolean; // Flag to auto-trigger library refresh

  // UI State
  activeTab: string;
  setupWizardOpen: boolean; // Setup wizard modal visibility

  // Installation
  installProgress: InstallProgress | null;

  // Logs
  logs: LogEntry[];

  // Settings
  settings: Settings;

  // Queue
  installQueue: QueueItem[];
  addToQueue: (item: QueueItem) => void;
  removeFromQueue: (appId: string) => void;
  popQueue: () => QueueItem | undefined;

  // Actions
  setSshConfig: (config: Partial<SshConfig>) => void;
  setConnectionStatus: (status: ConnectionStatus) => void;
  setConnectionMode: (mode: "remote" | "local") => void;
  toggleConnectionCheck: () => void;
  setSearchQuery: (query: string) => void;
  setSearchResults: (results: SearchResult[]) => void;
  setIsSearching: (searching: boolean) => void;
  setTriggerSearch: (trigger: boolean) => void;
  triggerLibraryRefresh: () => void;
  setActiveTab: (tab: string) => void;
  setSetupWizardOpen: (open: boolean) => void;
  setInstallProgress: (progress: InstallProgress | null) => void;
  addLog: (level: LogEntry["level"], message: string) => void;
  clearLogs: () => void;
  setSettings: (settings: Partial<Settings>) => void;
}

const defaultSshConfig: SshConfig = {
  ip: "",
  port: 22,
  username: "deck",
  password: "",
  privateKeyPath: "",
};

const defaultSettings: Settings = {
  apiKey: "",
  defaultSshConfig: { ...defaultSshConfig },
  targetDirectory: "/home/deck/.local/share/Steam/steamapps/common/",
  depotDownloaderPath: "",
  steamlessPath: "",
  slssteamPath: "",
  steamGridDbApiKey: "",
  steamApiKey: "",
  steamUserId: "",
  useGistKey: false,
};

export const useAppStore = create<AppState>((set, get) => ({
  // Initial state
  sshConfig: defaultSshConfig,
  connectionStatus: "offline",
  connectionMode: "remote", // Default to remote
  connectionCheckPaused: false,

  searchQuery: "",
  searchResults: [],
  isSearching: false,
  triggerSearch: false,
  libraryNeedsRefresh: false,
  activeTab: "search",
  setupWizardOpen: false,
  installProgress: null,
  logs: [],
  settings: { ...defaultSettings },

  // Actions
  setSshConfig: (config) => set((state) => ({ sshConfig: { ...state.sshConfig, ...config } })),
  setConnectionStatus: (status) => set({ connectionStatus: status }),
  setConnectionMode: (mode) => set({ connectionMode: mode, libraryNeedsRefresh: true }),
  toggleConnectionCheck: () => set((state) => ({ connectionCheckPaused: !state.connectionCheckPaused })),

  setSearchQuery: (query) => set({ searchQuery: query }),

  setTriggerSearch: (trigger) => set({ triggerSearch: trigger }),
  triggerLibraryRefresh: () => set({ libraryNeedsRefresh: true }),

  setActiveTab: (tab) => set({ activeTab: tab }),
  setSetupWizardOpen: (open) => set({ setupWizardOpen: open }),

  setSearchResults: (results) => set({ searchResults: results }),

  setIsSearching: (searching) => set({ isSearching: searching }),

  setInstallProgress: (progress) => set({ installProgress: progress }),

  addLog: (level, message) =>
    set((state) => ({
      logs: [
        ...state.logs,
        { timestamp: new Date(), level, message },
      ].slice(-500), // Keep last 500 logs
    })),

  clearLogs: () => set({ logs: [] }),

  setSettings: (settings) =>
    set((state) => ({
      settings: { ...state.settings, ...settings },
    })),

  installQueue: [],
  addToQueue: (item) => set((state) => ({ installQueue: [...state.installQueue, item] })),
  removeFromQueue: (appId) => set((state) => ({ installQueue: state.installQueue.filter(i => i.game.game_id !== appId) })),
  popQueue: () => {
    const state = get();
    if (state.installQueue.length === 0) return undefined;
    const item = state.installQueue[0];
    set({ installQueue: state.installQueue.slice(1) });
    return item;
  },
}));
