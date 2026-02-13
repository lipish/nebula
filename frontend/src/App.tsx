import { useCallback, useEffect, useMemo, useState } from 'react'
import { apiDelete, apiGet, apiPost } from '@/lib/api'
import type { ClusterStatus, EndpointStats, ModelLoadRequest, ModelRequest } from '@/lib/types'

// Components
import Sidebar from '@/components/Sidebar'
import { LoadModelDialog } from '@/components/LoadModelDialog'

// Views
import { DashboardView } from '@/components/views/dashboard'
import { ModelsView } from '@/components/views/models'
import { NodesView } from '@/components/views/nodes'
import { SettingsView } from '@/components/views/settings'
import { InferenceView } from '@/components/views/inference'
import { EndpointsView } from '@/components/views/endpoints'
import { AuditView } from '@/components/views/audit'
import { ImagesView } from '@/components/views/images'

const EMPTY_OVERVIEW: ClusterStatus = {
  nodes: [],
  endpoints: [],
  placements: [],
  model_requests: [],
}

type Page = 'dashboard' | 'models' | 'nodes' | 'settings' | 'inference' | 'endpoints' | 'audit' | 'images'

const statusVariant = (s: string): 'default' | 'outline' | 'secondary' | 'destructive' => {
  const n = s.toLowerCase()
  if (n.includes('fail')) return 'destructive'
  if (n.includes('unload') || n.includes('loading')) return 'outline'
  if (n.includes('run') || n.includes('ready')) return 'default'
  return 'secondary'
}

const fmtTime = (v: number) => (v ? new Date(v).toLocaleString() : 'n/a')

const pct = (used: number, total: number) =>
  total > 0 ? Math.round((used / total) * 100) : 0

function App() {
  const [token, setToken] = useState(() => localStorage.getItem('nebula_token') || '')
  const [overview, setOverview] = useState<ClusterStatus>(EMPTY_OVERVIEW)
  const [requests, setRequests] = useState<ModelRequest[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState<Page>('dashboard')
  const [showLoadDialog, setShowLoadDialog] = useState(false)
  const [metricsRaw, setMetricsRaw] = useState('')
  const [engineStats, setEngineStats] = useState<EndpointStats[]>([])

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
      // Fetch metrics and engine stats (best-effort)
      try {
        const BASE_URL = import.meta.env.VITE_BFF_BASE_URL || '/api'
        const [mResp, sResp] = await Promise.all([
          fetch(`${BASE_URL}/metrics`, {
            headers: token ? { Authorization: `Bearer ${token}` } : {},
          }),
          fetch(`${BASE_URL}/engine-stats`, {
            headers: token ? { Authorization: `Bearer ${token}` } : {},
          }),
        ])
        if (mResp.ok) setMetricsRaw(await mResp.text())
        if (sResp.ok) setEngineStats(await sResp.json())
      } catch { /* metrics are optional */ }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load data')
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => { refreshAll() }, [refreshAll])

  useEffect(() => {
    const id = setInterval(refreshAll, 10000)
    return () => clearInterval(id)
  }, [refreshAll])

  const handleLoadModel = useCallback(async (form: ModelLoadRequest, gpu: { nodeId: string; gpuIndices: number[] }) => {
    setError(null)
    try {
      await apiPost('/models/load', {
        ...form,
        node_id: gpu.nodeId,
        gpu_index: gpu.gpuIndices[0] ?? undefined,
        gpu_indices: gpu.gpuIndices,
      }, token)
      await refreshAll()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load model')
      throw err
    }
  }, [token, refreshAll])

  const handleUnload = async (id: string) => {
    setError(null)
    try {
      await apiDelete(`/models/requests/${id}`, token)
      await refreshAll()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to unload model')
    }
  }

  const usedGpus = useMemo(() => {
    const m = new Map<string, Set<number>>()
    for (const p of overview.placements) {
      for (const a of p.assignments) {
        if (!m.has(a.node_id)) m.set(a.node_id, new Set())
        const set = m.get(a.node_id)!
        if (a.gpu_index != null) set.add(a.gpu_index)
        if (Array.isArray(a.gpu_indices)) {
          for (const idx of a.gpu_indices) {
            if (typeof idx === 'number') set.add(idx)
          }
        }
      }
    }
    return m
  }, [overview.placements])

  const gpuModel = (nodeId: string, gpuIdx: number) => {
    for (const p of overview.placements) {
      for (const a of p.assignments) {
        if (a.node_id === nodeId && a.gpu_index === gpuIdx) return p.model_uid
      }
    }
    return null
  }

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
    <div className="flex min-h-screen w-full bg-background font-sans">
      <Sidebar
        page={page}
        setPage={(p) => setPage(p)}
        clusterHealthy={!error && overview.nodes.length > 0}
      />

      <main className="ml-64 p-8 flex-1 min-w-0">
        {page === 'dashboard' && (
          <DashboardView
            overview={overview}
            counts={counts}
            gpuStats={gpuStats}
            pct={pct}
            engineStats={engineStats}
            token={token}
          />
        )}
        {page === 'models' && (
          <>
            <ModelsView
              requests={requests}
              endpoints={overview.endpoints}
              onOpenLoadDialog={() => setShowLoadDialog(true)}
              handleUnload={handleUnload}
              fmtTime={fmtTime}
            />
            <LoadModelDialog
              open={showLoadDialog}
              onOpenChange={setShowLoadDialog}
              overview={overview}
              usedGpus={usedGpus}
              pct={pct}
              token={token}
              onSubmit={handleLoadModel}
              onUnloadRequestId={handleUnload}
            />
          </>
        )}
        {page === 'nodes' && (
          <NodesView
            overview={overview}
            gpuModel={gpuModel}
            pct={pct}
            fmtTime={fmtTime}
          />
        )}
        {page === 'settings' && (
          <SettingsView
            token={token}
            setToken={setToken}
            onSaveToken={() => {
              localStorage.setItem('nebula_token', token)
              refreshAll()
            }}
          />
        )}
        {page === 'inference' && (
          <InferenceView overview={overview} metricsRaw={metricsRaw} engineStats={engineStats} />
        )}
        {page === 'endpoints' && (
          <EndpointsView overview={overview} pct={pct} engineStats={engineStats} />
        )}
        {page === 'audit' && (
          <AuditView token={token} />
        )}
        {page === 'images' && (
          <ImagesView token={token} />
        )}
      </main>
    </div>
  )
}

export default App
