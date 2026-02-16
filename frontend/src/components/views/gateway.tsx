import { useCallback, useEffect, useMemo, useState } from 'react'
import { Activity, AlertTriangle, Clock3, RefreshCw, Shield } from 'lucide-react'
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { v2 } from '@/lib/api'
import type {
  GatewayLatency,
  GatewayOverview,
  GatewayProtection,
  GatewayReliability,
  GatewayTimePoint,
  GatewayTraffic,
} from '@/lib/types'
import { useI18n } from '@/lib/i18n'
import { Button } from '@/components/ui/button'

interface GatewayViewProps {
  token: string
}

const WINDOW_OPTIONS = ['5m', '15m', '1h', '6h', '24h'] as const

const latestValue = (points: GatewayTimePoint[]) => points[points.length - 1]?.value ?? 0

const toPct = (value: number) => `${(value * 100).toFixed(2)}%`

const fmtNumber = (value: number, digits = 2) => value.toLocaleString(undefined, { maximumFractionDigits: digits })

const chartAxisStyle = { fontSize: 11, fill: 'hsl(var(--muted-foreground))' }

export function GatewayView({ token }: GatewayViewProps) {
  const { t } = useI18n()
  const [windowValue, setWindowValue] = useState<(typeof WINDOW_OPTIONS)[number]>('15m')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [overview, setOverview] = useState<GatewayOverview | null>(null)
  const [traffic, setTraffic] = useState<GatewayTraffic | null>(null)
  const [reliability, setReliability] = useState<GatewayReliability | null>(null)
  const [protection, setProtection] = useState<GatewayProtection | null>(null)
  const [latency, setLatency] = useState<GatewayLatency | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [ov, tr, rel, pro, lat] = await Promise.all([
        v2.gatewayOverview(windowValue, token),
        v2.gatewayTraffic(windowValue, token),
        v2.gatewayReliability(windowValue, token),
        v2.gatewayProtection(windowValue, token),
        v2.gatewayLatency(windowValue, token),
      ])
      setOverview(ov)
      setTraffic(tr)
      setReliability(rel)
      setProtection(pro)
      setLatency(lat)
    } catch (err) {
      setError(err instanceof Error ? err.message : t('gateway.loadFailed'))
    } finally {
      setLoading(false)
    }
  }, [token, windowValue, t])

  useEffect(() => {
    if (!token) return
    void load()
  }, [token, load])

  const trafficSummary = useMemo(() => {
    if (!traffic) return null
    return {
      total: latestValue(traffic.series.requests_total),
      s2xx: latestValue(traffic.series.responses_2xx),
      s4xx: latestValue(traffic.series.responses_4xx),
      s5xx: latestValue(traffic.series.responses_5xx),
    }
  }, [traffic])

  const trafficChartData = useMemo(() => {
    if (!trafficSummary) return []
    return [
      { name: '2xx', value: trafficSummary.s2xx },
      { name: '4xx', value: trafficSummary.s4xx },
      { name: '5xx', value: trafficSummary.s5xx },
    ]
  }, [trafficSummary])

  const reliabilitySummary = useMemo(() => {
    if (!reliability) return null
    return {
      retry: latestValue(reliability.series.retry_total),
      retrySuccess: latestValue(reliability.series.retry_success_total),
      connect: latestValue(reliability.series.upstream_error_connect),
      timeout: latestValue(reliability.series.upstream_error_timeout),
      up5xx: latestValue(reliability.series.upstream_error_5xx),
      other: latestValue(reliability.series.upstream_error_other),
    }
  }, [reliability])

  const reliabilityChartData = useMemo(() => {
    if (!reliabilitySummary) return []
    return [
      { name: 'retry', value: reliabilitySummary.retry },
      { name: 'retry_success', value: reliabilitySummary.retrySuccess },
      { name: 'connect', value: reliabilitySummary.connect },
      { name: 'timeout', value: reliabilitySummary.timeout },
      { name: 'upstream_5xx', value: reliabilitySummary.up5xx },
      { name: 'other', value: reliabilitySummary.other },
    ]
  }, [reliabilitySummary])

  const latencySummary = useMemo(() => {
    if (!latency) return null
    return {
      p50: latestValue(latency.series.latency_p50_ms),
      p95: latestValue(latency.series.latency_p95_ms),
      p99: latestValue(latency.series.latency_p99_ms),
      ttft50: latestValue(latency.series.ttft_p50_ms),
      ttft95: latestValue(latency.series.ttft_p95_ms),
    }
  }, [latency])

  const latencyChartData = useMemo(() => {
    if (!latencySummary) return []
    return [
      { name: 'p50', value: latencySummary.p50 },
      { name: 'p95', value: latencySummary.p95 },
      { name: 'p99', value: latencySummary.p99 },
      { name: 'ttft_p50', value: latencySummary.ttft50 },
      { name: 'ttft_p95', value: latencySummary.ttft95 },
    ]
  }, [latencySummary])

  const protectionChartData = useMemo(() => {
    if (!protection) return []
    return [
      { name: 'too_large', value: protection.request_too_large_count },
      { name: 'circuit_skipped', value: protection.circuit_skipped_count },
      { name: 'circuit_open', value: protection.circuit_open_count },
    ]
  }, [protection])

  return (
    <>
      <div className="mb-6 flex items-center justify-between gap-3">
        <div>
          <h2 className="text-2xl font-bold text-foreground mb-1">{t('gateway.title')}</h2>
          <p className="text-sm text-muted-foreground">{t('gateway.subtitle')}</p>
        </div>
        <div className="flex items-center gap-2">
          {WINDOW_OPTIONS.map((option) => (
            <Button
              key={option}
              variant={windowValue === option ? 'default' : 'outline'}
              size="sm"
              onClick={() => setWindowValue(option)}
            >
              {option}
            </Button>
          ))}
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw className="h-4 w-4 mr-1" />
            {t('common.refresh')}
          </Button>
        </div>
      </div>

      {error && (
        <div className="mb-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {t('gateway.loadFailed')}: {error}
        </div>
      )}

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <div className="bg-card border border-border rounded-xl p-5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-muted-foreground">{t('gateway.rps')}</span>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="text-2xl font-bold">{overview ? fmtNumber(overview.rps, 3) : '—'}</div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-muted-foreground">{t('gateway.error5xx')}</span>
            <AlertTriangle className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="text-2xl font-bold">{overview ? toPct(overview.error_5xx_ratio) : '—'}</div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-muted-foreground">{t('gateway.retrySuccess')}</span>
            <Shield className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="text-2xl font-bold">{overview ? toPct(overview.retry_success_ratio) : '—'}</div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-muted-foreground">{t('gateway.circuitOpen')}</span>
            <Clock3 className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="text-2xl font-bold">{overview ? overview.circuit_open_count : '—'}</div>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
        <div className="bg-card border border-border rounded-xl p-5">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">{t('gateway.traffic')}</h3>
            <span className="text-xs text-muted-foreground">{t('gateway.currentWindow', { window: windowValue })}</span>
          </div>
          <div className="text-sm mb-3">req/s: <span className="font-semibold">{trafficSummary ? fmtNumber(trafficSummary.total) : '—'}</span></div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={trafficChartData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <YAxis axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <Tooltip />
                <Bar dataKey="value" fill="hsl(var(--primary))" radius={[6, 6, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">{t('gateway.reliability')}</h3>
            <span className="text-xs text-muted-foreground">{t('gateway.currentWindow', { window: windowValue })}</span>
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={reliabilityChartData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <YAxis axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <Tooltip />
                <Bar dataKey="value" fill="hsl(var(--chart-2))" radius={[6, 6, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">{t('gateway.latency')}</h3>
            <span className="text-xs text-muted-foreground">{t('gateway.currentWindow', { window: windowValue })}</span>
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={latencyChartData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <YAxis axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <Tooltip formatter={(value) => `${fmtNumber(Number(value ?? 0))} ms`} />
                <Bar dataKey="value" fill="hsl(var(--chart-3))" radius={[6, 6, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        <div className="bg-card border border-border rounded-xl p-5">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold">{t('gateway.protection')}</h3>
            <span className="text-xs text-muted-foreground">{t('gateway.currentWindow', { window: windowValue })}</span>
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={protectionChartData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={chartAxisStyle} />
                <YAxis axisLine={false} tickLine={false} tick={chartAxisStyle} allowDecimals={false} />
                <Tooltip />
                <Bar dataKey="value" fill="hsl(var(--chart-4))" radius={[6, 6, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs text-muted-foreground mt-2">
            <div>{t('gateway.tooLarge')}: <span className="font-semibold text-foreground">{protection ? protection.request_too_large_count : '—'}</span></div>
            <div>{t('gateway.circuitSkipped')}: <span className="font-semibold text-foreground">{protection ? protection.circuit_skipped_count : '—'}</span></div>
            <div>{t('gateway.circuitOpen')}: <span className="font-semibold text-foreground">{protection ? protection.circuit_open_count : '—'}</span></div>
          </div>
        </div>
      </div>
    </>
  )
}
