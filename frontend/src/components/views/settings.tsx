import { ShieldCheck, KeyRound } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"

interface SettingsProps {
    token: string
    setToken: (v: string) => void
    onSaveToken: () => void
}

export function SettingsView({ token, setToken, onSaveToken }: SettingsProps) {
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
                            <p className="text-xs font-medium text-muted-foreground">Manage your access credentials</p>
                        </div>
                    </div>
                    <Badge variant="outline" className="border-border text-muted-foreground font-bold text-[9px] uppercase tracking-widest">
                        Local Storage
                    </Badge>
                </div>

                <div className="space-y-6">
                    <div className="space-y-2">
                        <div className="flex items-center justify-between">
                            <Label htmlFor="token" className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
                                BFF API Bearer Token
                            </Label>
                            <div className="flex h-2 w-2 rounded-full bg-success" />
                        </div>
                        <div className="relative group">
                            <div className="absolute left-3.5 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors group-focus-within:text-primary">
                                <KeyRound className="h-4 w-4" />
                            </div>
                            <Input
                                id="token"
                                type="password"
                                placeholder="Enter your security token..."
                                className="pl-10 h-12 rounded-xl border-border bg-accent/20 focus:bg-background transition-all font-mono"
                                value={token}
                                onChange={(e) => setToken(e.target.value)}
                            />
                        </div>
                        <p className="text-[10px] font-medium text-muted-foreground/70 bg-accent/30 px-3 py-2 rounded-lg">
                            <span className="font-bold text-primary">Security Note:</span> This token is stored locally in your browser's encrypted storage and sent with every API request to authorize communication with the Nebula Backend.
                        </p>
                    </div>

                    <Button
                        onClick={onSaveToken}
                        className="w-full bg-primary font-bold rounded-xl h-11 shadow-sm hover:shadow-md active:scale-[0.99] transition-all"
                    >
                        Apply Security Changes
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
