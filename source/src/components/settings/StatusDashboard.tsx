/**
 * Status Dashboard - quick overview of configuration status at top of settings
 */
import { useEffect, useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import {
    CheckCircle2,
    XCircle,
    AlertTriangle,
    Loader2,
    Key,
    Shield,
    Wrench,
    Wifi,
} from "lucide-react";

interface StatusItem {
    label: string;
    status: "ok" | "warning" | "error" | "loading" | "unknown";
    detail?: string;
}

function StatusIcon({ status }: { status: StatusItem["status"] }) {
    switch (status) {
        case "ok":
            return <CheckCircle2 className="w-4 h-4 text-green-400" />;
        case "warning":
            return <AlertTriangle className="w-4 h-4 text-yellow-400" />;
        case "error":
            return <XCircle className="w-4 h-4 text-red-400" />;
        case "loading":
            return <Loader2 className="w-4 h-4 text-gray-400 animate-spin" />;
        default:
            return <div className="w-4 h-4 rounded-full bg-gray-600" />;
    }
}

export function StatusDashboard() {
    const { settings, connectionMode, connectionStatus, sshConfig } = useAppStore();
    const [slsStatus, setSlsStatus] = useState<"ok" | "error" | "loading" | "unknown">("unknown");

    // Check SLSsteam status on mount
    useEffect(() => {
        const checkSls = async () => {
            setSlsStatus("loading");
            try {
                if (connectionMode === "local") {
                    const { verifySlssteamLocal } = await import("@/lib/api");
                    const status = await verifySlssteamLocal();
                    setSlsStatus(status.slssteam_so_exists && status.config_exists ? "ok" : "error");
                } else if (sshConfig.ip && sshConfig.password) {
                    const { verifySlssteam } = await import("@/lib/api");
                    const status = await verifySlssteam(sshConfig);
                    setSlsStatus(status.slssteam_so_exists && status.config_exists ? "ok" : "error");
                } else {
                    setSlsStatus("unknown");
                }
            } catch {
                setSlsStatus("error");
            }
        };

        checkSls();
    }, [connectionMode, sshConfig]);

    // Calculate status items
    const items: { icon: React.ReactNode; items: StatusItem[] }[] = [
        {
            icon: <Key className="w-4 h-4 text-[#67c1f5]" />,
            items: [
                {
                    label: "Depot Provider",
                    status: settings.apiKey || settings.useApiKeyUrl ? "ok" : "error",
                },
                {
                    label: "SteamGridDB",
                    status: settings.steamGridDbApiKey ? "ok" : "warning",
                    detail: !settings.steamGridDbApiKey ? "Optional" : undefined,
                },
            ],
        },
        {
            icon: <Shield className="w-4 h-4 text-[#67c1f5]" />,
            items: [
                {
                    label: "SLSsteam",
                    status: slsStatus,
                },
            ],
        },
        {
            icon: connectionMode === "local" ? <Wrench className="w-4 h-4 text-[#67c1f5]" /> : <Wifi className="w-4 h-4 text-[#67c1f5]" />,
            items: connectionMode === "local"
                ? [
                    {
                        label: "Mode",
                        status: "ok" as const,
                        detail: "Local",
                    },
                ]
                : [
                    {
                        label: "SSH",
                        status: connectionStatus === "ssh_ok" ? "ok" : connectionStatus === "online" ? "warning" : "error",
                        detail: connectionStatus === "ssh_ok" ? "Connected" : connectionStatus === "online" ? "Online" : "Offline",
                    },
                ],
        },
    ];

    return (
        <div className="bg-[#171a21] border border-[#2a475e] p-4 mb-4">
            <div className="flex items-center gap-6 justify-center flex-wrap">
                {items.map((group, i) => (
                    <div key={i} className="flex items-center gap-4">
                        {group.icon}
                        {group.items.map((item, j) => (
                            <div key={j} className="flex items-center gap-1.5">
                                <StatusIcon status={item.status} />
                                <span className="text-sm text-gray-300">{item.label}</span>
                                {item.detail && (
                                    <span className="text-xs text-gray-500">({item.detail})</span>
                                )}
                            </div>
                        ))}
                        {i < items.length - 1 && (
                            <div className="w-px h-6 bg-[#2a475e] mx-2" />
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}
