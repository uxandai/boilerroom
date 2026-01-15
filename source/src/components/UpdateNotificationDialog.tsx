import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { ArrowUpCircle, Terminal } from "lucide-react";

interface UpdateNotificationDialogProps {
    open: boolean;
    onClose: () => void;
    currentVersion: string;
    latestVersion: string;
    releaseUrl: string;
}

export function UpdateNotificationDialog({
    open,
    onClose,
    currentVersion,
    latestVersion,
    releaseUrl,
}: UpdateNotificationDialogProps) {
    return (
        <AlertDialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
            <AlertDialogContent className="bg-[#1b2838] border-[#2a475e] max-w-md">
                <AlertDialogHeader>
                    <AlertDialogTitle className="flex items-center gap-2 text-[#67c1f5]">
                        <ArrowUpCircle className="w-5 h-5" />
                        Update Available
                    </AlertDialogTitle>
                    <AlertDialogDescription asChild>
                        <div className="space-y-4 text-gray-300">
                            <p>
                                A new version of BoilerRoom is available!
                            </p>

                            <div className="bg-[#0d1117] rounded-lg p-4 space-y-2">
                                <div className="flex justify-between">
                                    <span className="text-gray-400">Current version:</span>
                                    <span className="font-mono text-gray-400">v{currentVersion}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-gray-400">Latest version:</span>
                                    <span className="font-mono text-[#67c1f5]">v{latestVersion}</span>
                                </div>
                            </div>

                            <div className="bg-[#0d1117] rounded-lg p-4">
                                <p className="text-sm text-gray-400 mb-2">
                                    To update, run the installer again:
                                </p>
                                <div className="flex items-center gap-2 bg-[#171a21] rounded p-2">
                                    <Terminal className="w-4 h-4 text-[#67c1f5] shrink-0" />
                                    <code className="text-xs text-[#67c1f5] break-all">
                                        curl -fsSL https://raw.githubusercontent.com/uxandai/boilerroom/main/install.sh | bash
                                    </code>
                                </div>
                            </div>

                            <a
                                href={releaseUrl}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="block text-center text-sm text-[#67c1f5] hover:underline"
                            >
                                View release notes →
                            </a>
                        </div>
                    </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                    <AlertDialogAction
                        onClick={onClose}
                        className="bg-[#2a475e] hover:bg-[#3d5a6c] text-white"
                    >
                        Dismiss
                    </AlertDialogAction>
                </AlertDialogFooter>
            </AlertDialogContent>
        </AlertDialog>
    );
}
