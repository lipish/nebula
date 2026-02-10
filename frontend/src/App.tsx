import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Activity,
  Boxes,
  ChevronRight,
  Cpu,
  LayoutDashboard,
  Plus,
  RefreshCw,
  Server,
  Settings,
  Sparkles,
  Trash2,
  TriangleAlert,
  Zap,
} from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { apiDelete, apiGet, apiPost } from '@/lib/api'
import type { ClusterStatus, GpuStatus, ModelLoadRequest, ModelRequest } from '@/lib/types'

const EMPTY_OVERVIEW: ClusterStatus = {
  nodes: [],
  endpoints: [],
  placements: [],
  model_requests: [],
}

type Page = 'dashboard' | 'models' | 'nodes' | 'settings'

/* ── helpers ── */
const statusVariant = (s: string): 'danger' | 'warning' | 'success' | 'muted' => {
  const n = s.toLowerCase()
  if (n.includes('fail')) return 'danger'
  if (n.includes('unload')) return 'warning'
  if (n.includes('run') || n.includes('ready')) return 'success'
  return 'muted'
}

const fmtTime = (v: number) => (v ? new Date(v).toLocaleString() : 'n/a')

const pct = (used: number, total: number) =>
  total > 0 ? Math.round((used / total) * 100) : 0

/* ── GPU bar ── */
function GpuBar({ gpu, selected, onClick }: { gpu: GpuStatus; selected?: boolean; onClick?: () => void }) {
  const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
  const color = usage > 80 ? 'bg-destructive' : usage > 50 ? 'bg-warning' : 'bg-primary'
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex flex-col gap-1.5 rounded-lg border p-3 text-left transition-all ${selected
        ? 'border-primary bg-primary/5 ring-2 ring-primary/20'
        : 'border-border bg-card hover:border-primary/40'
        }`}
    >
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-foreground">GPU {gpu.index}</span>
        <span className="text-xs text-muted-foreground">{usage}%</span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
        <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${usage}%` }} />
      </div>
      <span className="text-[11px] text-muted-foreground">
        {gpu.memory_used_mb.toLocaleString()} / {gpu.memory_total_mb.toLocaleString()} MB
      </span>
    </button>
  )
}

/* ── Sidebar ── */
function Sidebar({ page, onNavigate }: { page: Page; onNavigate: (p: Page) => void }) {
  const items: { id: Page; label: string; icon: typeof LayoutDashboard }[] = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'models', label: 'Models', icon: Boxes },
    { id: 'nodes', label: 'Nodes & GPUs', icon: Server },
    { id: 'settings', label: 'Settings', icon: Settings },
  ]
  return (
    <aside className="fixed inset-y-0 left-0 z-30 flex w-56 flex-col bg-sidebar border-r border-sidebar-border">
      <div className="flex h-14 items-center gap-2.5 px-5">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-sidebar-primary">
          <Zap className="h-4 w-4 text-white" />
        </div>
        <span className="text-lg font-bold text-white tracking-tight">Nebula</span>
      </div>
      <nav className="flex-1 space-y-0.5 px-3">
        {items.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => onNavigate(id)}
            className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors ${page === id
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'text-sidebar-foreground hover:bg-sidebar-accent/50 hover:text-white'
              }`}
          >
            <Icon className="h-4 w-4" />
            {label}
          </button>
        ))}
      </nav>
      <div className="border-t border-sidebar-border px-5 py-4">
        <div>
          <p className="text-xs font-medium text-sidebar-accent-foreground">Nebula Platform</p>
          <p className="text-[11px] text-sidebar-foreground">v0.1.0 · BFF Connected</p>
        </div>
      </div>
    </aside>
  )
}

/* ── Main App ── */
function App() {
  const [token, setToken] = useState(() => localStorage.getItem('nebula_token') || '')
  const [overview, setOverview] = useState<ClusterStatus>(EMPTY_OVERVIEW)
  const [requests, setRequests] = useState<ModelRequest[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState<Page>('dashboard')
  const [showLoadForm, setShowLoadForm] = useState(false)
  const [selectedGpu, setSelectedGpu] = useState<{ nodeId: string; gpuIndex: number } | null>(null)
  const [form, setForm] = useState<ModelLoadRequest>({
    model_name: '',
    model_uid: '',
    replicas: 1,
    config: { max_model_len: 4096 },
  })

  const counts = useMemo(
    () => ({
      nodes: overview.nodes.length,
      endpoints: overview.endpoints.length,
      placements: overview.placements.length,
      requests: overview.model_requests.length,
    }),
    [overview]
  )

  const refreshAll = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [o, r] = await Promise.all([
        apiGet<ClusterStatus>('/overview', token),
        apiGet<ModelRequest[]>('/requests', token),
      ])
      setOverview(o)
      setRequests(r)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load data')
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => { refreshAll() }, [refreshAll])

  // auto-refresh every 10s
  useEffect(() => {
    const id = setInterval(refreshAll, 10000)
    return () => clearInterval(id)
  }, [refreshAll])

  const handleLoadModel = async () => {
    setError(null)
    try {
      await apiPost('/models/load', form, token)
      setShowLoadForm(false)
      setForm({ model_name: '', model_uid: '', replicas: 1, config: { max_model_len: 4096 } })
      setSelectedGpu(null)
      await refreshAll()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load model')
    }
  }

  const handleUnload = async (id: string) => {
    setError(null)
    try {
      await apiDelete(`/models/requests/${id}`, token)
      await refreshAll()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to unload model')
    }
  }

  /* ── GPU usage map: nodeId -> Set<gpuIndex> ── */
  const usedGpus = useMemo(() => {
    const m = new Map<string, Set<number>>()
    for (const p of overview.placements) {
      for (const a of p.assignments) {
        if (a.gpu_index != null) {
          if (!m.has(a.node_id)) m.set(a.node_id, new Set())
          m.get(a.node_id)!.add(a.gpu_index)
        }
      }
    }
    return m
  }, [overview.placements])

  /* ── model running on a GPU ── */
  const gpuModel = (nodeId: string, gpuIdx: number) => {
    for (const p of overview.placements) {
      for (const a of p.assignments) {
        if (a.node_id === nodeId && a.gpu_index === gpuIdx) return p.model_uid
      }
    }
    return null
  }

  /* ── total GPU memory stats ── */
  const gpuStats = useMemo(() => {
    let total = 0, used = 0, count = 0
    for (const n of overview.nodes) {
      for (const g of n.gpus) {
        total += g.memory_total_mb
        used += g.memory_used_mb
        count++
      }
    }
    return { total, used, count }
  }, [overview.nodes])

  return (
    <div className="min-h-screen bg-background">
      <Sidebar page={page} onNavigate={setPage} />

      <main className="ml-56">
        {/* Top bar */}
        <header className="flex h-14 items-center justify-between px-8 border-b border-border">
          <div className="flex items-center gap-2 text-sm">
            <h2 className="text-lg font-semibold text-foreground">
              {page === 'dashboard' && 'Dashboard'}
              {page === 'models' && 'Model Management'}
              {page === 'nodes' && 'Nodes & GPUs'}
              {page === 'settings' && 'Settings'}
            </h2>
          </div>
          <div className="flex items-center gap-3">
            <Badge variant="outline" className={`font-medium ${error ? 'text-destructive border-destructive/30' : 'text-success border-success/30'}`}>
              {error ? 'Degraded' : 'Healthy'}
            </Badge>
            <Button variant="outline" size="sm" onClick={refreshAll} disabled={loading}>
              <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
              Refresh
            </Button>
          </div>
        </header>

        <div className="p-8 space-y-6">
          {error && (
            <div className="mb-4 flex items-center gap-2 rounded-lg border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm text-destructive">
              <TriangleAlert className="h-4 w-4 shrink-0" />
              {error}
            </div>
          )}

          {/* ═══ DASHBOARD ═══ */}
          {page === 'dashboard' && (
            <div className="space-y-6">
              {/* KPI cards */}
              <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                {[
                  { label: 'Total Nodes', value: counts.nodes, icon: Server, desc: 'Active compute nodes' },
                  { label: 'Endpoints', value: counts.endpoints, icon: Cpu, desc: 'Serving models' },
                  { label: 'GPU Utilization', value: gpuStats.count > 0 ? `${pct(gpuStats.used, gpuStats.total)}%` : '0%', icon: Activity, desc: `${gpuStats.count} GPUs total` },
                  { label: 'Requests', value: counts.requests, icon: Sparkles, desc: 'Model load requests' },
                ].map(({ label, value, icon: Icon, desc }) => (
                  <div key={label} className="bg-card border border-border rounded-xl p-5 flex flex-col justify-between min-h-[130px]">
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-medium text-muted-foreground">{label}</span>
                      <Icon className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <div>
                      <p className="text-3xl font-bold text-foreground">{value}</p>
                      <p className="text-sm text-muted-foreground mt-0.5">{desc}</p>
                    </div>
                  </div>
                ))}
              </div>

              {/* GPU overview + recent activity */}
              <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
                <div className="lg:col-span-3 bg-card border border-border rounded-xl p-5">
                  <h3 className="text-base font-semibold text-foreground">GPU Overview</h3>
                  <p className="text-sm text-muted-foreground mt-0.5 mb-4">Real-time GPU memory usage across all nodes</p>
                  {overview.nodes.length === 0 ? (
                    <p className="text-sm text-muted-foreground">No nodes reporting.</p>
                  ) : (
                    <div className="space-y-4">
                      {overview.nodes.map((node) => (
                        <div key={node.node_id}>
                          <div className="mb-2 flex items-center gap-2">
                            <Server className="h-4 w-4 text-muted-foreground" />
                            <span className="text-sm font-medium text-foreground">{node.node_id}</span>
                            <Badge variant="outline" className="text-xs">{node.gpus.length} GPUs</Badge>
                          </div>
                          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            {node.gpus.map((gpu) => {
                              const model = gpuModel(node.node_id, gpu.index)
                              const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                              const color = usage > 80 ? 'bg-destructive' : usage > 50 ? 'bg-warning' : 'bg-primary'
                              return (
                                <div key={gpu.index} className="border border-border rounded-lg p-4">
                                  <div className="flex items-center justify-between mb-2">
                                    <span className="text-sm font-medium text-foreground">GPU {gpu.index}</span>
                                    <span className="text-sm font-semibold text-foreground">{usage}%</span>
                                  </div>
                                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary mb-2">
                                    <div className={`h-full rounded-full ${color}`} style={{ width: `${usage}%` }} />
                                  </div>
                                  <p className="text-xs text-muted-foreground mb-2">
                                    {gpu.memory_used_mb.toLocaleString()} / {gpu.memory_total_mb.toLocaleString()} MB
                                  </p>
                                  {model && (
                                    <Badge variant="secondary" className="text-xs font-mono">{model}</Badge>
                                  )}
                                </div>
                              )
                            })}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                <div className="lg:col-span-2 bg-card border border-border rounded-xl p-5">
                  <h3 className="text-base font-semibold text-foreground">Active Endpoints</h3>
                  <p className="text-sm text-muted-foreground mt-0.5 mb-4">Currently serving model instances</p>
                  {overview.endpoints.length === 0 ? (
                    <p className="text-sm text-muted-foreground">No endpoints online.</p>
                  ) : (
                    <div className="space-y-2">
                      {overview.endpoints.map((ep) => (
                        <button
                          key={`${ep.model_uid}-${ep.replica_id}`}
                          className="w-full flex items-center justify-between border border-border rounded-lg p-4 hover:bg-accent/50 transition-colors text-left"
                        >
                          <div>
                            <div className="flex items-center gap-2 mb-1">
                              <span className="text-sm font-medium font-mono text-foreground">{ep.model_uid}</span>
                              <Badge className="bg-success/10 text-success border-success/20 text-xs hover:bg-success/10">{ep.status}</Badge>
                            </div>
                            <p className="text-xs text-muted-foreground">
                              {ep.node_id} · replica {ep.replica_id}
                            </p>
                          </div>
                          <ChevronRight className="h-4 w-4 text-muted-foreground" />
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* ═══ MODELS ═══ */}
          {page === 'models' && (
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="text-lg font-semibold">Model Management</h2>
                  <p className="text-sm text-muted-foreground">Load, monitor, and manage model deployments</p>
                </div>
                <Button onClick={() => setShowLoadForm(!showLoadForm)}>
                  <Plus className="h-4 w-4" />
                  Load Model
                </Button>
              </div>

              {/* Load form */}
              {showLoadForm && (
                <Card>
                  <CardHeader>
                    <CardTitle>Load New Model</CardTitle>
                    <CardDescription>Select a target GPU and configure model parameters</CardDescription>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    {/* GPU selector */}
                    <div>
                      <label className="mb-2 block text-sm font-medium">Target GPU</label>
                      <div className="space-y-3">
                        {overview.nodes.map((node) => (
                          <div key={node.node_id}>
                            <p className="mb-1.5 text-xs font-medium text-muted-foreground">{node.node_id}</p>
                            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                              {node.gpus.map((gpu) => {
                                const isUsed = usedGpus.get(node.node_id)?.has(gpu.index) ?? false
                                const isSel = selectedGpu?.nodeId === node.node_id && selectedGpu?.gpuIndex === gpu.index
                                return (
                                  <div key={gpu.index} className="relative">
                                    <GpuBar
                                      gpu={gpu}
                                      selected={isSel}
                                      onClick={() => setSelectedGpu({ nodeId: node.node_id, gpuIndex: gpu.index })}
                                    />
                                    {isUsed && (
                                      <Badge variant="warning" className="absolute -right-1 -top-1 text-[10px]">
                                        In use
                                      </Badge>
                                    )}
                                  </div>
                                )
                              })}
                            </div>
                          </div>
                        ))}
                      </div>
                      {selectedGpu && (
                        <p className="mt-2 text-xs text-accent">
                          Selected: {selectedGpu.nodeId} → GPU {selectedGpu.gpuIndex}
                        </p>
                      )}
                    </div>

                    {/* Form fields */}
                    <div className="grid gap-3 sm:grid-cols-2">
                      <div>
                        <label className="mb-1 block text-xs font-medium text-muted-foreground">Model Path</label>
                        <Input
                          placeholder="/DATA/Model/Qwen/Qwen2.5-0.5B-Instruct"
                          value={form.model_name}
                          onChange={(e) => setForm((p) => ({ ...p, model_name: e.target.value }))}
                        />
                      </div>
                      <div>
                        <label className="mb-1 block text-xs font-medium text-muted-foreground">Model UID</label>
                        <Input
                          placeholder="qwen2_5_0_5b"
                          value={form.model_uid}
                          onChange={(e) => setForm((p) => ({ ...p, model_uid: e.target.value }))}
                        />
                      </div>
                      <div>
                        <label className="mb-1 block text-xs font-medium text-muted-foreground">Replicas</label>
                        <Input
                          type="number"
                          value={form.replicas ?? 1}
                          onChange={(e) => setForm((p) => ({ ...p, replicas: Number(e.target.value || 1) }))}
                        />
                      </div>
                      <div>
                        <label className="mb-1 block text-xs font-medium text-muted-foreground">Max Model Length</label>
                        <Input
                          type="number"
                          value={form.config?.max_model_len ?? ''}
                          onChange={(e) =>
                            setForm((p) => ({
                              ...p,
                              config: { ...p.config, max_model_len: Number(e.target.value || 0) },
                            }))
                          }
                        />
                      </div>
                    </div>

                    <div className="flex gap-2">
                      <Button onClick={handleLoadModel}>Submit Request</Button>
                      <Button variant="outline" onClick={() => { setShowLoadForm(false); setSelectedGpu(null) }}>
                        Cancel
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              )}

              {/* Requests table */}
              <Card>
                <CardHeader>
                  <CardTitle>Model Requests</CardTitle>
                  <CardDescription>All model load/unload requests and their status</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="overflow-hidden rounded-lg border border-border">
                    <table className="w-full text-left text-sm">
                      <thead className="border-b border-border bg-muted/50">
                        <tr>
                          <th className="px-4 py-3 text-xs font-medium text-muted-foreground">Model</th>
                          <th className="px-4 py-3 text-xs font-medium text-muted-foreground">Status</th>
                          <th className="px-4 py-3 text-xs font-medium text-muted-foreground">Created</th>
                          <th className="px-4 py-3 text-xs font-medium text-muted-foreground">Config</th>
                          <th className="px-4 py-3 text-right text-xs font-medium text-muted-foreground">Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {requests.length === 0 ? (
                          <tr>
                            <td className="px-4 py-8 text-center text-muted-foreground" colSpan={5}>
                              No model requests yet. Click "Load Model" to get started.
                            </td>
                          </tr>
                        ) : (
                          requests.map((req) => (
                            <tr key={req.id} className="border-t border-border">
                              <td className="px-4 py-3">
                                <div className="font-medium text-foreground">{req.request.model_uid}</div>
                                <div className="text-xs text-muted-foreground">{req.request.model_name}</div>
                              </td>
                              <td className="px-4 py-3">
                                <Badge variant={statusVariant(req.status)}>{req.status}</Badge>
                              </td>
                              <td className="px-4 py-3 text-xs text-muted-foreground">{fmtTime(req.created_at_ms)}</td>
                              <td className="px-4 py-3 text-xs text-muted-foreground">
                                {req.request.config?.max_model_len && `len=${req.request.config.max_model_len}`}
                                {req.request.config?.gpu_memory_utilization && ` · mem=${req.request.config.gpu_memory_utilization}`}
                              </td>
                              <td className="px-4 py-3 text-right">
                                <Button variant="ghost" size="sm" onClick={() => handleUnload(req.id)}>
                                  <Trash2 className="h-3.5 w-3.5" />
                                  Unload
                                </Button>
                              </td>
                            </tr>
                          ))
                        )}
                      </tbody>
                    </table>
                  </div>
                </CardContent>
              </Card>

              {/* Placements */}
              {overview.placements.length > 0 && (
                <Card>
                  <CardHeader>
                    <CardTitle>Active Placements</CardTitle>
                    <CardDescription>Current model-to-GPU assignments</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                      {overview.placements.map((plan) => (
                        <div key={plan.model_uid} className="rounded-lg border border-border p-4">
                          <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">{plan.model_uid}</span>
                            <Badge variant="muted" className="text-[10px]">v{plan.version}</Badge>
                          </div>
                          <div className="mt-2 space-y-1">
                            {plan.assignments.map((a) => (
                              <div key={a.replica_id} className="flex items-center gap-2 text-xs text-muted-foreground">
                                <div className="h-1.5 w-1.5 rounded-full bg-success" />
                                {a.node_id} · GPU {a.gpu_index ?? 'auto'} · port {a.port}
                              </div>
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  </CardContent>
                </Card>
              )}
            </div>
          )}

          {/* ═══ NODES ═══ */}
          {page === 'nodes' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-lg font-semibold">Nodes & GPUs</h2>
                <p className="text-sm text-muted-foreground">Monitor compute infrastructure and GPU resources</p>
              </div>

              {overview.nodes.length === 0 ? (
                <Card>
                  <CardContent className="py-12 text-center text-muted-foreground">
                    No nodes reporting. Ensure nebula-node is running.
                  </CardContent>
                </Card>
              ) : (
                overview.nodes.map((node) => (
                  <Card key={node.node_id}>
                    <CardHeader>
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10">
                            <Server className="h-4 w-4 text-primary" />
                          </div>
                          <div>
                            <CardTitle className="text-base">{node.node_id}</CardTitle>
                            <CardDescription>Last heartbeat: {fmtTime(node.last_heartbeat_ms)}</CardDescription>
                          </div>
                        </div>
                        <Badge variant="success">Online</Badge>
                      </div>
                    </CardHeader>
                    <CardContent>
                      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                        {node.gpus.map((gpu) => {
                          const model = gpuModel(node.node_id, gpu.index)
                          const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                          const color = usage > 80 ? 'bg-destructive' : usage > 50 ? 'bg-warning' : 'bg-success'
                          return (
                            <div key={gpu.index} className="border border-border rounded-lg p-4">
                              <div className="flex items-center justify-between mb-2">
                                <div className="flex items-center gap-2">
                                  <Cpu className="h-3.5 w-3.5 text-muted-foreground" />
                                  <span className="text-sm font-medium text-foreground">GPU {gpu.index}</span>
                                </div>
                                <span className={`text-sm font-semibold ${usage > 80 ? 'text-destructive' : usage > 50 ? 'text-warning' : 'text-success'}`}>
                                  {usage}%
                                </span>
                              </div>
                              <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary mb-2">
                                <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${usage}%` }} />
                              </div>
                              <p className="text-xs text-muted-foreground mb-2">
                                {gpu.memory_used_mb.toLocaleString()} / {gpu.memory_total_mb.toLocaleString()} MB
                              </p>
                              {model ? (
                                <Badge variant="secondary" className="text-xs font-mono">{model}</Badge>
                              ) : (
                                <span className="text-xs text-muted-foreground">Available</span>
                              )}
                            </div>
                          )
                        })}
                      </div>
                    </CardContent>
                  </Card>
                ))
              )}
            </div>
          )}

          {/* ═══ SETTINGS ═══ */}
          {page === 'settings' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-lg font-semibold">Settings</h2>
                <p className="text-sm text-muted-foreground">Configure authentication and preferences</p>
              </div>
              <Card>
                <CardHeader>
                  <CardTitle>Authentication</CardTitle>
                  <CardDescription>Configure your BFF access token for API authentication</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div>
                    <label className="mb-1 block text-xs font-medium text-muted-foreground">Bearer Token</label>
                    <Input
                      type="password"
                      placeholder="Enter your access token"
                      value={token}
                      onChange={(e) => setToken(e.target.value)}
                    />
                  </div>
                  <Button
                    onClick={() => {
                      localStorage.setItem('nebula_token', token)
                      refreshAll()
                    }}
                  >
                    Save & Apply
                  </Button>
                </CardContent>
              </Card>
            </div>
          )}
        </div>
      </main >
    </div >
  )
}

export default App
