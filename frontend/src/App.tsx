import { useCallback, useEffect, useMemo, useState } from 'react'
import { Bell, Search, User, Settings, LogOut } from 'lucide-react'
import { apiDelete, apiGet, apiPost, authApi } from '@/lib/api'
import type { AuthUser, ClusterStatus, EndpointStats, ModelLoadRequest, ModelRequest } from '@/lib/types'
import { useI18n } from '@/lib/i18n'

// Components
import Sidebar from '@/components/Sidebar'
import { LoadModelDialog } from '@/components/LoadModelDialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

// Views
import { DashboardView } from '@/components/views/dashboard'
import { ModelsView } from '@/components/views/models'
import { ModelDetailView_Page } from '@/components/views/model-detail'
import { NodesView } from '@/components/views/nodes'
import { SettingsView } from '@/components/views/settings'
import { InferenceView } from '@/components/views/inference'
import { EndpointsView } from '@/components/views/endpoints'
import { GatewayView } from '@/components/views/gateway'
import { AuditView } from '@/components/views/audit'
import { ImagesView } from '@/components/views/images'
import { TemplatesView } from '@/components/views/templates'
import { ModelCatalogView } from '@/components/views/model-catalog'
import { ModelLibraryView } from '@/components/views/model-library'
import { UserProfileView } from '@/components/views/user-profile'
import { AccountSettingsView } from '@/components/views/account-settings'
import { LoginView } from '@/components/views/login'

const EMPTY_OVERVIEW: ClusterStatus = {
  nodes: [],
  endpoints: [],
  placements: [],
  model_requests: [],
}

type Page = 'dashboard' | 'models' | 'model-detail' | 'model-catalog' | 'model-library' | 'nodes' | 'settings' | 'inference' | 'gateway' | 'endpoints' | 'audit' | 'images' | 'templates' | 'profile' | 'account-settings'

const PAGE_PATH: Record<Page, string> = {
  dashboard: '/',
  models: '/models',
  'model-detail': '/models/detail',
  'model-catalog': '/resources/model-catalog',
  'model-library': '/resources/model-library',
  nodes: '/infrastructure/nodes',
  settings: '/system/settings',
  inference: '/inference',
  gateway: '/inference/gateway',
  endpoints: '/endpoints',
  audit: '/resources/audit',
  images: '/infrastructure/images',
  templates: '/infrastructure/templates',
  profile: '/system/profile',
  'account-settings': '/system/account-settings',
}

const normalizePath = (pathname: string) => {
  if (!pathname || pathname === '/') return '/'
  return pathname.replace(/\/+$/, '')
}

const readRouteFromLocation = (): { page: Page; modelUid: string | null } => {
  const path = normalizePath(window.location.pathname)
  const params = new URLSearchParams(window.location.search)

  if (path === '/models/detail') {
    const modelUid = params.get('uid')
    return modelUid ? { page: 'model-detail', modelUid } : { page: 'models', modelUid: null }
  }

  const byPath = (Object.entries(PAGE_PATH) as [Page, string][]).find(([, routePath]) => routePath === path)
  if (byPath) return { page: byPath[0], modelUid: null }

  return { page: 'dashboard', modelUid: null }
}

const fmtTime = (v: number, fallback: string) => (v ? new Date(v).toLocaleString() : fallback)

const pct = (used: number, total: number) =>
  total > 0 ? Math.round((used / total) * 100) : 0

function App() {
  const { t, locale, setLocale } = useI18n()
  const initialRoute = readRouteFromLocation()
  const [token, setToken] = useState(() => localStorage.getItem('nebula_token') || '')
  const [currentUser, setCurrentUser] = useState<AuthUser | null>(null)
  const [authReady, setAuthReady] = useState(false)
  const [overview, setOverview] = useState<ClusterStatus>(EMPTY_OVERVIEW)
  const [, setRequests] = useState<ModelRequest[]>([])
  const [, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState<Page>(initialRoute.page)
  const [showLoadDialog, setShowLoadDialog] = useState(false)
  const [selectedModelUid, setSelectedModelUid] = useState<string | null>(initialRoute.modelUid)
  const [metricsRaw, setMetricsRaw] = useState('')
  const [engineStats, setEngineStats] = useState<EndpointStats[]>([])

  const navigateToPage = useCallback((nextPage: Page, options?: { modelUid?: string | null; replace?: boolean }) => {
    const nextModelUid = options?.modelUid ?? null
    const nextPath = PAGE_PATH[nextPage]
    const nextUrl =
      nextPage === 'model-detail' && nextModelUid
        ? `${nextPath}?uid=${encodeURIComponent(nextModelUid)}`
        : nextPath

    setPage(nextPage)
    setSelectedModelUid(nextPage === 'model-detail' ? nextModelUid : null)

    const currentPath = normalizePath(window.location.pathname)
    const currentQuery = window.location.search
    const [nextUrlPath, nextUrlQuery = ''] = nextUrl.split('?')
    const targetQuery = nextUrlQuery ? `?${nextUrlQuery}` : ''
    const isSameRoute = currentPath === normalizePath(nextUrlPath) && currentQuery === targetQuery

    if (!isSameRoute) {
      if (options?.replace) {
        window.history.replaceState(null, '', nextUrl)
      } else {
        window.history.pushState(null, '', nextUrl)
      }
    }
  }, [])

  useEffect(() => {
    const onPopState = () => {
      const route = readRouteFromLocation()
      setPage(route.page)
      setSelectedModelUid(route.modelUid)
    }
    window.addEventListener('popstate', onPopState)
    return () => window.removeEventListener('popstate', onPopState)
  }, [])

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
    if (!token) return
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
      setError(err instanceof Error ? err.message : t('models.failedLoad'))
    } finally {
      setLoading(false)
    }
  }, [token, t])

  useEffect(() => {
    if (token) {
      refreshAll()
    }
  }, [refreshAll, token])

  useEffect(() => {
    if (!token) return
    const id = setInterval(refreshAll, 10000)
    return () => clearInterval(id)
  }, [refreshAll, token])

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
      setError(err instanceof Error ? err.message : t('models.failedLoad'))
      throw err
    }
  }, [token, refreshAll, t])

  const handleUnload = async (id: string) => {
    setError(null)
    try {
      await apiDelete(`/models/requests/${id}`, token)
      await refreshAll()
    } catch (err) {
      setError(err instanceof Error ? err.message : t('models.actionFailed'))
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

  const alertCount = useMemo(() => {
    const endpointAlerts = overview.endpoints.filter((ep) => {
      const status = (ep.status || '').toLowerCase()
      if (!status) return false
      return !status.includes('ready') && !status.includes('running')
    }).length
    return endpointAlerts + (error ? 1 : 0)
  }, [overview.endpoints, error])

  const handleLogout = useCallback(async () => {
    try {
      if (token) await authApi.logout(token)
    } catch {
      // ignore logout API errors and clear local session anyway
    }
    setToken('')
    setCurrentUser(null)
    localStorage.removeItem('nebula_token')
    navigateToPage('dashboard', { replace: true })
  }, [navigateToPage, token])

  const refreshCurrentUser = useCallback(async () => {
    if (!token) {
      setCurrentUser(null)
      return
    }
    const me = await authApi.me(token)
    setCurrentUser(me)
  }, [token])

  useEffect(() => {
    let cancelled = false
    const verifySession = async () => {
      if (!token) {
        if (!cancelled) {
          setCurrentUser(null)
          setAuthReady(true)
        }
        return
      }

      try {
        const me = await authApi.me(token)
        if (!cancelled) {
          setCurrentUser(me)
          setAuthReady(true)
        }
      } catch {
        if (!cancelled) {
          setToken('')
          setCurrentUser(null)
          localStorage.removeItem('nebula_token')
          setAuthReady(true)
        }
      }
    }
    void verifySession()
    return () => {
      cancelled = true
    }
  }, [token])

  if (!authReady) {
    return (
      <div className="min-h-screen w-full bg-background flex items-center justify-center text-sm text-muted-foreground">
        {t('app.loadingSession')}
      </div>
    )
  }

  if (!token || !currentUser) {
    return (
      <LoginView
        onLoginSuccess={(nextToken, user) => {
          setToken(nextToken)
          setCurrentUser(user)
          localStorage.setItem('nebula_token', nextToken)
          setAuthReady(true)
          navigateToPage('dashboard', { replace: true })
        }}
      />
    )
  }

  return (
    <div className="flex min-h-screen w-full bg-background font-sans">
      <Sidebar
        page={page}
        setPage={(p) => navigateToPage(p as Page)}
        clusterHealthy={!error && overview.nodes.length > 0}
      />

        <main className="flex-1 min-w-0 p-4 lg:p-5 space-y-3 bg-background">
          <header className="rounded-2xl bg-background px-1 py-1 flex items-center justify-end gap-1.5">
              <Button variant="ghost" size="icon" className="h-8 w-8 rounded-lg bg-card relative">
                <Bell className="h-3.5 w-3.5 text-muted-foreground" />
                {alertCount > 0 && (
                  <span className="absolute -right-1 -top-1 h-4 min-w-4 px-1 rounded-full bg-destructive text-destructive-foreground text-[10px] leading-4">
                    {alertCount > 99 ? '99+' : alertCount}
                  </span>
                )}
              </Button>
              <div className="relative w-64">
                <Search className="h-3.5 w-3.5 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
                <Input
                  placeholder={t('app.searchPlaceholder')}
                  className="h-8 rounded-lg pl-8 border-border/60 bg-card"
                />
              </div>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-8 w-8 rounded-lg bg-card">
                    <User className="h-3.5 w-3.5 text-muted-foreground" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-44">
                  <DropdownMenuLabel>{t('app.account')}</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => setLocale(locale === 'zh' ? 'en' : 'zh')}>
                    <User className="h-4 w-4" /> {t('lang.switch')}: {locale === 'zh' ? t('lang.zh') : t('lang.en')}
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => navigateToPage('profile')}>
                    <User className="h-4 w-4" /> {t('app.editProfile')}
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => navigateToPage('account-settings')}>
                    <Settings className="h-4 w-4" /> {t('app.accountSettings')}
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem variant="destructive" onClick={handleLogout}>
                    <LogOut className="h-4 w-4" /> {t('app.logout')}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
          </header>

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
              token={token}
              onOpenLoadDialog={() => setShowLoadDialog(true)}
              onNavigate={(p) => navigateToPage(p as Page)}
              onSelectModel={(uid) => {
                navigateToPage('model-detail', { modelUid: uid })
              }}
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
        {page === 'model-detail' && selectedModelUid && (
          <ModelDetailView_Page
            modelUid={selectedModelUid}
            token={token}
            onBack={() => {
              navigateToPage('models')
            }}
          />
        )}
        {page === 'nodes' && (
          <NodesView
            overview={overview}
            gpuModel={gpuModel}
            pct={pct}
            fmtTime={(v) => fmtTime(v, t('common.n_a'))}
          />
        )}
        {page === 'settings' && (
          <SettingsView
            token={token}
            setToken={setToken}
            onSaveToken={() => {
              refreshAll()
            }}
          />
        )}
        {page === 'profile' && (
          <UserProfileView token={token} user={currentUser} onProfileUpdated={refreshCurrentUser} />
        )}
        {page === 'account-settings' && (
          <AccountSettingsView
            token={token}
            user={currentUser}
            onOpenSecuritySettings={() => navigateToPage('settings')}
          />
        )}
        {page === 'inference' && (
          <InferenceView overview={overview} metricsRaw={metricsRaw} engineStats={engineStats} />
        )}
        {page === 'gateway' && (
          <GatewayView token={token} />
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
        {page === 'templates' && (
          <TemplatesView token={token} />
        )}
        {page === 'model-catalog' && (
          <ModelCatalogView
            token={token}
            onOpenModels={() => navigateToPage('model-library')}
            onSelectModel={(uid) => {
              navigateToPage('model-detail', { modelUid: uid })
            }}
          />
        )}
        {page === 'model-library' && (
          <ModelLibraryView
            token={token}
            onOpenService={(uid) => {
              navigateToPage('model-detail', { modelUid: uid })
            }}
          />
        )}
      </main>
    </div>
  )
}

export default App
