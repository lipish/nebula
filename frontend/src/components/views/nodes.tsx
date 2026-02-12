import { Cpu, Server, Activity, Thermometer, Gauge } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Progress } from "@/components/ui/progress"
import type { ClusterStatus } from "@/lib/types"
import { cn } from "@/lib/utils"

interface NodesProps {
    overview: ClusterStatus
    gpuModel: (nodeId: string, gpuIdx: number) => string | null
    pct: (used: number, total: number) => number
    fmtTime: (v: number) => string
}

export function NodesView({ overview, gpuModel, pct, fmtTime }: NodesProps) {
    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div>
                <h2 className="text-2xl font-bold text-foreground">Nodes & GPUs</h2>
                <p className="text-sm text-muted-foreground mt-1">Monitor compute infrastructure and GPU resources</p>
            </div>

            {overview.nodes.length === 0 ? (
                <div className="bg-card border border-border rounded-2xl p-20 flex flex-col items-center justify-center text-center shadow-sm">
                    <Server className="h-16 w-16 text-muted-foreground/20 mb-4" />
                    <h3 className="text-lg font-bold text-foreground">No Nodes Detected</h3>
                    <p className="text-sm text-muted-foreground mt-1 max-w-sm">
                        Ensure the <span className="font-mono bg-accent px-1 rounded">nebula-node</span> process is running on your compute servers and heartbeating to etcd.
                    </p>
                </div>
            ) : (
                <div className="space-y-10">
                    {overview.nodes.map((node) => (
                        <div key={node.node_id} className="space-y-5">
                            {/* Node Header */}
                            <div className="flex items-center justify-between px-1">
                                <div className="flex items-center gap-4">
                                    <div className="h-12 w-12 rounded-2xl bg-primary text-primary-foreground flex items-center justify-center shadow-sm">
                                        <Server className="h-6 w-6" />
                                    </div>
                                    <div>
                                        <h3 className="text-lg font-bold text-foreground tracking-tight">{node.node_id}</h3>
                                        <p className="text-xs font-semibold text-muted-foreground/80 uppercase tracking-wider">
                                            Heartbeat: {fmtTime(node.last_heartbeat_ms)}
                                        </p>
                                    </div>
                                </div>
                                <div className="flex items-center gap-3">
                                    <Badge className="bg-success/10 text-success border-0 font-bold px-3 py-1 hover:bg-success/10">
                                        ONLINE
                                    </Badge>
                                </div>
                            </div>

                            {/* GPUs Grid */}
                            <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
                                {node.gpus.map((gpu) => {
                                    const model = gpuModel(node.node_id, gpu.index)
                                    const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                                    return (
                                        <div key={gpu.index} className="bg-card border border-border rounded-2xl p-5 shadow-sm hover:shadow-md transition-all duration-200 space-y-5">
                                            <div className="flex items-center justify-between">
                                                <div className="flex items-center gap-2.5">
                                                    <div className="p-2 rounded-lg bg-accent/50">
                                                        <Cpu className="h-4 w-4 text-muted-foreground" />
                                                    </div>
                                                    <span className="text-sm font-bold text-foreground tracking-tight">GPU {gpu.index}</span>
                                                </div>
                                                <span className={cn("text-sm font-bold tracking-tighter", usage > 80 ? "text-destructive" : "text-primary")}>
                                                    {usage}%
                                                </span>
                                            </div>

                                            <div className="space-y-2">
                                                <Progress value={usage} className="h-1.5 bg-accent" />
                                                <div className="flex items-center justify-between text-[11px] font-bold text-muted-foreground/70 uppercase tracking-tighter">
                                                    <span>VRAM Usage</span>
                                                    <span className="text-foreground">
                                                        {Math.round(gpu.memory_used_mb / 1024 * 10) / 10}GB / {Math.round(gpu.memory_total_mb / 1024 * 10) / 10}GB
                                                    </span>
                                                </div>
                                            </div>

                                            {/* Temperature & Utilization */}
                                            <div className="flex items-center gap-4 text-[11px] font-bold text-muted-foreground/70 uppercase tracking-tighter">
                                                <div className="flex items-center gap-1.5 flex-1">
                                                    <Thermometer className="h-3.5 w-3.5" />
                                                    <span>Temp</span>
                                                    <span className={cn(
                                                        "ml-auto text-foreground",
                                                        gpu.temperature_c != null && gpu.temperature_c > 80 ? "text-destructive" : ""
                                                    )}>
                                                        {gpu.temperature_c != null ? `${gpu.temperature_c}°C` : "—"}
                                                    </span>
                                                </div>
                                                <div className="flex items-center gap-1.5 flex-1">
                                                    <Gauge className="h-3.5 w-3.5" />
                                                    <span>Util</span>
                                                    <span className={cn(
                                                        "ml-auto text-foreground",
                                                        gpu.utilization_gpu != null && gpu.utilization_gpu > 80 ? "text-destructive" : ""
                                                    )}>
                                                        {gpu.utilization_gpu != null ? `${gpu.utilization_gpu}%` : "—"}
                                                    </span>
                                                </div>
                                            </div>

                                            <div className="pt-2 border-t border-border/50">
                                                <div className="flex items-center justify-between">
                                                    <span className="text-[11px] font-bold text-muted-foreground/70 uppercase">Workload</span>
                                                    {model ? (
                                                        <Badge className="px-2 py-0 h-5 text-[10px] font-bold font-mono bg-primary text-primary-foreground border-0">
                                                            {model}
                                                        </Badge>
                                                    ) : (
                                                        <div className="flex items-center gap-1.5">
                                                            <Activity className="h-3 w-3 text-success" />
                                                            <span className="text-[11px] font-bold text-success uppercase">Available</span>
                                                        </div>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    )
                                })}
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    )
}
