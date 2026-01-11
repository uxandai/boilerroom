import { Component, ErrorInfo, ReactNode } from "react";
import { AlertCircle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

interface Props {
    children?: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false,
        error: null,
    };

    public static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        console.error("Uncaught error:", error, errorInfo);
    }

    public render() {
        if (this.state.hasError) {
            return (
                <div className="min-h-screen bg-background flex items-center justify-center p-4">
                    <Card className="bg-[#1b2838] border-[#2a475e] max-w-md w-full">
                        <CardHeader>
                            <CardTitle className="text-red-400 flex items-center gap-2">
                                <AlertCircle className="w-5 h-5" />
                                Something went wrong
                            </CardTitle>
                            <CardDescription>
                                An unexpected error occurred in the application.
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="bg-[#0a0a0a] p-3 rounded border border-[#2a475e] text-xs font-mono text-gray-300 max-h-48 overflow-y-auto">
                                {this.state.error?.message || "Unknown error"}
                            </div>
                            <Button
                                onClick={() => window.location.reload()}
                                className="btn-steam w-full"
                            >
                                <RefreshCw className="w-4 h-4 mr-2" />
                                Reload Application
                            </Button>
                        </CardContent>
                    </Card>
                </div>
            );
        }

        return this.props.children;
    }
}
