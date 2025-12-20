import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Trash2, Download, Info, AlertTriangle, AlertCircle } from "lucide-react";

export function LogsPanel() {
  const { logs, clearLogs, addLog } = useAppStore();

  // Export logs to file
  const handleExport = () => {
    const content = logs
      .map(
        (log) =>
          `[${log.timestamp.toISOString()}] [${log.level.toUpperCase()}] ${log.message}`
      )
      .join("\n");

    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `tontondeck-logs-${new Date().toISOString().split("T")[0]}.txt`;
    a.click();
    URL.revokeObjectURL(url);

    addLog("info", "Logs exported to file");
  };

  // Get icon for log level
  const LogIcon = ({ level }: { level: string }) => {
    switch (level) {
      case "error":
        return <AlertCircle className="w-4 h-4 text-destructive flex-shrink-0" />;
      case "warn":
        return <AlertTriangle className="w-4 h-4 text-yellow-500 flex-shrink-0" />;
      default:
        return <Info className="w-4 h-4 text-blue-500 flex-shrink-0" />;
    }
  };

  // Format time
  const formatTime = (date: Date): string => {
    return date.toLocaleTimeString("en-US", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  return (
    <div className="space-y-6">
      <Card className="bg-[#1b2838] border-[#2a475e]">
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-white">Logs</CardTitle>
              <CardDescription>Application activity history</CardDescription>
            </div>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={handleExport} disabled={logs.length === 0} className="border-[#2a475e]">
                <Download className="w-4 h-4 mr-1" />
                Export
              </Button>
              <Button variant="outline" size="sm" onClick={clearLogs} disabled={logs.length === 0} className="border-[#2a475e]">
                <Trash2 className="w-4 h-4 mr-1" />
                Clear
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-[400px]">
            <div className="space-y-1 font-mono text-sm">
              {logs.length === 0 ? (
                <p className="text-muted-foreground text-center py-8">
                  No logs. Activity will appear here.
                </p>
              ) : (
                logs.map((log, index) => (
                  <div
                    key={index}
                    className={`flex items-start gap-2 py-1 ${
                      log.level === "error"
                        ? "text-destructive"
                        : log.level === "warn"
                        ? "text-yellow-500"
                        : "text-foreground"
                    }`}
                  >
                    <LogIcon level={log.level} />
                    <span className="text-muted-foreground">{formatTime(log.timestamp)}</span>
                    <span className="flex-1 break-all">{log.message}</span>
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}
