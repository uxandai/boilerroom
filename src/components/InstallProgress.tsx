import { useAppStore } from "@/store/useAppStore";
import { cancelInstallation, pauseInstallation, resumeInstallation } from "@/lib/api";
import { X, Loader2, CheckCircle, AlertCircle, PauseCircle } from "lucide-react";

export function InstallProgress() {
  const { installProgress, setInstallProgress, addLog } = useAppStore();

  if (!installProgress || installProgress.step === "idle") {
    return null;
  }

  // Format bytes
  const formatBytes = (bytes: number): string => {
    if (!bytes || bytes < 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    let size = bytes;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  // Map state to display info
  const getPhaseInfo = () => {
    switch (installProgress.step) {
      case "downloading":
        return { icon: <Loader2 className="w-5 h-5 animate-spin text-[#67c1f5]" />, label: "Downloading", color: "text-[#67c1f5]", bgColor: "bg-[#67c1f5]" };
      case "steamless":
        return { icon: <Loader2 className="w-5 h-5 animate-spin text-yellow-400" />, label: "Patching DRM", color: "text-yellow-400", bgColor: "bg-yellow-400" };
      case "transferring":
        return { icon: <Loader2 className="w-5 h-5 animate-spin text-purple-400" />, label: "Transferring", color: "text-purple-400", bgColor: "bg-purple-400" };
      case "configuring":
        return { icon: <Loader2 className="w-5 h-5 animate-spin text-green-400" />, label: "Configuring", color: "text-green-400", bgColor: "bg-green-400" };
      case "finished":
        return { icon: <CheckCircle className="w-5 h-5 text-green-500" />, label: "Complete", color: "text-green-500", bgColor: "bg-green-500" };
      case "error":
        return { icon: <AlertCircle className="w-5 h-5 text-red-500" />, label: "Error", color: "text-red-500", bgColor: "bg-red-500" };
      case "paused":
        return { icon: <PauseCircle className="w-5 h-5 text-yellow-500" />, label: "Paused", color: "text-yellow-500", bgColor: "bg-yellow-500" };
      case "cancelled":
        return { icon: <X className="w-5 h-5 text-orange-500" />, label: "Cancelled", color: "text-orange-500", bgColor: "bg-orange-500" };
      default:
        return { icon: <Loader2 className="w-5 h-5 animate-spin" />, label: "Working", color: "text-gray-400", bgColor: "bg-gray-400" };
    }
  };

  const phaseInfo = getPhaseInfo();

  // Backend now sends downloadPercent in 0-100% range (0-50% download, 50-100% transfer)
  const downloadProgress = installProgress.downloadPercent || 0;

  // Show overall progress - use directly from backend
  let overallProgress = 0;
  if (installProgress.step === "downloading" || installProgress.step === "transferring") {
    overallProgress = downloadProgress;
  } else if (installProgress.step === "steamless") {
    overallProgress = 50;
  } else if (installProgress.step === "configuring") {
    overallProgress = 95;
  } else if (installProgress.step === "finished") {
    overallProgress = 100;
  } else if (installProgress.step === "paused") {
    overallProgress = downloadProgress;
  }

  const handleCancel = async () => {
    addLog("warn", "Cancelling installation...");
    try {
      await cancelInstallation();
      addLog("info", "Installation cancelled");
    } catch (error) {
      addLog("error", `Cancel failed: ${error}`);
    }
    setInstallProgress(null);
  };
  
  const handlePause = async () => {
    addLog("info", "Pausing installation...");
    try {
      await pauseInstallation();
      addLog("info", "Installation paused");
    } catch (error) {
      addLog("error", `Pause failed: ${error}`);
    }
  };
  
  const handleResume = async () => {
    addLog("info", "Resuming installation...");
    try {
      await resumeInstallation();
      addLog("info", "Installation resumed");
    } catch (error) {
      addLog("error", `Resume failed: ${error}`);
    }
  };

  const isDone = installProgress.step === "finished";
  const isError = installProgress.step === "error";
  const isCancelled = installProgress.step === "cancelled";

  return (
    <div className="relative overflow-hidden bg-[#1b2838] border-b border-[#0a0a0a]">
      {/* Background Hero Image */}
      <div 
        className="absolute inset-0 bg-cover bg-center opacity-30"
        style={{ 
          backgroundImage: installProgress.heroImage 
            ? `url(${installProgress.heroImage})` 
            : 'linear-gradient(135deg, #1b2838 0%, #2a475e 100%)'
        }}
      />
      <div className="absolute inset-0 bg-gradient-to-r from-[#1b2838] via-[#1b2838]/80 to-transparent" />
      
      {/* Content */}
      <div className="relative flex items-stretch">
        {/* Left: Game Info */}
        <div className="w-72 p-4 flex items-center justify-center">
          {installProgress.heroImage ? (
            <img 
              src={installProgress.heroImage} 
              alt={installProgress.gameName}
              className="max-h-20 object-contain"
            />
          ) : (
            <h2 className="text-xl font-bold text-white text-center">
              {installProgress.gameName}
            </h2>
          )}
        </div>

        {/* Right: Progress */}
        <div className="flex-1 p-4 flex flex-col justify-center gap-2">
          {/* Phase indicator */}
          <div className="flex items-center gap-3">
            {phaseInfo.icon}
            <span className="text-white font-bold text-lg">{phaseInfo.label}</span>
            {installProgress.transferSpeed && (
              <span className="text-[#67c1f5] font-mono text-sm ml-auto">
                {installProgress.transferSpeed}
              </span>
            )}
          </div>

          {/* Main progress bar */}
          <div className="flex items-center gap-3">
            <div className="flex-1 h-3 bg-[#0a0a0a] rounded-full overflow-hidden">
              <div 
                className={`h-full ${phaseInfo.bgColor} transition-all duration-300`}
                style={{ width: `${overallProgress}%` }}
              />
            </div>
            <span className="text-white text-sm w-16 text-right font-mono">
              {Math.round(overallProgress)}%
            </span>
          </div>

          {/* Phase-specific details */}
          <div className="text-sm text-gray-400 flex items-center gap-4">
            {installProgress.step === "downloading" && (
              <>
                <span>Downloading: {downloadProgress.toFixed(1)}%</span>
                {installProgress.downloadSpeed && (
                  <span className="text-[#67c1f5] font-mono">{installProgress.downloadSpeed}</span>
                )}
                {installProgress.eta && (
                  <span className="text-gray-500">ETA: {installProgress.eta}</span>
                )}
              </>
            )}
            {installProgress.step === "transferring" && (
              <span>
                Files: {installProgress.filesTransferred}/{installProgress.filesTotal}
                {installProgress.bytesTransferred > 0 && ` (${formatBytes(installProgress.bytesTransferred)})`}
              </span>
            )}
            {installProgress.message && (
              <span className="ml-3 truncate">{installProgress.message}</span>
            )}
          </div>

          {/* Error */}
          {isError && installProgress.error && (
            <div className="text-red-400 text-sm bg-red-900/20 px-3 py-1 rounded">
              {installProgress.error}
            </div>
          )}
        </div>

        {/* Controls */}
        <div className="flex items-center gap-2 px-4">
          {/* Pause button - only during downloading/transferring */}
          {(installProgress.step === "downloading" || installProgress.step === "transferring") && (
            <button
              onClick={handlePause}
              className="w-10 h-10 rounded bg-yellow-600 hover:bg-yellow-500 flex items-center justify-center"
              title="Pause"
            >
              <svg className="w-5 h-5 text-white" fill="currentColor" viewBox="0 0 24 24">
                <rect x="6" y="4" width="4" height="16" />
                <rect x="14" y="4" width="4" height="16" />
              </svg>
            </button>
          )}
          
          {/* Resume button - only when paused */}
          {installProgress.step === "paused" && (
            <button
              onClick={handleResume}
              className="w-10 h-10 rounded bg-green-600 hover:bg-green-500 flex items-center justify-center"
              title="Resume"
            >
              <svg className="w-5 h-5 text-white" fill="currentColor" viewBox="0 0 24 24">
                <polygon points="5,3 19,12 5,21" />
              </svg>
            </button>
          )}
          
          {/* Cancel button */}
          {!isDone && !isError && !isCancelled && (
            <button
              onClick={handleCancel}
              className="w-10 h-10 rounded bg-red-600 hover:bg-red-500 flex items-center justify-center"
              title="Cancel"
            >
              <X className="w-5 h-5 text-white" />
            </button>
          )}
          
          {/* Close button - for done/error/cancelled states */}
          {(isDone || isError || isCancelled) && (
            <button
              onClick={() => setInstallProgress(null)}
              className="px-4 py-2 rounded bg-[#67c1f5] hover:bg-[#7dd0ff] text-[#1b2838] font-bold"
            >
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
