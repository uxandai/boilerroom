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
  
  // UI State
  activeTab: string;
  
  // Installation
  installProgress: InstallProgress | null;
  
  // Logs
  logs: LogEntry[];
  
  // Settings
  settings: Settings;
  
  // Actions
  setSshConfig: (config: Partial<SshConfig>) => void;
  setConnectionStatus: (status: ConnectionStatus) => void;
  setConnectionMode: (mode: "remote" | "local") => void;
  toggleConnectionCheck: () => void;
  setSearchQuery: (query: string) => void;
  setSearchResults: (results: SearchResult[]) => void;
  setIsSearching: (searching: boolean) => void;
  setTriggerSearch: (trigger: boolean) => void;
  setActiveTab: (tab: string) => void;
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
};

export const useAppStore = create<AppState>((set) => ({
  // Initial state
  sshConfig: defaultSshConfig,
  connectionStatus: "offline",
  connectionMode: "remote", // Default to remote
  connectionCheckPaused: false,
  
  searchQuery: "",
  searchResults: [],
  isSearching: false,
  triggerSearch: false,
  activeTab: "search",
  installProgress: null,
  logs: [],
  settings: { ...defaultSettings },

  // Actions
  setSshConfig: (config) => set((state) => ({ sshConfig: { ...state.sshConfig, ...config } })),
  setConnectionStatus: (status) => set({ connectionStatus: status }),
  setConnectionMode: (mode) => set({ connectionMode: mode }),
  toggleConnectionCheck: () => set((state) => ({ connectionCheckPaused: !state.connectionCheckPaused })),

  setSearchQuery: (query) => set({ searchQuery: query }),
  
  setTriggerSearch: (trigger) => set({ triggerSearch: trigger }),
  
  setActiveTab: (tab) => set({ activeTab: tab }),

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
}));
