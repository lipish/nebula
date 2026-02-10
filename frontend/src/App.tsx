import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Activity,
  Boxes,
  Cpu,
  RefreshCw,
  Server,
  Sparkles,
  Trash2,
  TriangleAlert,
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { apiDelete, apiGet, apiPost } from '@/lib/api'
import type { ClusterStatus, ModelLoadRequest, ModelRequest } from '@/lib/types'

const EMPTY_OVERVIEW: ClusterStatus = {
  nodes: [],
  endpoints: [],
  placements: [],
  model_requests: [],
}

function App() {
  const [token, setToken] = useState(() => localStorage.getItem('nebula_token') || '')
  const [overview, setOverview] = useState<ClusterStatus>(EMPTY_OVERVIEW)
  const [requests, setRequests] = useState<ModelRequest[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
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
      const [nextOverview, nextRequests] = await Promise.all([
        apiGet<ClusterStatus>('/overview', token),
        apiGet<ModelRequest[]>('/requests', token),
      ])
      setOverview(nextOverview)
      setRequests(nextRequests)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load data')
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    refreshAll()
  }, [refreshAll])

  const handleSaveToken = () => {
    localStorage.setItem('nebula_token', token)
  }

  const handleLoadModel = async () => {
    setError(null)
    try {
      await apiPost('/models/load', form, token)
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

  const statusBadge = (
    status: string
  ): 'danger' | 'warning' | 'success' | 'muted' => {
    const normalized = status.toLowerCase()
    if (normalized.includes('fail')) return 'danger'
    if (normalized.includes('unload')) return 'warning'
    if (normalized.includes('run') || normalized.includes('ready')) return 'success'
    return 'muted'
  }

  const formatTime = (value: number) => {
    if (!value) return 'n/a'
    return new Date(value).toLocaleString()
  }

  return (
    <div className="min-h-screen px-6 py-10 text-ink">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-8">
        <header className="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
          <div className="space-y-2">
            <div className="data-label">Nebula Admin</div>
            <h1 className="font-display text-3xl font-semibold">Control & Observability</h1>
            <p className="max-w-xl text-sm text-muted">
              BFF-driven console for cluster awareness, model lifecycle control, and routing health
              signals.
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Badge variant="default" className="rounded-full px-4">
              BFF Connected
            </Badge>
            <Button variant="outline" size="sm" onClick={refreshAll}>
              <RefreshCw className="h-4 w-4" />
              Refresh
            </Button>
          </div>
        </header>

        <section className="glass-panel p-6">
          <div className="grid gap-4 lg:grid-cols-[1.2fr_1fr_auto] lg:items-center">
            <div className="space-y-2">
              <div className="data-label">Access Token</div>
              <Input
                placeholder="Bearer token"
                value={token}
                onChange={(event) => setToken(event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <div className="data-label">Last Status</div>
              <div className="flex items-center gap-2 text-sm">
                <Activity className="h-4 w-4 text-accent" />
                {loading ? 'Loading…' : error ? 'Degraded' : 'Healthy'}
                {error && (
                  <Badge variant="warning" className="ml-2">
                    Attention
                  </Badge>
                )}
              </div>
            </div>
            <Button variant="ghost" size="sm" onClick={handleSaveToken}>
              Save Token
            </Button>
          </div>
          {error && (
            <div className="mt-4 flex items-center gap-2 rounded-xl border border-warning/30 bg-warning/10 px-4 py-3 text-sm text-warning">
              <TriangleAlert className="h-4 w-4" />
              {error}
            </div>
          )}
        </section>

        <section className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          {[
            { label: 'Nodes', value: counts.nodes, icon: Server },
            { label: 'Endpoints', value: counts.endpoints, icon: Cpu },
            { label: 'Placements', value: counts.placements, icon: Boxes },
            { label: 'Requests', value: counts.requests, icon: Sparkles },
          ].map(({ label, value, icon: Icon }) => (
            <Card key={label} className="bg-white/85">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle>{label}</CardTitle>
                  <Icon className="h-5 w-5 text-accent" />
                </div>
                <CardDescription>Active footprint</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="text-3xl font-semibold text-ink">{value}</div>
              </CardContent>
            </Card>
          ))}
        </section>

        <section className="grid gap-4 lg:grid-cols-[1.1fr_1fr]">
          <Card>
            <CardHeader>
              <CardTitle>Model Intake</CardTitle>
              <CardDescription>Submit a load request into the scheduler queue.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <Input
                  placeholder="Model name"
                  value={form.model_name}
                  onChange={(event) =>
                    setForm((prev) => ({ ...prev, model_name: event.target.value }))
                  }
                />
                <Input
                  placeholder="Model UID"
                  value={form.model_uid}
                  onChange={(event) =>
                    setForm((prev) => ({ ...prev, model_uid: event.target.value }))
                  }
                />
                <Input
                  type="number"
                  placeholder="Replicas"
                  value={form.replicas ?? 1}
                  onChange={(event) =>
                    setForm((prev) => ({
                      ...prev,
                      replicas: Number(event.target.value || 1),
                    }))
                  }
                />
                <Input
                  type="number"
                  placeholder="Max model len"
                  value={form.config?.max_model_len ?? ''}
                  onChange={(event) =>
                    setForm((prev) => ({
                      ...prev,
                      config: {
                        ...prev.config,
                        max_model_len: Number(event.target.value || 0),
                      },
                    }))
                  }
                />
              </div>
              <Button onClick={handleLoadModel}>Submit Request</Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Placement Signals</CardTitle>
              <CardDescription>Snapshot of the latest placement versions.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {overview.placements.length === 0 ? (
                <div className="text-sm text-muted">No placements available.</div>
              ) : (
                overview.placements.slice(0, 4).map((plan) => (
                  <div
                    key={plan.model_uid}
                    className="flex items-center justify-between rounded-xl border border-stroke bg-white/60 px-3 py-2"
                  >
                    <div>
                      <div className="text-sm font-semibold text-ink">{plan.model_uid}</div>
                      <div className="text-xs text-muted">
                        {plan.assignments.length} replicas
                      </div>
                    </div>
                    <Badge variant="muted">v{plan.version}</Badge>
                  </div>
                ))
              )}
            </CardContent>
          </Card>
        </section>

        <section className="glass-panel p-6">
          <div className="flex items-center justify-between">
            <div>
              <div className="data-label">Fleet Data</div>
              <h2 className="section-title">Requests & Endpoints</h2>
            </div>
          </div>
          <Tabs defaultValue="requests" className="mt-6">
            <TabsList>
              <TabsTrigger value="requests">Requests</TabsTrigger>
              <TabsTrigger value="endpoints">Endpoints</TabsTrigger>
              <TabsTrigger value="nodes">Nodes</TabsTrigger>
            </TabsList>

            <TabsContent value="requests">
              <div className="mt-4 overflow-hidden rounded-2xl border border-stroke bg-white/70">
                <table className="w-full text-left text-sm">
                  <thead className="bg-white/80 text-xs uppercase tracking-[0.2em] text-muted">
                    <tr>
                      <th className="px-4 py-3">Model</th>
                      <th className="px-4 py-3">Status</th>
                      <th className="px-4 py-3">Created</th>
                      <th className="px-4 py-3"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {requests.length === 0 ? (
                      <tr>
                        <td className="px-4 py-4 text-muted" colSpan={4}>
                          No requests found.
                        </td>
                      </tr>
                    ) : (
                      requests.map((req) => (
                        <tr key={req.id} className="border-t border-stroke/60">
                          <td className="px-4 py-3">
                            <div className="font-semibold text-ink">
                              {req.request.model_uid}
                            </div>
                            <div className="text-xs text-muted">{req.request.model_name}</div>
                          </td>
                          <td className="px-4 py-3">
                            <Badge variant={statusBadge(req.status)}>{req.status}</Badge>
                          </td>
                          <td className="px-4 py-3 text-muted">
                            {formatTime(req.created_at_ms)}
                          </td>
                          <td className="px-4 py-3 text-right">
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => handleUnload(req.id)}
                            >
                              <Trash2 className="h-4 w-4" />
                              Unload
                            </Button>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            </TabsContent>

            <TabsContent value="endpoints">
              <div className="mt-4 grid gap-3">
                {overview.endpoints.length === 0 ? (
                  <div className="text-sm text-muted">No endpoints online.</div>
                ) : (
                  overview.endpoints.map((endpoint) => (
                    <div
                      key={`${endpoint.model_uid}-${endpoint.replica_id}`}
                      className="flex items-center justify-between rounded-2xl border border-stroke bg-white/70 px-4 py-3"
                    >
                      <div>
                        <div className="text-sm font-semibold text-ink">
                          {endpoint.model_uid}
                        </div>
                        <div className="text-xs text-muted">
                          {endpoint.node_id} · replica {endpoint.replica_id}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge variant={statusBadge(endpoint.status)}>{endpoint.status}</Badge>
                        {endpoint.base_url && (
                          <Badge variant="muted">{endpoint.base_url}</Badge>
                        )}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </TabsContent>

            <TabsContent value="nodes">
              <div className="mt-4 grid gap-3">
                {overview.nodes.length === 0 ? (
                  <div className="text-sm text-muted">No nodes reporting.</div>
                ) : (
                  overview.nodes.map((node) => (
                    <div
                      key={node.node_id}
                      className="rounded-2xl border border-stroke bg-white/70 px-4 py-3"
                    >
                      <div className="flex items-center justify-between">
                        <div>
                          <div className="text-sm font-semibold text-ink">{node.node_id}</div>
                          <div className="text-xs text-muted">
                            Last heartbeat: {formatTime(node.last_heartbeat_ms)}
                          </div>
                        </div>
                        <Badge variant="muted">{node.gpus.length} GPUs</Badge>
                      </div>
                      <div className="mt-2 grid gap-2 sm:grid-cols-2">
                        {node.gpus.map((gpu) => (
                          <div
                            key={`${node.node_id}-${gpu.index}`}
                            className="rounded-xl border border-stroke/60 bg-white/60 px-3 py-2 text-xs text-muted"
                          >
                            GPU {gpu.index} · {gpu.memory_used_mb}/{gpu.memory_total_mb} MB
                          </div>
                        ))}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </TabsContent>
          </Tabs>
        </section>
      </div>
    </div>
  )
}

export default App
