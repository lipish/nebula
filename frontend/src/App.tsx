import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  RefreshCw,
} from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { apiDelete, apiGet, apiPost } from '@/lib/api'
import type { ClusterStatus, ModelLoadRequest, ModelRequest } from '@/lib/types'

// Components
import Sidebar from '@/components/Sidebar'

// Views
import { DashboardView } from '@/components/views/dashboard'
import { ModelsView } from '@/components/views/models'
import { NodesView } from '@/components/views/nodes'
import { SettingsView } from '@/components/views/settings'

const EMPTY_OVERVIEW: ClusterStatus = {
  nodes: [],
  endpoints: [],
  placements: [],
  model_requests: [],
}

type Page = 'dashboard' | 'models' | 'nodes' | 'settings'

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

  useEffect(() => {
    const id = setInterval(refreshAll, 10000)
    return () => clearInterval(id)
  }, [refreshAll])

  const handleLoadModel = async () => {
    setError(null)
    if (!selectedGpu) {
      setError('Please select a target GPU')
      return
    }
    try {
      await apiPost('/models/load', {
        ...form,
        node_id: selectedGpu.nodeId,
        gpu_index: selectedGpu.gpuIndex
      }, token)
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

      <div className="flex flex-1 flex-col ml-64 min-w-0 transition-all duration-300">
        <header className="sticky top-0 z-20 flex h-20 items-center justify-between border-b border-border/60 bg-background/80 backdrop-blur-md px-10 shadow-[0_1px_2px_rgba(0,0,0,0.03)]">
          <div className="flex items-center gap-2">
            <h1 className="text-xl font-bold tracking-tight text-foreground uppercase text-[15px] opacity-80">
              {page}
            </h1>
          </div>
          <div className="flex items-center gap-6">
            <Badge
              className={`font-bold px-3 py-1 border-0 shadow-sm ${error ? 'bg-destructive/10 text-destructive' : 'bg-success/10 text-success'
                }`}
            >
              {error ? 'SYSTEM OFFLINE' : 'CLUSTER HEALTHY'}
            </Badge>
            <Separator orientation="vertical" className="h-4" />
            <Button
              variant="ghost"
              size="icon"
              onClick={refreshAll}
              disabled={loading}
              className="rounded-xl hover:bg-accent/50 transition-all h-10 w-10 active:scale-95"
            >
              <RefreshCw className={`h-4 w-4 text-muted-foreground ${loading ? 'animate-spin' : ''}`} />
              <span className="sr-only">Refresh</span>
            </Button>
          </div>
        </header>

        <main className="flex-1 p-8 md:p-10 max-w-[1600px] w-full mx-auto">
          {page === 'dashboard' && (
            <DashboardView
              overview={overview}
              counts={counts}
              gpuStats={gpuStats}
              gpuModel={gpuModel}
              pct={pct}
            />
          )}
          {page === 'models' && (
            <ModelsView
              overview={overview}
              requests={requests}
              showLoadForm={showLoadForm}
              setShowLoadForm={setShowLoadForm}
              form={form}
              setForm={setForm}
              handleLoadModel={handleLoadModel}
              handleUnload={handleUnload}
              selectedGpu={selectedGpu}
              setSelectedGpu={setSelectedGpu}
              usedGpus={usedGpus}
              statusVariant={statusVariant}
              fmtTime={fmtTime}
              pct={pct}
            />
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
        </main>
      </div>
    </div>
  )
}

export default App
