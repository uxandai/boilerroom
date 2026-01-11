/**
 * Setup Wizard Modal - first-launch setup flow
 * Inspired by enter-the-wired installer patterns
 */
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useSetupWizard } from "@/hooks/useSetupWizard";
import { useAppStore } from "@/store/useAppStore";
import {
    CheckCircle2,
    XCircle,
    Loader2,
    Circle,
    RefreshCw,
    Rocket,
    Settings,
} from "lucide-react";
import type { StepStatus } from "@/lib/api/setup";

// Step status icon component
function StepIcon({ status }: { status: StepStatus }) {
    switch (status) {
        case "done":
            return <CheckCircle2 className="w-5 h-5 text-green-500" />;
        case "running":
            return <Loader2 className="w-5 h-5 text-blue-500 animate-spin" />;
        case "error":
            return <XCircle className="w-5 h-5 text-red-500" />;
        case "skipped":
            return <Circle className="w-5 h-5 text-gray-500" />;
        default:
            return <Circle className="w-5 h-5 text-gray-600" />;
    }
}

export function SetupWizard() {
    const {
        isOpen,
        isRunning,
        setupState,
        result,
        closeWizard,
        startSetup,
        restartSetup,
    } = useSetupWizard();

    const { connectionMode } = useAppStore();

    const isComplete = result?.success === true;
    const hasFailed = result?.success === false;

    return (
        <Dialog open={isOpen} onOpenChange={(open) => !isRunning && !open && closeWizard()}>
            <DialogContent className="bg-[#1b2838] border-[#2a475e] max-w-lg">
                <DialogHeader>
                    <DialogTitle className="text-white flex items-center gap-2">
                        <Rocket className="w-6 h-6 text-[#67c1f5]" />
                        TonTonDeck Setup
                    </DialogTitle>
                    <DialogDescription>
                        {!setupState && !result && (
                            <>
                                Welcome! This wizard will configure SLSsteam and required dependencies
                                for {connectionMode === "local" ? "your local system" : "your Steam Deck"}.
                            </>
                        )}
                        {isRunning && "Setting up your system..."}
                        {isComplete && "Setup completed successfully!"}
                        {hasFailed && `Setup failed: ${result?.error}`}
                    </DialogDescription>
                </DialogHeader>

                <div className="py-4 space-y-4">
                    {/* Mode indicator */}
                    <div className="bg-[#2a475e]/50 p-3 rounded flex items-center gap-2">
                        <Settings className="w-4 h-4 text-[#67c1f5]" />
                        <span className="text-sm text-gray-300">
                            Mode: <strong className="text-white">{connectionMode === "local" ? "Local" : "Remote (Steam Deck)"}</strong>
                        </span>
                    </div>

                    {/* Steps list */}
                    {setupState && setupState.steps.length > 0 && (
                        <div className="space-y-2">
                            {setupState.steps.map((step) => (
                                <div
                                    key={step.id}
                                    className={`flex items-start gap-3 p-2 rounded ${step.status === "running"
                                        ? "bg-blue-900/20 border border-blue-600/30"
                                        : step.status === "error"
                                            ? "bg-red-900/20 border border-red-600/30"
                                            : step.status === "done"
                                                ? "bg-green-900/10"
                                                : "bg-[#171a21]"
                                        }`}
                                >
                                    <StepIcon status={step.status} />
                                    <div className="flex-1 min-w-0">
                                        <div className="text-sm font-medium text-white">{step.name}</div>
                                        {step.message && (
                                            <div className={`text-xs mt-0.5 ${step.status === "error" ? "text-red-400" : "text-gray-400"
                                                }`}>
                                                {step.message}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}

                    {/* Pre-start message */}
                    {!setupState && !result && (
                        <div className="text-center py-6">
                            <Rocket className="w-12 h-12 text-[#67c1f5] mx-auto mb-4 opacity-50" />
                            <p className="text-gray-400 text-sm">
                                Click "Start Setup" to begin the automatic configuration process.
                            </p>
                        </div>
                    )}

                    {/* Success message */}
                    {isComplete && (
                        <div className="bg-green-900/20 border border-green-600/30 p-4 rounded">
                            <div className="flex items-center gap-2 text-green-400">
                                <CheckCircle2 className="w-5 h-5" />
                                <span className="font-medium">All done!</span>
                            </div>
                            <p className="text-sm text-gray-300 mt-2">
                                SLSsteam is installed and configured. Restart Steam to apply changes.
                            </p>
                        </div>
                    )}
                </div>

                <DialogFooter>
                    {/* Before setup starts */}
                    {!setupState && !result && (
                        <>
                            <Button
                                variant="outline"
                                onClick={closeWizard}
                                className="border-[#2a475e]"
                            >
                                Skip for now
                            </Button>
                            <Button
                                onClick={startSetup}
                                className="btn-steam"
                            >
                                <Rocket className="w-4 h-4 mr-2" />
                                Start Setup
                            </Button>
                        </>
                    )}

                    {/* During setup */}
                    {isRunning && (
                        <Button disabled className="btn-steam">
                            <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                            Setting up...
                        </Button>
                    )}

                    {/* After success */}
                    {isComplete && (
                        <Button onClick={closeWizard} className="btn-steam">
                            Done
                        </Button>
                    )}

                    {/* After failure */}
                    {hasFailed && (
                        <>
                            <Button
                                variant="outline"
                                onClick={closeWizard}
                                className="border-[#2a475e]"
                            >
                                Close
                            </Button>
                            <Button onClick={restartSetup} className="btn-steam">
                                <RefreshCw className="w-4 h-4 mr-2" />
                                Retry
                            </Button>
                        </>
                    )}
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}

// Button to relaunch wizard from settings
export function RelaunchSetupButton() {
    const { openWizard } = useSetupWizard();

    return (
        <Button
            onClick={openWizard}
            variant="outline"
            className="w-full mt-2 border-[#2a475e] text-white hover:bg-[#2a475e]/50"
        >
            <Rocket className="w-4 h-4 mr-2" />
            Relaunch Setup Wizard
        </Button>
    );
}
