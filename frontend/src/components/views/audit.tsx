import { useCallback, useEffect, useState } from "react"
import { Shield, RefreshCw, ChevronLeft, ChevronRight, Search } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { useI18n } from "@/lib/i18n"

interface AuditTrace {
  id: string
  timestamp: string
  name: string | null
  input: Record<string, unknown> | null
  output: Record<string, unknown> | null
  metadata: Record<string, unknown> | null
  tags: string[]
  userId: string | null
  latency: number | null
}

interface AuditPageMeta {
  page: number
  limit: number
  totalItems: number
  totalPages: number
}

interface AuditResponse {
  data: AuditTrace[]
  meta: AuditPageMeta
}

interface AuditViewProps {
  token: string
}

const BASE_URL = import.meta.env.VITE_BFF_BASE_URL || "/api"

const statusColor = (code: number | undefined) => {
  if (!code) return "secondary"
  if (code >= 500) return "destructive"
  if (code >= 400) return "outline"
  return "default"
}

const fmtTs = (iso: string) => {
  try {
    return new Date(iso).toLocaleString()
  } catch {
    return iso
  }
}

export function AuditView({ token }: AuditViewProps) {
  const { t } = useI18n()
  const [data, setData] = useState<AuditTrace[]>([])
  const [meta, setMeta] = useState<AuditPageMeta>({ page: 1, limit: 50, totalItems: 0, totalPages: 0 })
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState(1)
  const [filterUser, setFilterUser] = useState("")

  const fetchAudit = useCallback(async (p: number) => {
    setLoading(true)
    setError(null)
    try {
      const params = new URLSearchParams({ page: String(p), limit: "50" })
      if (filterUser.trim()) params.set("userId", filterUser.trim())
      const resp = await fetch(`${BASE_URL}/audit-logs?${params}`, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      })
      if (!resp.ok) {
        const text = await resp.text()
        throw new Error(text || `HTTP ${resp.status}`)
      }
      const json: AuditResponse = await resp.json()
      setData(json.data ?? [])
      setMeta(json.meta ?? { page: 1, limit: 50, totalItems: 0, totalPages: 0 })
    } catch (err) {
      setError(err instanceof Error ? err.message : t('audit.failedLoad'))
    } finally {
      setLoading(false)
    }
  }, [token, filterUser, t])

  useEffect(() => { fetchAudit(page) }, [fetchAudit, page])

  const getStatus = (t: AuditTrace): number | undefined => {
    if (t.output && typeof t.output === "object" && "status" in t.output) {
      return t.output.status as number
    }
    if (t.metadata && typeof t.metadata === "object" && "status" in t.metadata) {
      return t.metadata.status as number
    }
    return undefined
  }

  const getRole = (t: AuditTrace): string => {
    const tag = t.tags?.find((s) => s.startsWith("role:"))
    if (tag) return tag.slice(5)
    if (t.metadata && typeof t.metadata === "object" && "role" in t.metadata) {
      return String(t.metadata.role)
    }
    return "—"
  }

  const getLatencyMs = (t: AuditTrace): string => {
    if (t.metadata && typeof t.metadata === "object" && "latency_ms" in t.metadata) {
      return `${t.metadata.latency_ms}ms`
    }
    if (t.latency != null) return `${Math.round(t.latency * 1000)}ms`
    return "—"
  }

  return (
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-foreground">{t('audit.title')}</h2>
          <p className="text-sm text-muted-foreground mt-1">{t('audit.subtitle')}</p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => fetchAudit(page)}
          disabled={loading}
          className="gap-2"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          {t('common.refresh')}
        </Button>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-xs">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder={t('audit.filterByPrincipal')}
            className="pl-9 h-9"
            value={filterUser}
            onChange={(e) => setFilterUser(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { setPage(1); fetchAudit(1) } }}
          />
        </div>
        <Badge variant="secondary" className="text-xs">
          {meta.totalItems} {t('common.total')}
        </Badge>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/30 text-destructive rounded-xl px-4 py-3 text-sm">
          {error}
        </div>
      )}

      {/* Table */}
      <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[180px] px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('audit.time')}</TableHead>
              <TableHead className="px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('audit.principal')}</TableHead>
              <TableHead className="px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('audit.role')}</TableHead>
              <TableHead className="px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('audit.action')}</TableHead>
              <TableHead className="w-[80px] px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('common.status')}</TableHead>
              <TableHead className="w-[90px] text-right px-4 py-3 text-[11px] font-bold text-muted-foreground uppercase">{t('audit.latency')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.length === 0 && !loading && (
              <TableRow>
                <TableCell colSpan={6} className="text-center py-12 text-muted-foreground">
                  <Shield className="h-8 w-8 mx-auto mb-2 opacity-30" />
                  {t('audit.noData')}
                </TableCell>
              </TableRow>
            )}
            {data.map((t) => {
              const status = getStatus(t)
              return (
                <TableRow key={t.id}>
                  <TableCell className="text-xs text-muted-foreground font-mono">
                    {fmtTs(t.timestamp)}
                  </TableCell>
                  <TableCell className="font-medium text-sm">
                    {t.userId || "—"}
                  </TableCell>
                  <TableCell>
                    <Badge variant="outline" className="text-[10px] uppercase font-bold">
                      {getRole(t)}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-sm font-mono">
                    {t.name || "—"}
                  </TableCell>
                  <TableCell>
                    {status != null && (
                      <Badge variant={statusColor(status)} className="text-xs font-bold">
                        {status}
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right text-xs text-muted-foreground font-mono">
                    {getLatencyMs(t)}
                  </TableCell>
                </TableRow>
              )
            })}
          </TableBody>
        </Table>
      </div>

      {/* Pagination */}
      {meta.totalPages > 1 && (
        <div className="flex items-center justify-between">
          <p className="text-xs text-muted-foreground">
            {t('audit.page', { page: meta.page, total: meta.totalPages })}
          </p>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={page <= 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={page >= meta.totalPages}
              onClick={() => setPage((p) => p + 1)}
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
