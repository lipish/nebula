import { useMemo } from "react"
import { Monitor, Search, Filter, MoreHorizontal } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import {
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table"
import {
    BarChart, Bar, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip,
} from "recharts"
import type { ClusterStatus } from "@/lib/types"

interface DashboardProps {
    overview: ClusterStatus
    counts: { nodes: number; endpoints: number; requests: number }
    gpuStats: { total: number; used: number; count: number }
    pct: (used: number, total: number) => number
}

export function DashboardView({ overview, counts, gpuStats, pct }: DashboardProps) {
    const gpuUsagePct = gpuStats.count > 0 ? pct(gpuStats.used, gpuStats.total) : 0

    // Real GPU memory bar chart data
    const gpuBarData = useMemo(() => {
        const rows: { name: string; memUsed: number; memFree: number }[] = []
        for (const node of overview.nodes) {
            for (const gpu of node.gpus) {
                rows.push({
                    name: `${node.node_id} GPU ${gpu.index}`,
                    memUsed: gpu.memory_used_mb,
                    memFree: gpu.memory_total_mb - gpu.memory_used_mb,
                })
            }
        }
        return rows
    }, [overview.nodes])

    // Endpoint table rows from real data, gpu_index comes from placements
    const endpointRows = useMemo(() => {
        return overview.endpoints.map((ep) => {
            // Find gpu_index from placements
            let gpuIndex: number | null = null
            for (const p of overview.placements) {
                if (p.model_uid === ep.model_uid) {
                    const a = p.assignments.find((a) => a.node_id === ep.node_id && a.replica_id === ep.replica_id)
                    if (a?.gpu_index != null) gpuIndex = a.gpu_index
                    break
                }
            }
            const gpu = gpuIndex != null ? `GPU ${gpuIndex}` : "CPU"
            // Find memory usage for this GPU
            let memUsed = ""
            for (const node of overview.nodes) {
                if (node.node_id === ep.node_id) {
                    const g = gpuIndex != null ? node.gpus.find((g) => g.index === gpuIndex) : null
                    if (g) memUsed = `${g.memory_used_mb.toLocaleString()} MB`
                    break
                }
            }
            return {
                model: ep.model_uid,
                node: ep.node_id,
                gpu,
                memUsed: memUsed || "—",
                status: ep.status?.toLowerCase().includes("ready") || ep.status?.toLowerCase().includes("run") ? "ready" as const : "loading" as const,
            }
        })
    }, [overview.endpoints, overview.nodes, overview.placements])

    return (
        <div className="space-y-5">
            {/* Header */}
            <div>
                <h2 className="text-2xl font-bold text-foreground mb-1">Overview</h2>
                <div className="mt-6 mb-2">
                    <h3 className="text-xl font-bold text-foreground">Good Morning, Nero</h3>
                    <p className="text-sm text-muted-foreground mt-1">Here's an overview of your cluster health and active models</p>
                </div>
            </div>

            {/* Cluster Summary — GPU Utilization */}
            <div className="bg-card border border-border rounded-2xl p-6 flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <div className="h-12 w-12 rounded-xl bg-accent flex items-center justify-center">
                        <Monitor className="h-5 w-5 text-muted-foreground" />
                    </div>
                    <div>
                        <p className="text-sm text-muted-foreground mb-1">GPU Utilization</p>
                        <div className="flex items-center gap-3">
                            <span className="text-3xl font-bold text-foreground">{gpuUsagePct}%</span>
                            <Badge className="bg-success/10 text-success border-0 text-xs font-medium hover:bg-success/10">
                                {gpuUsagePct > 80 ? "↑ High" : "↑ Healthy"}
                            </Badge>
                        </div>
                    </div>
                </div>
                <div className="flex items-center gap-6 text-center">
                    <div>
                        <p className="text-2xl font-bold text-foreground">{counts.nodes}</p>
                        <p className="text-xs text-muted-foreground">Nodes</p>
                    </div>
                    <div>
                        <p className="text-2xl font-bold text-foreground">{gpuStats.count}</p>
                        <p className="text-xs text-muted-foreground">GPUs</p>
                    </div>
                    <div>
                        <p className="text-2xl font-bold text-foreground">{counts.endpoints}</p>
                        <p className="text-xs text-muted-foreground">Endpoints</p>
                    </div>
                </div>
            </div>

            {/* GPU Memory Usage — Bar Chart (real data) */}
            <div className="bg-card border border-border rounded-2xl p-6">
                <div className="flex items-center justify-between mb-6">
                    <h3 className="text-lg font-bold text-foreground">GPU Memory Usage</h3>
                    <div className="flex items-center gap-2 text-sm text-muted-foreground">
                        <span>Total VRAM</span>
                        <span className="font-bold text-foreground text-lg">{Math.round(gpuStats.total / 1024)} GB</span>
                    </div>
                </div>
                {gpuBarData.length === 0 ? (
                    <p className="text-sm text-muted-foreground py-12 text-center">No GPU data available.</p>
                ) : (
                    <div className="h-64">
                        <ResponsiveContainer width="100%" height="100%">
                            <BarChart data={gpuBarData} barGap={2}>
                                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }} />
                                <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }} unit=" MB" />
                                <Tooltip
                                    contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 8, fontSize: 13 }}
                                    formatter={(value: number) => `${value.toLocaleString()} MB`}
                                />
                                <Bar dataKey="memUsed" stackId="a" fill="hsl(var(--chart-1))" name="Used" barSize={40} />
                                <Bar dataKey="memFree" stackId="a" fill="hsl(var(--chart-2))" name="Free" barSize={40} radius={[4, 4, 0, 0]} />
                            </BarChart>
                        </ResponsiveContainer>
                    </div>
                )}
            </div>

            {/* Endpoint Table */}
            <div className="bg-card border border-border rounded-2xl p-6">
                <div className="flex items-center gap-3 mb-4">
                    <div className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 flex-1 max-w-[200px]">
                        <Search className="h-4 w-4 text-muted-foreground" />
                        <input
                            type="text"
                            placeholder="Search models..."
                            className="bg-transparent text-sm outline-none w-full text-foreground placeholder:text-muted-foreground"
                        />
                    </div>
                    <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
                        <Filter className="h-4 w-4" />
                        All Status
                    </button>
                    <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
                        <MoreHorizontal className="h-4 w-4" />
                        More
                    </button>
                </div>

                <Table>
                    <TableHeader>
                        <TableRow className="hover:bg-transparent">
                            <TableHead className="w-10"><Checkbox /></TableHead>
                            <TableHead className="font-medium">Model</TableHead>
                            <TableHead className="font-medium">Node</TableHead>
                            <TableHead className="font-medium">GPU</TableHead>
                            <TableHead className="font-medium">Memory</TableHead>
                            <TableHead className="font-medium">Status</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {endpointRows.length === 0 ? (
                            <TableRow>
                                <TableCell colSpan={6} className="text-center py-8 text-muted-foreground">
                                    No endpoints online. Load a model to get started.
                                </TableCell>
                            </TableRow>
                        ) : (
                            endpointRows.map((ep) => (
                                <TableRow key={ep.model}>
                                    <TableCell><Checkbox /></TableCell>
                                    <TableCell className="font-mono text-sm">{ep.model}</TableCell>
                                    <TableCell className="text-sm">{ep.node}</TableCell>
                                    <TableCell className="text-sm text-muted-foreground">{ep.gpu}</TableCell>
                                    <TableCell className="text-sm font-medium">{ep.memUsed}</TableCell>
                                    <TableCell>
                                        <span className={`text-xs font-medium px-2 py-1 rounded-full ${ep.status === "ready" ? "bg-success/10 text-success" : "bg-accent text-muted-foreground"}`}>
                                            {ep.status}
                                        </span>
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
