import { Plus, Trash2, Box } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import type { EndpointInfo, ModelRequest } from "@/lib/types"
import { cn } from "@/lib/utils"

interface ModelsProps {
    requests: ModelRequest[]
    endpoints: EndpointInfo[]
    onOpenLoadDialog: () => void
    handleUnload: (id: string) => Promise<void>
    fmtTime: (v: number) => string
    onNavigate?: (page: string) => void
}

export function ModelsView({
    requests,
    endpoints,
    onOpenLoadDialog,
    handleUnload,
    fmtTime,
    onNavigate,
}: ModelsProps) {
    const endpointByModel = new Map<string, EndpointInfo[]>()
    for (const ep of endpoints) {
        if (!endpointByModel.has(ep.model_uid)) endpointByModel.set(ep.model_uid, [])
        endpointByModel.get(ep.model_uid)!.push(ep)
    }

    const isEndpointReady = (modelUid: string): boolean => {
        const eps = endpointByModel.get(modelUid) ?? []
        return eps.some((e) => {
            const s = (e.status ?? "").toLowerCase()
            return s.includes("ready") || s.includes("running") || s.includes("healthy")
        })
    }

    const extractStatus = (status: unknown): { kind: string; reason?: string } => {
        if (typeof status === "string") return { kind: status }
        if (status && typeof status === "object") {
            const s = status as Record<string, unknown>
            if (typeof s.Failed === "string") return { kind: "Failed", reason: s.Failed }
            if (typeof s.Running === "string") return { kind: "Running", reason: s.Running }
            if (typeof s.Scheduled === "string") return { kind: "Scheduled", reason: s.Scheduled }
            if (typeof s.Pending === "string") return { kind: "Pending", reason: s.Pending }
        }
        return { kind: String(status ?? "") }
    }

    const displayStatus = (
        req: ModelRequest,
    ): { label: string; variant: "failed" | "ready" | "loading"; pulse: boolean; reason?: string } => {
        const st = extractStatus(req.status)
        const s = (st.kind ?? "").toLowerCase()
        if (s.includes("fail")) return { label: "FAILED", variant: "failed", pulse: false, reason: st.reason }
        if (s.includes("running") || s.includes("ready")) return { label: "READY", variant: "ready", pulse: false }
        if (s.includes("scheduled")) {
            return isEndpointReady(req.request.model_uid)
                ? { label: "READY", variant: "ready", pulse: false }
                : { label: "LOADING...", variant: "loading", pulse: true }
        }
        if (s.includes("pending")) return { label: "QUEUED...", variant: "loading", pulse: true }
        if (s.includes("unload")) return { label: "UNLOADING...", variant: "loading", pulse: true }
        return { label: st.kind.toUpperCase(), variant: "loading", pulse: true }
    }

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-2xl font-bold text-foreground">Model Management</h2>
                    <p className="text-sm text-muted-foreground mt-1">Load, monitor, and manage model deployments</p>
                </div>
                <Button
                    onClick={onOpenLoadDialog}
                    className="bg-primary hover:bg-primary/90 rounded-xl shadow-sm px-5"
                >
                    <Plus className="mr-2 h-4 w-4" />
                    Load Model
                </Button>
            </div>

            <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
                <div className="px-6 py-5 border-b border-border bg-accent/30 flex items-center justify-between">
                    <div>
                        <h3 className="text-lg font-bold text-foreground tracking-tight">Deployment Requests</h3>
                        <p className="text-xs font-medium text-muted-foreground">Historical and active provisioning records</p>
                    </div>
                    <Badge variant="outline" className="font-bold border-primary/20 text-primary uppercase h-6">
                        {requests.length} Total
                    </Badge>
                </div>

                <Table>
                    <TableHeader>
                        <TableRow className="bg-muted hover:bg-muted border-b border-border">
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Workload ID</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Status & Health</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Deployment Phase</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Provisioned Resources</TableHead>
                            <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase py-4">Operations</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {requests.length === 0 ? (
                            <TableRow>
                                <TableCell colSpan={5} className="h-48 text-center">
                                    <div className="flex flex-col items-center justify-center opacity-40">
                                        <Box className="h-12 w-12 mb-2" />
                                        <p className="text-sm font-bold text-muted-foreground">No active resource requests found.</p>
                                    </div>
                                </TableCell>
                            </TableRow>
                        ) : (
                            requests.map((req) => (
                                (() => {
                                    const st = displayStatus(req)
                                    const badgeClass = cn(
                                        "text-[11px] font-black h-6 border-0 rounded-full",
                                        st.variant === "failed"
                                            ? "bg-destructive text-destructive-foreground"
                                            : st.variant === "ready"
                                                ? "bg-success text-success-foreground shadow-sm"
                                                : cn("bg-primary text-primary-foreground", st.pulse && "animate-pulse")
                                    )
                                    return (
                                        <TableRow key={req.id} className="group hover:bg-accent/20 transition-colors">
                                            <TableCell className="py-5">
                                                <div className="font-bold text-sm tracking-tight">{req.request.model_uid}</div>
                                                <div className="text-[10px] font-mono text-muted-foreground truncate max-w-[220px] bg-accent/40 inline-block px-1.5 rounded mt-1.5">{req.request.model_name}</div>
                                            </TableCell>
                                            <TableCell>
                                                <div className="space-y-1">
                                                    <Badge className={badgeClass}>{st.label}</Badge>
                                                    {st.variant === "failed" && st.reason && (
                                                        <div className="text-[10px] font-mono text-muted-foreground/80 max-w-[360px]">
                                                            <span className="truncate block">{st.reason}</span>
                                                            {st.reason.toLowerCase().includes("image") && onNavigate && (
                                                                <button
                                                                    onClick={() => onNavigate("images")}
                                                                    className="text-primary hover:underline font-sans font-bold mt-0.5 block"
                                                                >
                                                                    Go to Images →
                                                                </button>
                                                            )}
                                                        </div>
                                                    )}
                                                </div>
                                            </TableCell>
                                            <TableCell>
                                                <div className="text-xs font-bold text-foreground">{fmtTime(req.created_at_ms)}</div>
                                                <p className="text-[10px] font-medium text-muted-foreground/60">Cluster submission time</p>
                                            </TableCell>
                                            <TableCell>
                                                <div className="flex items-center gap-3">
                                                    <div className="text-center bg-accent/30 rounded-lg px-2 py-1">
                                                        <p className="text-[9px] font-bold text-muted-foreground uppercase">Engine</p>
                                                        <p className="text-xs font-bold">{req.request.engine_type === "sglang" ? "SGLang" : "vLLM"}</p>
                                                    </div>
                                                    {req.request.config?.max_model_len && (
                                                        <div className="text-center bg-accent/30 rounded-lg px-2 py-1">
                                                            <p className="text-[9px] font-bold text-muted-foreground uppercase">Context</p>
                                                            <p className="text-xs font-bold">{req.request.config.max_model_len}</p>
                                                        </div>
                                                    )}
                                                    {req.request.replicas && (
                                                        <div className="text-center bg-accent/30 rounded-lg px-2 py-1">
                                                            <p className="text-[9px] font-bold text-muted-foreground uppercase">Replicas</p>
                                                            <p className="text-xs font-bold">{req.request.replicas}</p>
                                                        </div>
                                                    )}
                                                </div>
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleUnload(req.id)}
                                                    className="text-destructive font-bold text-xs hover:text-white hover:bg-destructive rounded-xl transition-all h-9"
                                                >
                                                    <Trash2 className="h-4 w-4 mr-1.5" />
                                                    Unload
                                                </Button>
                                            </TableCell>
                                        </TableRow>
                                    )
                                })()
                            ))
                        )}
                    </TableBody>
                </Table>
            </div>
        </div>
    )
}
