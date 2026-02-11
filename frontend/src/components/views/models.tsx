import { Plus, Trash2, Box, Zap, Cpu, Server, ChevronRight } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import type { ClusterStatus, ModelLoadRequest, ModelRequest } from "@/lib/types"
import { cn } from "@/lib/utils"

interface ModelsProps {
    overview: ClusterStatus
    requests: ModelRequest[]
    showLoadForm: boolean
    setShowLoadForm: (v: boolean) => void
    form: ModelLoadRequest
    setForm: (v: ModelLoadRequest) => void
    handleLoadModel: () => Promise<void>
    handleUnload: (id: string) => Promise<void>
    selectedGpu: { nodeId: string; gpuIndex: number } | null
    setSelectedGpu: (v: { nodeId: string; gpuIndex: number } | null) => void
    usedGpus: Map<string, Set<number>>
    statusVariant: (s: string) => any
    fmtTime: (v: number) => string
    pct: (used: number, total: number) => number
}

export function ModelsView({
    overview,
    requests,
    showLoadForm,
    setShowLoadForm,
    form,
    setForm,
    handleLoadModel,
    handleUnload,
    selectedGpu,
    setSelectedGpu,
    usedGpus,
    statusVariant,
    fmtTime,
    pct
}: ModelsProps) {
    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-2xl font-bold text-foreground">Model Management</h2>
                    <p className="text-sm text-muted-foreground mt-1">Load, monitor, and manage model deployments</p>
                </div>
                <Button
                    onClick={() => setShowLoadForm(!showLoadForm)}
                    className="bg-primary hover:bg-primary/90 rounded-xl shadow-sm px-5"
                >
                    {showLoadForm ? "Hide Form" : (
                        <>
                            <Plus className="mr-2 h-4 w-4" />
                            Load Model
                        </>
                    )}
                </Button>
            </div>

            {showLoadForm && (
                <div className="bg-card border border-primary/20 rounded-2xl p-6 shadow-lg animate-in zoom-in-95 duration-200">
                    <div className="mb-6">
                        <h3 className="text-lg font-bold text-foreground">Deploy New Model</h3>
                        <p className="text-sm text-muted-foreground">Specify target hardware and model parameters</p>
                    </div>

                    <div className="space-y-8">
                        <div className="space-y-4">
                            <label className="text-xs font-bold text-muted-foreground uppercase tracking-wider">Target Hardware Selection</label>
                            <div className="grid gap-6 md:grid-cols-2">
                                {overview.nodes.map((node) => (
                                    <div key={node.node_id} className="space-y-3">
                                        <p className="text-[11px] font-bold text-muted-foreground/80 flex items-center gap-2">
                                            <Server className="h-3 w-3" />
                                            {node.node_id.toUpperCase()}
                                        </p>
                                        <div className="grid gap-3">
                                            {node.gpus.map((gpu) => {
                                                const isUsed = usedGpus.get(node.node_id)?.has(gpu.index) ?? false
                                                const isSel = selectedGpu?.nodeId === node.node_id && selectedGpu?.gpuIndex === gpu.index
                                                const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                                                return (
                                                    <button
                                                        key={gpu.index}
                                                        onClick={() => setSelectedGpu({ nodeId: node.node_id, gpuIndex: gpu.index })}
                                                        className={cn(
                                                            "relative group flex items-center justify-between rounded-xl border p-4 text-left transition-all duration-200 shadow-sm",
                                                            isSel
                                                                ? "border-primary bg-primary/[0.03] ring-1 ring-primary"
                                                                : "border-border hover:border-primary/40 hover:bg-accent/30"
                                                        )}
                                                    >
                                                        <div className="flex-1">
                                                            <div className="flex items-center justify-between mb-2">
                                                                <div className="flex items-center gap-2">
                                                                    <Cpu className={cn("h-4 w-4", isSel ? "text-primary" : "text-muted-foreground")} />
                                                                    <span className="text-xs font-bold">GPU {gpu.index}</span>
                                                                </div>
                                                                <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-tight">{usage}% BUSY</span>
                                                            </div>
                                                            <div className="h-1 w-full bg-accent rounded-full overflow-hidden">
                                                                <div
                                                                    className={cn("h-full rounded-full transition-all duration-500",
                                                                        usage > 80 ? "bg-destructive" : (isSel ? "bg-primary" : "bg-primary/50")
                                                                    )}
                                                                    style={{ width: `${usage}%` }}
                                                                />
                                                            </div>
                                                        </div>
                                                        {isUsed && !isSel && (
                                                            <Badge className="ml-3 text-[9px] font-bold bg-secondary text-secondary-foreground border-0">IN USE</Badge>
                                                        )}
                                                        {isSel && (
                                                            <div className="ml-3 h-5 w-5 rounded-full bg-primary flex items-center justify-center shadow-lg animate-in fade-in scale-in-0 duration-300">
                                                                <Zap className="h-3 w-3 text-primary-foreground fill-current" />
                                                            </div>
                                                        )}
                                                    </button>
                                                )
                                            })}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>

                        <div className="grid gap-6 sm:grid-cols-2 pt-4 border-t border-border/50">
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">HuggingFace Path / Model Hub Name</label>
                                <Input
                                    placeholder="e.g. Qwen/Qwen2.5-0.5B-Instruct"
                                    className="rounded-xl border-border/80 focus:ring-primary/20 h-11"
                                    value={form.model_name}
                                    onChange={(e) => setForm({ ...form, model_name: e.target.value })}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">Deployment Identifier (UID)</label>
                                <Input
                                    placeholder="e.g. production-qwen-7b"
                                    className="rounded-xl border-border/80 focus:ring-primary/20 h-11 font-mono"
                                    value={form.model_uid}
                                    onChange={(e) => setForm({ ...form, model_uid: e.target.value })}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">Model Replicas</label>
                                <Input
                                    type="number"
                                    className="rounded-xl border-border/80 h-11"
                                    value={form.replicas}
                                    onChange={(e) => setForm({ ...form, replicas: Number(e.target.value) })}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">Context Window Limit</label>
                                <Input
                                    type="number"
                                    placeholder="4096"
                                    className="rounded-xl border-border/80 h-11 font-mono"
                                    value={form.config?.max_model_len ?? ''}
                                    onChange={(e) => setForm({ ...form, config: { ...form.config, max_model_len: Number(e.target.value) } })}
                                />
                            </div>
                        </div>

                        <div className="flex justify-end gap-3 pt-6 border-t border-border/50">
                            <Button
                                variant="outline"
                                className="rounded-xl"
                                onClick={() => setShowLoadForm(false)}
                            >
                                Cancel
                            </Button>
                            <Button
                                className="bg-primary font-bold rounded-xl px-8 shadow-md"
                                onClick={handleLoadModel}
                                disabled={!selectedGpu || !form.model_name}
                            >
                                Launch Model Instance
                            </Button>
                        </div>
                    </div>
                </div>
            )}

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
                                <TableRow key={req.id} className="group hover:bg-accent/20 transition-colors">
                                    <TableCell className="py-5">
                                        <div className="font-bold text-sm tracking-tight">{req.request.model_uid}</div>
                                        <div className="text-[10px] font-mono text-muted-foreground truncate max-w-[220px] bg-accent/40 inline-block px-1.5 rounded mt-1.5">{req.request.model_name}</div>
                                    </TableCell>
                                    <TableCell>
                                        <Badge className={cn("text-[9px] font-black h-5 border-0 rounded-full",
                                            req.status.toLowerCase().includes('fail') ? "bg-destructive text-destructive-foreground" :
                                                req.status.toLowerCase().includes('ready') ? "bg-success text-success-foreground shadow-sm" :
                                                    "bg-primary text-primary-foreground animate-pulse"
                                        )}>
                                            {req.status.toUpperCase()}
                                        </Badge>
                                    </TableCell>
                                    <TableCell>
                                        <div className="text-xs font-bold text-foreground">{fmtTime(req.created_at_ms)}</div>
                                        <p className="text-[10px] font-medium text-muted-foreground/60">Cluster submission time</p>
                                    </TableCell>
                                    <TableCell>
                                        <div className="flex items-center gap-3">
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
                            ))
                        )}
                    </TableBody>
                </Table>
            </div>
        </div>
    )
}
