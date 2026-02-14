import { useCallback, useEffect, useMemo, useState } from "react"
import { Monitor, Search, Thermometer, Cpu, Activity, AlertTriangle } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Progress } from "@/components/ui/progress"
import {
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table"
import {
    BarChart, Bar, LineChart, Line, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip, Legend,
} from "recharts"
import { apiGet, v2 } from "@/lib/api"
import type { ClusterStatus, DiskAlert, EndpointStats } from "@/lib/types"

interface DashboardProps {
    overview: ClusterStatus
    counts: { nodes: number; endpoints: number; requests: number }
    gpuStats: { total: number; used: number; count: number }
    pct: (used: number, total: number) => number
    engineStats: EndpointStats[]
    token: string
}

interface MetricPoint {
    timestamp: string
    value: number
    labels?: Record<string, string>
}

interface MetricQueryResult {
    points: MetricPoint[]
}

function greeting(): string {
    const h = new Date().getHours()
    if (h < 6) return "Good Night"
    if (h < 12) return "Good Morning"
    if (h < 18) return "Good Afternoon"
    return "Good Evening"
}

export function DashboardView({ overview, counts, gpuStats, pct, engineStats, token }: DashboardProps) {
    const gpuUsagePct = gpuStats.count > 0 ? pct(gpuStats.used, gpuStats.total) : 0

    // Disk alerts from v2
    const [diskAlerts, setDiskAlerts] = useState<DiskAlert[]>([])
    const refreshDiskAlerts = useCallback(() => {
        v2.listAlerts(token)
            .then(setDiskAlerts)
            .catch(() => setDiskAlerts([]))
    }, [token])

    useEffect(() => {
        refreshDiskAlerts()
        const id = setInterval(refreshDiskAlerts, 30000)
        return () => clearInterval(id)
    }, [refreshDiskAlerts])

    // GPU utilization trend data from xtrace
    const [gpuTrend, setGpuTrend] = useState<{ time: string; utilization: number; temperature: number }[]>([])

    const fetchGpuTrend = useCallback(async () => {
        try {
            const now = new Date()
            const from = new Date(now.getTime() - 60 * 60 * 1000).toISOString() // 1h ago
            const to = now.toISOString()

            const [utilData, tempData] = await Promise.all([
                apiGet<MetricQueryResult>(
                    `/observe/metrics/query?name=gpu_utilization&from=${from}&to=${to}&step=60`,
                    token
                ).catch(() => ({ points: [] })),
                apiGet<MetricQueryResult>(
                    `/observe/metrics/query?name=gpu_temperature&from=${from}&to=${to}&step=60`,
                    token
                ).catch(() => ({ points: [] })),
            ])

            // Merge by timestamp
            const map = new Map<string, { utilization: number; temperature: number; count: number; tempCount: number }>()
            for (const p of utilData.points) {
                const key = p.timestamp.slice(11, 16) // HH:MM
                const existing = map.get(key) || { utilization: 0, temperature: 0, count: 0, tempCount: 0 }
                existing.utilization += p.value
                existing.count += 1
                map.set(key, existing)
            }
            for (const p of tempData.points) {
                const key = p.timestamp.slice(11, 16)
                const existing = map.get(key) || { utilization: 0, temperature: 0, count: 0, tempCount: 0 }
                existing.temperature += p.value
                existing.tempCount += 1
                map.set(key, existing)
            }

            const trend = Array.from(map.entries())
                .sort(([a], [b]) => a.localeCompare(b))
                .map(([time, v]) => ({
                    time,
                    utilization: v.count > 0 ? Math.round(v.utilization / v.count) : 0,
                    temperature: v.tempCount > 0 ? Math.round(v.temperature / v.tempCount) : 0,
                }))

            if (trend.length > 0) setGpuTrend(trend)
        } catch { /* xtrace may not be available */ }
    }, [token])

    useEffect(() => {
        fetchGpuTrend()
        const id = setInterval(fetchGpuTrend, 30000)
        return () => clearInterval(id)
    }, [fetchGpuTrend])

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

    // GPU summary: avg utilization and max temperature
    const gpuSummary = useMemo(() => {
        let utilSum = 0, utilCount = 0, maxTemp = 0
        for (const node of overview.nodes) {
            for (const gpu of node.gpus) {
                if (gpu.utilization_gpu != null) { utilSum += gpu.utilization_gpu; utilCount++ }
                if (gpu.temperature_c != null && gpu.temperature_c > maxTemp) maxTemp = gpu.temperature_c
            }
        }
        return {
            avgUtil: utilCount > 0 ? Math.round(utilSum / utilCount) : 0,
            maxTemp,
        }
    }, [overview.nodes])

    // Build stats lookup for endpoint table
    const statsMap = useMemo(() => {
        const m = new Map<string, EndpointStats>()
        for (const s of engineStats) {
            m.set(`${s.model_uid}-${s.replica_id}`, s)
        }
        return m
    }, [engineStats])

    // Endpoint table rows from real data
    const endpointRows = useMemo(() => {
        return overview.endpoints.map((ep) => {
            let gpuIndex: number | null = null
            for (const p of overview.placements) {
                if (p.model_uid === ep.model_uid) {
                    const a = p.assignments.find((a) => a.node_id === ep.node_id && a.replica_id === ep.replica_id)
                    if (a?.gpu_index != null) gpuIndex = a.gpu_index
                    break
                }
            }
            const gpu = gpuIndex != null ? `GPU ${gpuIndex}` : "CPU"
            let memUsed = ""
            for (const node of overview.nodes) {
                if (node.node_id === ep.node_id) {
                    const g = gpuIndex != null ? node.gpus.find((g) => g.index === gpuIndex) : null
                    if (g) memUsed = `${g.memory_used_mb.toLocaleString()} MB`
                    break
                }
            }

            const es = statsMap.get(`${ep.model_uid}-${ep.replica_id}`)
            const kvUsed = es?.kv_cache_used_bytes ?? 0
            const kvFree = es?.kv_cache_free_bytes ?? 0
            const kvTotal = kvUsed + kvFree
            const kvPct = kvTotal > 0 ? Math.round((kvUsed / kvTotal) * 100) : -1

            return {
                key: `${ep.model_uid}-${ep.replica_id}`,
                model: ep.model_uid,
                node: ep.node_id,
                gpu,
                memUsed: memUsed || "—",
                kvPct,
                pending: es?.pending_requests ?? 0,
                status: ep.status?.toLowerCase().includes("ready") || ep.status?.toLowerCase().includes("run") ? "ready" as const : "loading" as const,
            }
        })
    }, [overview.endpoints, overview.nodes, overview.placements, statsMap])

    return (
        <div className="space-y-5">
            {/* Header */}
            <div>
                <h2 className="text-2xl font-bold text-foreground mb-1">Overview</h2>
                <div className="mt-6 mb-2">
                    <h3 className="text-xl font-bold text-foreground">{greeting()}</h3>
                    <p className="text-sm text-muted-foreground mt-1">Here's an overview of your cluster health and active models</p>
                </div>
            </div>

            {/* Disk alert banners */}
            {diskAlerts.length > 0 && (
                <div className="space-y-2">
                    {diskAlerts.map((alert, i) => {
                        const isCritical = alert.alert_type === "disk_critical"
                        return (
                            <div key={i} className={`flex items-center gap-3 rounded-xl px-4 py-3 text-sm ${
                                isCritical
                                    ? "bg-destructive/10 text-destructive border border-destructive/20"
                                    : "bg-yellow-500/10 text-yellow-700 border border-yellow-500/20"
                            }`}>
                                <AlertTriangle className="h-4 w-4 shrink-0" />
                                <span className="font-medium">{alert.node_id}:</span>
                                <span>{alert.message}</span>
                                <Badge variant={isCritical ? "destructive" : "warning"} className="ml-auto text-[10px]">
                                    {isCritical ? "critical" : "warning"}
                                </Badge>
                            </div>
                        )
                    })}
                </div>
            )}

            {/* Summary Cards Row */}
            <div className="grid grid-cols-4 gap-4">
                {/* GPU Memory */}
                <div className="bg-card border border-border rounded-2xl p-5">
                    <div className="flex items-center justify-between mb-3">
                        <span className="text-sm text-muted-foreground">GPU Memory</span>
                        <Monitor className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <div className="flex items-center gap-2">
                        <span className="text-2xl font-bold text-foreground">{gpuUsagePct}%</span>
                        <Badge className="bg-success/10 text-success border-0 text-xs font-medium hover:bg-success/10">
                            {gpuUsagePct > 80 ? "High" : "Healthy"}
                        </Badge>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1">{Math.round(gpuStats.used / 1024)} / {Math.round(gpuStats.total / 1024)} GB</p>
                </div>

                {/* GPU Utilization */}
                <div className="bg-card border border-border rounded-2xl p-5">
                    <div className="flex items-center justify-between mb-3">
                        <span className="text-sm text-muted-foreground">Avg Utilization</span>
                        <Cpu className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <span className="text-2xl font-bold text-foreground">{gpuSummary.avgUtil}%</span>
                    <p className="text-xs text-muted-foreground mt-1">{gpuStats.count} GPUs across {counts.nodes} nodes</p>
                </div>

                {/* Temperature */}
                <div className="bg-card border border-border rounded-2xl p-5">
                    <div className="flex items-center justify-between mb-3">
                        <span className="text-sm text-muted-foreground">Max Temperature</span>
                        <Thermometer className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <div className="flex items-center gap-2">
                        <span className="text-2xl font-bold text-foreground">{gpuSummary.maxTemp > 0 ? `${gpuSummary.maxTemp}°C` : "—"}</span>
                        {gpuSummary.maxTemp > 80 && (
                            <Badge className="bg-destructive/10 text-destructive border-0 text-xs font-medium hover:bg-destructive/10">Hot</Badge>
                        )}
                    </div>
                    <p className="text-xs text-muted-foreground mt-1">{counts.endpoints} active endpoints</p>
                </div>

                {/* Endpoints */}
                <div className="bg-card border border-border rounded-2xl p-5">
                    <div className="flex items-center justify-between mb-3">
                        <span className="text-sm text-muted-foreground">Active Endpoints</span>
                        <Activity className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <span className="text-2xl font-bold text-foreground">{counts.endpoints}</span>
                    <p className="text-xs text-muted-foreground mt-1">{overview.model_requests.length} model requests</p>
                </div>
            </div>

            {/* Charts Row: GPU Memory Bar + GPU Trend Line */}
            <div className="grid grid-cols-2 gap-5">
                {/* GPU Memory Usage — Bar Chart */}
                <div className="bg-card border border-border rounded-2xl p-6">
                    <div className="flex items-center justify-between mb-5">
                        <h3 className="text-base font-bold text-foreground">GPU Memory Usage</h3>
                        <span className="text-sm text-muted-foreground">{Math.round(gpuStats.total / 1024)} GB total</span>
                    </div>
                    {gpuBarData.length === 0 ? (
                        <p className="text-sm text-muted-foreground py-12 text-center">No GPU data available.</p>
                    ) : (
                        <div className="h-56">
                            <ResponsiveContainer width="100%" height="100%">
                                <BarChart data={gpuBarData} barGap={2}>
                                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                                    <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }} />
                                    <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }} unit=" MB" />
                                    <Tooltip
                                        contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 8, fontSize: 12 }}
                                        formatter={(value) => `${Number(value).toLocaleString()} MB`}
                                    />
                                    <Bar dataKey="memUsed" stackId="a" fill="hsl(var(--chart-1))" name="Used" barSize={36} />
                                    <Bar dataKey="memFree" stackId="a" fill="hsl(var(--chart-2))" name="Free" barSize={36} radius={[4, 4, 0, 0]} />
                                </BarChart>
                            </ResponsiveContainer>
                        </div>
                    )}
                </div>

                {/* GPU Trend — Line Chart (from xtrace) */}
                <div className="bg-card border border-border rounded-2xl p-6">
                    <div className="flex items-center justify-between mb-5">
                        <h3 className="text-base font-bold text-foreground">GPU Trend (1h)</h3>
                        <span className="text-xs text-muted-foreground">from xtrace metrics</span>
                    </div>
                    {gpuTrend.length === 0 ? (
                        <p className="text-sm text-muted-foreground py-12 text-center">No trend data yet. Metrics will appear after xtrace collects data.</p>
                    ) : (
                        <div className="h-56">
                            <ResponsiveContainer width="100%" height="100%">
                                <LineChart data={gpuTrend}>
                                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                                    <XAxis dataKey="time" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }} />
                                    <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }} domain={[0, 100]} unit="%" />
                                    <Tooltip contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 8, fontSize: 12 }} />
                                    <Legend />
                                    <Line type="monotone" dataKey="utilization" stroke="hsl(var(--chart-1))" strokeWidth={2} dot={false} name="Utilization %" />
                                    <Line type="monotone" dataKey="temperature" stroke="hsl(var(--chart-3, 0 80% 60%))" strokeWidth={2} dot={false} name="Temp °C" />
                                </LineChart>
                            </ResponsiveContainer>
                        </div>
                    )}
                </div>
            </div>

            {/* Endpoint Table — Enhanced */}
            <div className="bg-card border border-border rounded-2xl p-6">
                <div className="flex items-center justify-between mb-4">
                    <h3 className="text-base font-bold text-foreground">Active Endpoints</h3>
                    <div className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 max-w-[200px]">
                        <Search className="h-4 w-4 text-muted-foreground" />
                        <input
                            type="text"
                            placeholder="Search..."
                            className="bg-transparent text-sm outline-none w-full text-foreground placeholder:text-muted-foreground"
                        />
                    </div>
                </div>

                <Table>
                    <TableHeader>
                        <TableRow className="hover:bg-transparent">
                            <TableHead className="font-medium">Model</TableHead>
                            <TableHead className="font-medium">Node</TableHead>
                            <TableHead className="font-medium">GPU</TableHead>
                            <TableHead className="font-medium">VRAM</TableHead>
                            <TableHead className="font-medium">KV Cache</TableHead>
                            <TableHead className="font-medium">Pending</TableHead>
                            <TableHead className="font-medium">Status</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {endpointRows.length === 0 ? (
                            <TableRow>
                                <TableCell colSpan={7} className="text-center py-8 text-muted-foreground">
                                    No endpoints online. Load a model to get started.
                                </TableCell>
                            </TableRow>
                        ) : (
                            endpointRows.map((ep) => {
                                const kvOverloaded = ep.kvPct > 95
                                return (
                                    <TableRow key={ep.key}>
                                        <TableCell className="font-mono text-sm">{ep.model}</TableCell>
                                        <TableCell className="text-sm">{ep.node}</TableCell>
                                        <TableCell className="text-sm text-muted-foreground">{ep.gpu}</TableCell>
                                        <TableCell className="text-sm font-medium">{ep.memUsed}</TableCell>
                                        <TableCell>
                                            {ep.kvPct >= 0 ? (
                                                <div className="w-20">
                                                    <div className="flex justify-between text-xs mb-1">
                                                        <span className={kvOverloaded ? "text-destructive font-bold" : "text-muted-foreground"}>{ep.kvPct}%</span>
                                                    </div>
                                                    <Progress value={ep.kvPct} className={`h-1.5 ${kvOverloaded ? "[&>div]:bg-destructive" : ""}`} />
                                                </div>
                                            ) : (
                                                <span className="text-xs text-muted-foreground">—</span>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <span className={`text-sm font-bold ${ep.pending > 5 ? "text-yellow-600" : "text-foreground"}`}>
                                                {ep.pending}
                                            </span>
                                        </TableCell>
                                        <TableCell>
                                            <span className={`text-xs font-medium px-2 py-1 rounded-full ${ep.status === "ready" ? "bg-success/10 text-success" : "bg-accent text-muted-foreground"}`}>
                                                {ep.status}
                                            </span>
                                        </TableCell>
                                    </TableRow>
                                )
                            })
                        )}
                    </TableBody>
                </Table>
            </div>
        </div>
    )
}
