import {
    ButtonItem,
    definePlugin,
    DialogButton,
    Field,
    Focusable,
    Navigation,
    PanelSection,
    PanelSectionRow,
    ProgressBarWithInfo,
    ServerAPI,
    SidebarNavigation,
    staticClasses,
    TextField,
    ToggleField,
} from "@decky/ui";
import { routerHook } from "@decky/api";
import { useState, useEffect, VFC } from "react";
import { FaSync, FaPlug, FaGamepad, FaCog } from "react-icons/fa";

// Types
interface Game {
    name: string;
    app_id: string;
    size_bytes: number;
}

interface Settings {
    pc_ip: string;
    pc_user: string;
    pc_password: string;
    steam_path: string;
}

// Utility function to format bytes
function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

// ============================================================================
// Settings Panel
// ============================================================================
const SettingsPanel: VFC<{ serverAPI: ServerAPI }> = ({ serverAPI }) => {
    const [settings, setSettings] = useState<Settings>({
        pc_ip: "",
        pc_user: "",
        pc_password: "",
        steam_path: "~/.steam/steam",
    });
    const [testing, setTesting] = useState(false);
    const [testResult, setTestResult] = useState<string | null>(null);
    const [saving, setSaving] = useState(false);

    // Load settings on mount
    useEffect(() => {
        serverAPI.callPluginMethod<{}, Settings>("get_settings", {}).then((res) => {
            if (res.success && res.result) {
                setSettings(res.result);
            }
        });
    }, []);

    const saveSettings = async () => {
        setSaving(true);
        const res = await serverAPI.callPluginMethod<Settings, boolean>(
            "save_settings",
            settings
        );
        setSaving(false);
        if (res.success && res.result) {
            setTestResult("Settings saved!");
        } else {
            setTestResult("Failed to save settings");
        }
    };

    const testConnection = async () => {
        setTesting(true);
        setTestResult(null);
        const res = await serverAPI.callPluginMethod<
            {},
            { success: boolean; error?: string }
        >("test_connection", {});
        setTesting(false);
        if (res.success && res.result?.success) {
            setTestResult("✓ Connected successfully!");
        } else {
            setTestResult("✗ " + (res.result?.error || "Connection failed"));
        }
    };

    return (
        <PanelSection title="PC Connection">
            <PanelSectionRow>
                <Field label="PC IP Address">
                    <TextField
                        value={settings.pc_ip}
                        onChange={(e) =>
                            setSettings({ ...settings, pc_ip: e.target.value })
                        }
                        placeholder="192.168.1.100"
                    />
                </Field>
            </PanelSectionRow>
            <PanelSectionRow>
                <Field label="Username">
                    <TextField
                        value={settings.pc_user}
                        onChange={(e) =>
                            setSettings({ ...settings, pc_user: e.target.value })
                        }
                        placeholder="deck"
                    />
                </Field>
            </PanelSectionRow>
            <PanelSectionRow>
                <Field label="Password">
                    <TextField
                        value={settings.pc_password}
                        onChange={(e) =>
                            setSettings({ ...settings, pc_password: e.target.value })
                        }
                        bIsPassword={true}
                        placeholder="••••••••"
                    />
                </Field>
            </PanelSectionRow>
            <PanelSectionRow>
                <Field label="Steam Path">
                    <TextField
                        value={settings.steam_path}
                        onChange={(e) =>
                            setSettings({ ...settings, steam_path: e.target.value })
                        }
                        placeholder="~/.steam/steam"
                    />
                </Field>
            </PanelSectionRow>
            <PanelSectionRow>
                <ButtonItem layout="below" onClick={saveSettings} disabled={saving}>
                    {saving ? "Saving..." : "Save Settings"}
                </ButtonItem>
            </PanelSectionRow>
            <PanelSectionRow>
                <ButtonItem layout="below" onClick={testConnection} disabled={testing}>
                    <FaPlug style={{ marginRight: 8 }} />
                    {testing ? "Testing..." : "Test Connection"}
                </ButtonItem>
            </PanelSectionRow>
            {testResult && (
                <PanelSectionRow>
                    <Field label="Status">{testResult}</Field>
                </PanelSectionRow>
            )}
        </PanelSection>
    );
};

// ============================================================================
// Game List Panel
// ============================================================================
const GameListPanel: VFC<{ serverAPI: ServerAPI }> = ({ serverAPI }) => {
    const [games, setGames] = useState<Game[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [syncing, setSyncing] = useState<string | null>(null);
    const [syncStatus, setSyncStatus] = useState<string | null>(null);

    const fetchGames = async () => {
        setLoading(true);
        setError(null);
        const res = await serverAPI.callPluginMethod<
            {},
            { success: boolean; games: Game[]; error?: string }
        >("get_pc_games", {});
        setLoading(false);

        if (res.success && res.result?.success) {
            setGames(res.result.games);
        } else {
            setError(res.result?.error || "Failed to fetch games");
        }
    };

    const syncGame = async (game: Game) => {
        setSyncing(game.name);
        setSyncStatus(`Syncing ${game.name}...`);

        const res = await serverAPI.callPluginMethod<
            { game_name: string; app_id: string },
            { success: boolean; message?: string; error?: string }
        >("sync_game", { game_name: game.name, app_id: game.app_id });

        setSyncing(null);

        if (res.success && res.result?.success) {
            setSyncStatus(`✓ ${res.result.message}`);
        } else {
            setSyncStatus(`✗ ${res.result?.error || "Sync failed"}`);
        }
    };

    useEffect(() => {
        fetchGames();
    }, []);

    return (
        <PanelSection title="TonTonDeck Games">
            <PanelSectionRow>
                <ButtonItem layout="below" onClick={fetchGames} disabled={loading}>
                    <FaSync style={{ marginRight: 8 }} />
                    {loading ? "Loading..." : "Refresh Games"}
                </ButtonItem>
            </PanelSectionRow>

            {error && (
                <PanelSectionRow>
                    <Field label="Error">{error}</Field>
                </PanelSectionRow>
            )}

            {syncStatus && (
                <PanelSectionRow>
                    <Field label="Status">{syncStatus}</Field>
                </PanelSectionRow>
            )}

            {games.length === 0 && !loading && !error && (
                <PanelSectionRow>
                    <Field label="No games">Configure PC connection and refresh</Field>
                </PanelSectionRow>
            )}

            {games.map((game) => (
                <PanelSectionRow key={game.app_id}>
                    <Focusable style={{ display: "flex", alignItems: "center", width: "100%" }}>
                        <div style={{ flex: 1 }}>
                            <div style={{ fontWeight: "bold" }}>{game.name}</div>
                            <div style={{ fontSize: "0.8em", opacity: 0.7 }}>
                                ID: {game.app_id} • {formatBytes(game.size_bytes)}
                            </div>
                        </div>
                        <DialogButton
                            onClick={() => syncGame(game)}
                            disabled={syncing !== null}
                            style={{ minWidth: 70 }}
                        >
                            {syncing === game.name ? "..." : "Sync"}
                        </DialogButton>
                    </Focusable>
                </PanelSectionRow>
            ))}
        </PanelSection>
    );
};

// ============================================================================
// Main Plugin Content
// ============================================================================
const Content: VFC<{ serverAPI: ServerAPI }> = ({ serverAPI }) => {
    const [activeTab, setActiveTab] = useState<"games" | "settings">("games");

    return (
        <div>
            <PanelSection>
                <PanelSectionRow>
                    <Focusable style={{ display: "flex", gap: 8 }}>
                        <DialogButton
                            onClick={() => setActiveTab("games")}
                            style={{
                                flex: 1,
                                backgroundColor:
                                    activeTab === "games" ? "#1a9fff" : undefined,
                            }}
                        >
                            <FaGamepad style={{ marginRight: 4 }} /> Games
                        </DialogButton>
                        <DialogButton
                            onClick={() => setActiveTab("settings")}
                            style={{
                                flex: 1,
                                backgroundColor:
                                    activeTab === "settings" ? "#1a9fff" : undefined,
                            }}
                        >
                            <FaCog style={{ marginRight: 4 }} /> Settings
                        </DialogButton>
                    </Focusable>
                </PanelSectionRow>
            </PanelSection>

            {activeTab === "games" ? (
                <GameListPanel serverAPI={serverAPI} />
            ) : (
                <SettingsPanel serverAPI={serverAPI} />
            )}
        </div>
    );
};

// ============================================================================
// Plugin Definition
// ============================================================================
export default definePlugin((serverApi: ServerAPI) => {
    return {
        title: <div className={staticClasses.Title}>TonTon</div>,
        content: <Content serverAPI={serverApi} />,
        icon: <FaSync />,
        onDismount() {
            // Cleanup if needed
        },
    };
});
