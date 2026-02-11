import { Activity, Cpu, Server, Sparkles, ChevronRight, Monitor } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Progress } from "@/components/ui/progress"
import StatsCard from "@/components/StatsCard"
import { LineChart, Line, ResponsiveContainer } from "recharts"
import type { ClusterStatus } from "@/lib/types"
import { cn } from "@/lib/utils"

interface DashboardProps {
    overview: ClusterStatus
    counts: { nodes: number; endpoints: number; requests: number }
    gpuStats: { total: number; used: number; count: number }
    gpuModel: (nodeId: string, gpuIdx: number) => string | null
    pct: (used: number, total: number) => number
}

const sparkData = [
    { v: 30 }, { v: 45 }, { v: 34 }, { v: 50 }, { v: 38 }, { v: 42 }, { v: 34 }, { v: 55 }, { v: 40 }, { v: 34 },
];

export function DashboardView({ overview, counts, gpuStats, gpuModel, pct }: DashboardProps) {
    const gpuUsagePct = gpuStats.count > 0 ? pct(gpuStats.used, gpuStats.total) : 0;

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            {/* Welcome Header */}
            <div>
                <h2 className="text-2xl font-bold text-foreground">Overview</h2>
                <div className="mt-4">
                    <h3 className="text-xl font-bold text-foreground">Welcome back, Admin</h3>
                    <p className="text-sm text-muted-foreground mt-1">Here's an overview of your cluster health and active models</p>
                </div>
            </div>

            {/* Hero Sparkline Card */}
            <div className="bg-card border border-border rounded-2xl p-6 flex flex-col md:flex-row items-center justify-between gap-6 shadow-sm">
                <div className="flex items-center gap-5">
                    <div className="h-14 w-14 rounded-2xl bg-accent flex items-center justify-center">
                        <Monitor className="h-6 w-6 text-muted-foreground" />
                    </div>
                    <div>
                        <p className="text-sm font-semibold text-muted-foreground mb-1 uppercase tracking-tight">System GPU Utilization</p>
                        <div className="flex items-center gap-3">
                            <span className="text-4xl font-bold text-foreground tracking-tighter">{gpuUsagePct}%</span>
                            <Badge className="bg-success/10 text-success border-0 text-xs font-bold px-2 py-0.5 hover:bg-success/10">
                                ↑ Healthy
                            </Badge>
                        </div>
                    </div>
                </div>
                <div className="flex items-center gap-6 w-full md:w-auto">
                    <div className="w-48 h-14">
                        <ResponsiveContainer width="100%" height="100%">
                            <LineChart data={sparkData}>
                                <Line
                                    type="monotone"
                                    dataKey="v"
                                    stroke="hsl(var(--muted-foreground))"
                                    strokeWidth={2}
                                    dot={false}
                                    isAnimationActive={true}
                                />
                            </LineChart>
                        </ResponsiveContainer>
                    </div>
                    <div className="h-14 w-14 rounded-2xl border border-border flex items-center justify-center shadow-inner">
                        <Activity className="h-6 w-6 text-muted-foreground" />
                    </div>
                </div>
            </div>

            {/* Grid of Stats */}
            <div className="grid gap-5 md:grid-cols-2 lg:grid-cols-4">
                <StatsCard
                    title="Total Nodes"
                    value={counts.nodes}
                    subtitle="Active compute nodes"
                    icon={Server}
                />
                <StatsCard
                    title="Active Endpoints"
                    value={counts.endpoints}
                    subtitle="Model instances online"
                    icon={Cpu}
                />
                <StatsCard
                    title="Cluster Requests"
                    value={counts.requests}
                    subtitle="Load requests in queue"
                    icon={Sparkles}
                />
                <StatsCard
                    title="GPU Count"
                    value={gpuStats.count}
                    subtitle="Total visible GPUs"
                    icon={Activity}
                />
            </div>

            {/* Detail Sections */}
            <div className="grid gap-6 lg:grid-cols-7">
                <div className="lg:col-span-4 space-y-4">
                    <div className="flex items-center justify-between px-1">
                        <h3 className="text-lg font-bold text-foreground">GPU Hardware Overview</h3>
                        <button className="text-xs font-bold text-muted-foreground hover:text-primary transition-colors">View All Nodes</button>
                    </div>

                    <div className="bg-card border border-border rounded-2xl p-6 shadow-sm space-y-8">
                        {overview.nodes.length === 0 ? (
                            <div className="flex flex-col items-center justify-center py-12 text-center">
                                <Server className="h-12 w-12 text-muted-foreground/20 mb-3" />
                                <p className="text-sm font-medium text-muted-foreground">No compute nodes registered in the cluster.</p>
                            </div>
                        ) : (
                            overview.nodes.map((node) => (
                                <div key={node.node_id} className="space-y-5">
                                    <div className="flex items-center gap-3">
                                        <div className="bg-accent/50 p-1.5 rounded-lg">
                                            <Server className="h-4 w-4 text-muted-foreground" />
                                        </div>
                                        <span className="text-sm font-bold tracking-tight">{node.node_id}</span>
                                        <Badge variant="secondary" className="text-[10px] font-bold px-1.5 py-0 bg-accent text-accent-foreground">
                                            {node.gpus.length} GPUs
                                        </Badge>
                                    </div>
                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-6">
                                        {node.gpus.map((gpu) => {
                                            const model = gpuModel(node.node_id, gpu.index)
                                            const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                                            return (
                                                <div key={gpu.index} className="bg-accent/20 border border-border/40 rounded-xl p-4 space-y-3">
                                                    <div className="flex items-center justify-between text-xs font-bold">
                                                        <span className="text-muted-foreground">UNIT {gpu.index}</span>
                                                        <span className={cn(usage > 80 ? "text-destructive" : "text-primary")}>{usage}%</span>
                                                    </div>
                                                    <Progress value={usage} className="h-1.5 bg-border/50" />
                                                    <div className="flex items-center justify-between text-[10px] font-medium text-muted-foreground/80">
                                                        <span>
                                                            {Math.round(gpu.memory_used_mb / 1024 * 10) / 10} / {Math.round(gpu.memory_total_mb / 1024 * 10) / 10} GB
                                                        </span>
                                                        {model && (
                                                            <span className="font-bold text-primary truncate max-w-[120px]">{model}</span>
                                                        )}
                                                    </div>
                                                </div>
                                            )
                                        })}
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </div>

                <div className="lg:col-span-3 space-y-4">
                    <div className="flex items-center justify-between px-1">
                        <h3 className="text-lg font-bold text-foreground">Active Endpoints</h3>
                        <div className="flex h-2 w-2 rounded-full bg-success animate-pulse" />
                    </div>

                    <div className="bg-card border border-border rounded-2xl p-6 shadow-sm min-h-[400px]">
                        {overview.endpoints.length === 0 ? (
                            <div className="flex flex-col items-center justify-center py-20 text-center">
                                <Cpu className="h-12 w-12 text-muted-foreground/20 mb-3" />
                                <p className="text-sm font-medium text-muted-foreground text-center max-w-[200px]">
                                    No model instances are currently online.
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-4">
                                {overview.endpoints.map((ep) => (
                                    <div
                                        key={`${ep.model_uid}-${ep.replica_id}`}
                                        className="flex items-center justify-between rounded-xl border border-border/50 p-4 hover:border-primary/30 hover:bg-sidebar-accent/50 transition-all cursor-pointer group"
                                    >
                                        <div className="space-y-1.5">
                                            <div className="flex items-center gap-2">
                                                <span className="text-sm font-bold font-mono tracking-tight">{ep.model_uid}</span>
                                                <Badge className="text-[10px] font-bold h-4 bg-success/10 text-success border-0">
                                                    {ep.status}
                                                </Badge>
                                            </div>
                                            <p className="text-[11px] font-medium text-muted-foreground">
                                                {ep.node_id} · Unit {ep.gpu_index ?? 'CPU'}
                                            </p>
                                        </div>
                                        <div className="h-8 w-8 rounded-full flex items-center justify-center group-hover:bg-primary/10 transition-colors">
                                            <ChevronRight className="h-4 w-4 text-muted-foreground group-hover:text-primary" />
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    )
}
