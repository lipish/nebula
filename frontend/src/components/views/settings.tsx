import { ShieldCheck, KeyRound } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"

interface SettingsProps {
    token: string
    setToken: (v: string) => void
    onSaveToken: () => void
}

export function SettingsView({ token: _token, setToken: _setToken, onSaveToken }: SettingsProps) {
    return (
        <div className="max-w-2xl space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div>
                <h2 className="text-2xl font-bold text-foreground">Settings</h2>
                <p className="text-sm text-muted-foreground mt-1">Manage your platform preferences and security</p>
            </div>

            <div className="bg-card border border-border rounded-2xl p-6 shadow-sm">
                <div className="flex items-center justify-between mb-8">
                    <div className="flex items-center gap-3">
                        <div className="h-10 w-10 rounded-xl bg-primary/10 flex items-center justify-center text-primary">
                            <ShieldCheck className="h-5 w-5" />
                        </div>
                        <div>
                            <h3 className="text-lg font-bold text-foreground tracking-tight">Authentication</h3>
                            <p className="text-xs font-medium text-muted-foreground">Managed by backend PostgreSQL auth service</p>
                        </div>
                    </div>
                    <Badge variant="outline" className="border-border text-muted-foreground font-bold text-[9px] uppercase tracking-widest">
                        Server Managed
                    </Badge>
                </div>

                <div className="space-y-6">
                    <div className="rounded-xl border border-border px-4 py-3 bg-accent/20">
                        <div className="flex items-center gap-2 mb-1">
                            <KeyRound className="h-4 w-4 text-primary" />
                            <p className="text-sm font-semibold text-foreground">Session Authentication</p>
                        </div>
                        <p className="text-xs text-muted-foreground">Current session token is issued by backend login API and validated against PostgreSQL sessions.</p>
                    </div>

                    <Button
                        onClick={onSaveToken}
                        className="w-full bg-primary font-bold rounded-xl h-11 shadow-sm hover:shadow-md active:scale-[0.99] transition-all"
                    >
                        Refresh Protected Data
                    </Button>
                </div>
            </div>

            <div className="bg-muted/30 border border-border/50 rounded-2xl p-6 border-dashed">
                <h4 className="text-sm font-bold text-muted-foreground uppercase tracking-widest mb-2 text-center">System Information</h4>
                <div className="grid grid-cols-2 gap-4">
                    <div className="text-center p-3 bg-background/50 rounded-xl border border-border/30 shadow-sm">
                        <p className="text-[10px] font-bold text-muted-foreground uppercase mb-1">UI Version</p>
                        <p className="text-sm font-bold font-mono">0.1.0-prod</p>
                    </div>
                    <div className="text-center p-3 bg-background/50 rounded-xl border border-border/30 shadow-sm">
                        <p className="text-[10px] font-bold text-muted-foreground uppercase mb-1">API Tier</p>
                        <p className="text-sm font-bold font-mono text-primary">Core (BETA)</p>
                    </div>
                </div>
            </div>
        </div>
    )
}
