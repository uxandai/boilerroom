/**
 * Setup Wizard Modal - first-launch setup guide
 * Shows users how to install SLSsteam via headcrab.sh
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
import { useAppStore } from "@/store/useAppStore";
import { useState } from "react";
import {
    Rocket,
    Copy,
    Check,
    ExternalLink,
    Terminal,
} from "lucide-react";

const HEADCRAB_COMMAND = 'curl -fsSL https://raw.githubusercontent.com/Deadboy666/h3adcr-b/main/headcrab.sh | bash';

export function SetupWizard() {
    const { setupWizardOpen, setSetupWizardOpen } = useAppStore();
    const [copied, setCopied] = useState(false);

    const handleCopy = async () => {
        try {
            await navigator.clipboard.writeText(HEADCRAB_COMMAND);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch {
            // Fallback for older browsers
            const textArea = document.createElement('textarea');
            textArea.value = HEADCRAB_COMMAND;
            document.body.appendChild(textArea);
            textArea.select();
            document.execCommand('copy');
            document.body.removeChild(textArea);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    const closeWizard = () => {
        setSetupWizardOpen(false);
        // Mark as seen
        localStorage.setItem('boilerroom_setup_seen', 'true');
    };

    return (
        <Dialog open={setupWizardOpen} onOpenChange={(open) => !open && closeWizard()}>
            <DialogContent className="bg-[#1b2838] border-[#2a475e] max-w-lg">
                <DialogHeader>
                    <DialogTitle className="text-white flex items-center gap-2">
                        <Rocket className="w-6 h-6 text-[#67c1f5]" />
                        Welcome to BoilerRoom!
                    </DialogTitle>
                    <DialogDescription>
                        To use BoilerRoom's full features, you need SLSsteam installed.
                    </DialogDescription>
                </DialogHeader>

                <div className="py-4 space-y-4">
                    {/* Info */}
                    <div className="bg-[#171a21] border border-[#2a475e] p-4 rounded">
                        <div className="flex items-center gap-2 text-[#67c1f5] mb-2">
                            <Terminal className="w-5 h-5" />
                            <span className="font-medium">Install SLSsteam</span>
                        </div>
                        <p className="text-gray-400 text-sm mb-3">
                            Run this command in your terminal (Konsole on Steam Deck):
                        </p>

                        {/* Command box */}
                        <div className="bg-[#0a0a0a] border border-[#2a475e] rounded p-3 font-mono text-xs text-green-400 break-all">
                            {HEADCRAB_COMMAND}
                        </div>

                        {/* Copy button */}
                        <Button
                            onClick={handleCopy}
                            className="w-full mt-3 btn-steam"
                        >
                            {copied ? (
                                <><Check className="w-4 h-4 mr-2" /> Copied!</>
                            ) : (
                                <><Copy className="w-4 h-4 mr-2" /> Copy Command</>
                            )}
                        </Button>
                    </div>

                    {/* GitHub link */}
                    <a
                        href="https://github.com/Deadboy666/h3adcr-b"
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex items-center gap-2 text-sm text-[#67c1f5] hover:underline"
                    >
                        <ExternalLink className="w-4 h-4" />
                        View headcrab on GitHub
                    </a>

                    {/* Note */}
                    <p className="text-xs text-gray-500">
                        After running headcrab, restart Steam for changes to take effect.
                    </p>
                </div>

                <DialogFooter>
                    <Button
                        variant="outline"
                        onClick={closeWizard}
                        className="border-[#2a475e]"
                    >
                        I'll do this later
                    </Button>
                    <Button onClick={closeWizard} className="btn-steam">
                        Done
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}

// Button to relaunch wizard from settings
export function RelaunchSetupButton() {
    const { setSetupWizardOpen } = useAppStore();

    return (
        <Button
            onClick={() => setSetupWizardOpen(true)}
            variant="outline"
            className="w-full mt-2 border-[#2a475e] text-white hover:bg-[#2a475e]/50"
        >
            <Rocket className="w-4 h-4 mr-2" />
            Show Setup Guide
        </Button>
    );
}
