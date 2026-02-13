import { useMemo } from "react";
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip } from "recharts";
import { Activity, Zap, TrendingUp, Gauge, Timer, AlertTriangle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import type { ClusterStatus, EndpointStats } from "@/lib/types";

interface GatewayMetrics {
  requests_total: number;
  requests_inflight: number;
  responses_2xx: number;
  responses_4xx: number;
  responses_5xx: number;
  auth_missing: number;
  auth_invalid: number;
  auth_forbidden: number;
  auth_rate_limited: number;
}

interface RouterModelMetrics {
  model_uid: string;
  route_2xx: number;
  route_4xx: number;
  route_5xx: number;
  latency_count: number;
  latency_sum: number;
  ttft_count: number;
  ttft_sum: number;
}

function parseGatewayMetrics(raw: string): GatewayMetrics {
  const m: GatewayMetrics = {
    requests_total: 0, requests_inflight: 0,
    responses_2xx: 0, responses_4xx: 0, responses_5xx: 0,
    auth_missing: 0, auth_invalid: 0, auth_forbidden: 0, auth_rate_limited: 0,
  };
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const parts = trimmed.split(/\s+/);
    if (parts.length < 2) continue;
    const [key, val] = parts;
    const n = parseInt(val, 10);
    if (isNaN(n)) continue;
    // gateway prefix — always authoritative
    if (key === "nebula_gateway_requests_total") m.requests_total = n;
    else if (key === "nebula_gateway_requests_inflight") m.requests_inflight = n;
    else if (key === "nebula_gateway_responses_2xx") m.responses_2xx = n;
    else if (key === "nebula_gateway_responses_4xx") m.responses_4xx = n;
    else if (key === "nebula_gateway_responses_5xx") m.responses_5xx = n;
    // router prefix — only fills in fields still at 0
    else if (key === "nebula_router_requests_total" && m.requests_total === 0) m.requests_total = n;
    else if (key === "nebula_router_requests_inflight" && m.requests_inflight === 0) m.requests_inflight = n;
    else if (key === "nebula_router_responses_2xx" && m.responses_2xx === 0) m.responses_2xx = n;
    else if (key === "nebula_router_responses_4xx" && m.responses_4xx === 0) m.responses_4xx = n;
    else if (key === "nebula_router_responses_5xx" && m.responses_5xx === 0) m.responses_5xx = n;
    // auth — gateway prefix only
    else if (key === "nebula_gateway_auth_missing") m.auth_missing = n;
    else if (key === "nebula_gateway_auth_invalid") m.auth_invalid = n;
    else if (key === "nebula_gateway_auth_forbidden") m.auth_forbidden = n;
    else if (key === "nebula_gateway_auth_rate_limited") m.auth_rate_limited = n;
  }
  return m;
}

function extractLabel(line: string, label: string): string | null {
  const re = new RegExp(`${label}="([^"]+)"`);
  const m = re.exec(line);
  return m ? m[1] : null;
}

function parseRouterModelMetrics(raw: string): RouterModelMetrics[] {
  const map = new Map<string, RouterModelMetrics>();
  const ensure = (uid: string) => {
    if (!map.has(uid)) map.set(uid, { model_uid: uid, route_2xx: 0, route_4xx: 0, route_5xx: 0, latency_count: 0, latency_sum: 0, ttft_count: 0, ttft_sum: 0 });
    return map.get(uid)!;
  };

  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const parts = trimmed.split(/\s+/);
    if (parts.length < 2) continue;
    const [key, val] = parts;
    const n = parseFloat(val);
    if (isNaN(n)) continue;
    const uid = extractLabel(key, "model_uid");
    if (!uid) continue;

    if (key.startsWith("nebula_route_total")) {
      const status = extractLabel(key, "status");
      const m = ensure(uid);
      if (status === "2xx") m.route_2xx = n;
      else if (status === "4xx") m.route_4xx = n;
      else if (status === "5xx") m.route_5xx = n;
    } else if (key.startsWith("nebula_route_latency_seconds_count")) {
      ensure(uid).latency_count = n;
    } else if (key.startsWith("nebula_route_latency_seconds_sum")) {
      ensure(uid).latency_sum = n;
    } else if (key.startsWith("nebula_route_ttft_seconds_count")) {
      ensure(uid).ttft_count = n;
    } else if (key.startsWith("nebula_route_ttft_seconds_sum")) {
      ensure(uid).ttft_sum = n;
    }
  }
  return Array.from(map.values());
}

interface InferenceProps {
  overview: ClusterStatus;
  metricsRaw: string;
  engineStats: EndpointStats[];
}

export function InferenceView({ overview, metricsRaw, engineStats }: InferenceProps) {
  const gw = useMemo(() => parseGatewayMetrics(metricsRaw), [metricsRaw]);
  const routerModels = useMemo(() => parseRouterModelMetrics(metricsRaw), [metricsRaw]);

  const successRate = gw.requests_total > 0
    ? ((gw.responses_2xx / gw.requests_total) * 100).toFixed(1)
    : "—";

  // Check overload: all endpoints for any model have kv_cache > 95%
  const overloadedModels = useMemo(() => {
    const byModel = new Map<string, EndpointStats[]>();
    for (const s of engineStats) {
      if (!byModel.has(s.model_uid)) byModel.set(s.model_uid, []);
      byModel.get(s.model_uid)!.push(s);
    }
    const result: string[] = [];
    for (const [uid, stats] of byModel) {
      if (stats.length === 0) continue;
      const allOverloaded = stats.every(s => {
        if (s.kv_cache_used_bytes == null || s.kv_cache_free_bytes == null) return false;
        const total = s.kv_cache_used_bytes + s.kv_cache_free_bytes;
        return total > 0 && (s.kv_cache_used_bytes / total) > 0.95;
      });
      if (allOverloaded) result.push(uid);
    }
    return result;
  }, [engineStats]);

  const stats = [
    { label: "Total Requests", value: gw.requests_total.toLocaleString(), icon: Activity },
    { label: "In-Flight", value: gw.requests_inflight.toLocaleString(), icon: Gauge },
    { label: "Active Endpoints", value: String(overview.endpoints.length), icon: Zap },
    { label: "Success Rate", value: successRate === "—" ? "—" : `${successRate}%`, icon: TrendingUp },
  ];

  const responseData = useMemo(() => [
    { name: "2xx", count: gw.responses_2xx },
    { name: "4xx", count: gw.responses_4xx },
    { name: "5xx", count: gw.responses_5xx },
  ], [gw]);

  const authData = useMemo(() => [
    { name: "Missing", count: gw.auth_missing },
    { name: "Invalid", count: gw.auth_invalid },
    { name: "Forbidden", count: gw.auth_forbidden },
    { name: "Rate Limited", count: gw.auth_rate_limited },
  ], [gw]);

  const statusStyle = (s: string): string => {
    const n = s.toLowerCase();
    if (n.includes("run") || n.includes("ready")) return "bg-success/10 text-success border-success/20 hover:bg-success/10";
    if (n.includes("load")) return "bg-yellow-500/10 text-yellow-600 border-yellow-500/20 hover:bg-yellow-500/10";
    if (n.includes("fail") || n.includes("error")) return "bg-destructive/10 text-destructive border-destructive/20 hover:bg-destructive/10";
    return "bg-muted text-muted-foreground";
  };

  const fmtMs = (seconds: number) => {
    if (seconds < 0.001) return "<1ms";
    if (seconds < 1) return `${(seconds * 1000).toFixed(0)}ms`;
    return `${seconds.toFixed(2)}s`;
  };

  return (
    <>
      <h2 className="text-2xl font-bold text-foreground mb-1">Inference</h2>
      <p className="text-sm text-muted-foreground mb-6">Monitor real-time inference performance and gateway metrics</p>

      {/* Overload Alert */}
      {overloadedModels.length > 0 && (
        <div className="bg-destructive/10 border border-destructive/30 rounded-xl p-4 mb-6 flex items-center gap-3">
          <AlertTriangle className="h-5 w-5 text-destructive flex-shrink-0" />
          <div>
            <p className="text-sm font-bold text-destructive">Overload Detected</p>
            <p className="text-xs text-destructive/80 mt-0.5">
              All endpoints overloaded (KV cache &gt;95%) for: <span className="font-mono font-bold">{overloadedModels.join(", ")}</span>. New requests will receive 429 responses.
            </p>
          </div>
        </div>
      )}

      {/* Stats Cards */}
      <div className="grid grid-cols-4 gap-4 mb-6">
        {stats.map((stat) => (
          <div key={stat.label} className="bg-card border border-border rounded-xl p-5">
            <div className="flex items-center justify-between mb-3">
              <span className="text-sm text-muted-foreground">{stat.label}</span>
              <stat.icon className="h-4 w-4 text-muted-foreground" />
            </div>
            <span className="text-2xl font-bold text-foreground">{stat.value}</span>
          </div>
        ))}
      </div>

      {/* Per-Model Router Metrics */}
      {routerModels.length > 0 && (
        <div className="bg-card border border-border rounded-2xl p-6 mb-6">
          <div className="mb-5">
            <h3 className="text-base font-bold text-foreground">Per-Model Routing Metrics</h3>
            <p className="text-xs text-muted-foreground mt-0.5">E2E latency, TTFT, and request counts from the Router</p>
          </div>
          <div className="overflow-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border">
                  <th className="text-left font-medium text-muted-foreground py-3 px-2">Model</th>
                  <th className="text-left font-medium text-muted-foreground py-3 px-2">Requests</th>
                  <th className="text-left font-medium text-muted-foreground py-3 px-2">2xx / 4xx / 5xx</th>
                  <th className="text-left font-medium text-muted-foreground py-3 px-2">Avg Latency</th>
                  <th className="text-left font-medium text-muted-foreground py-3 px-2">Avg TTFT</th>
                </tr>
              </thead>
              <tbody>
                {routerModels.map((rm) => {
                  const totalReqs = rm.route_2xx + rm.route_4xx + rm.route_5xx;
                  const avgLatency = rm.latency_count > 0 ? rm.latency_sum / rm.latency_count : 0;
                  const avgTtft = rm.ttft_count > 0 ? rm.ttft_sum / rm.ttft_count : 0;
                  return (
                    <tr key={rm.model_uid} className="border-b border-border last:border-0 hover:bg-muted/30 transition-colors">
                      <td className="py-3 px-2 font-mono text-xs font-medium">{rm.model_uid}</td>
                      <td className="py-3 px-2 text-sm font-bold">{totalReqs.toLocaleString()}</td>
                      <td className="py-3 px-2 text-xs">
                        <span className="text-success font-bold">{rm.route_2xx}</span>
                        {" / "}
                        <span className="text-yellow-600 font-bold">{rm.route_4xx}</span>
                        {" / "}
                        <span className="text-destructive font-bold">{rm.route_5xx}</span>
                      </td>
                      <td className="py-3 px-2">
                        <div className="flex items-center gap-1.5">
                          <Timer className="h-3.5 w-3.5 text-muted-foreground" />
                          <span className="text-sm font-bold">{rm.latency_count > 0 ? fmtMs(avgLatency) : "—"}</span>
                        </div>
                      </td>
                      <td className="py-3 px-2">
                        <div className="flex items-center gap-1.5">
                          <Zap className="h-3.5 w-3.5 text-muted-foreground" />
                          <span className="text-sm font-bold">{rm.ttft_count > 0 ? fmtMs(avgTtft) : "—"}</span>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Engine Stats — KV Cache & Pending Requests */}
      {engineStats.length > 0 && (
        <div className="bg-card border border-border rounded-2xl p-6 mb-6">
          <div className="mb-5">
            <h3 className="text-base font-bold text-foreground">Engine Stats</h3>
            <p className="text-xs text-muted-foreground mt-0.5">KV cache usage and pending requests per endpoint</p>
          </div>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {engineStats.map((s) => {
              const kvTotal = (s.kv_cache_used_bytes ?? 0) + (s.kv_cache_free_bytes ?? 0);
              const kvPct = kvTotal > 0 ? Math.round(((s.kv_cache_used_bytes ?? 0) / kvTotal) * 100) : 0;
              const isOverloaded = kvPct > 95;
              return (
                <div key={`${s.model_uid}-${s.replica_id}`} className="border border-border rounded-xl p-4 space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-xs font-bold">{s.model_uid}</span>
                    <Badge className="text-[10px] px-1.5 py-0 h-4 bg-muted text-muted-foreground border-0">R{s.replica_id}</Badge>
                  </div>
                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-[11px] font-bold text-muted-foreground/70 uppercase">
                      <span>KV Cache</span>
                      <span className={isOverloaded ? "text-destructive" : "text-foreground"}>{kvPct}%</span>
                    </div>
                    <Progress value={kvPct} className={`h-1.5 ${isOverloaded ? "[&>div]:bg-destructive" : ""}`} />
                  </div>
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-muted-foreground/70 uppercase">Pending</span>
                    <span className="font-bold text-foreground">{s.pending_requests}</span>
                  </div>
                  {s.prefix_cache_hit_rate != null && (
                    <div className="flex items-center justify-between text-[11px]">
                      <span className="font-bold text-muted-foreground/70 uppercase">Prefix Cache Hit</span>
                      <span className="font-bold text-foreground">{(s.prefix_cache_hit_rate * 100).toFixed(1)}%</span>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Charts Row */}
      <div className="grid grid-cols-2 gap-5 mb-6">
        {/* Response Status Chart */}
        <div className="bg-card border border-border rounded-2xl p-6">
          <div className="mb-5">
            <h3 className="text-base font-bold text-foreground">Response Status</h3>
            <p className="text-xs text-muted-foreground mt-0.5">Breakdown by HTTP status code</p>
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={responseData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={{ background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 8, fontSize: 12 }} />
                <Bar dataKey="count" fill="hsl(var(--chart-1))" radius={[4, 4, 0, 0]} barSize={40} name="Count" />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* Auth Events Chart */}
        <div className="bg-card border border-border rounded-2xl p-6">
          <div className="mb-5">
            <h3 className="text-base font-bold text-foreground">Auth Events</h3>
            <p className="text-xs text-muted-foreground mt-0.5">Authentication and authorization failures</p>
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={authData}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
                <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }} />
                <Tooltip contentStyle={{ background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 8, fontSize: 12 }} />
                <Bar dataKey="count" fill="hsl(var(--chart-2))" radius={[4, 4, 0, 0]} barSize={40} name="Count" />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>

      {/* Active Endpoints */}
      <div className="bg-card border border-border rounded-2xl p-6">
        <h3 className="text-base font-bold text-foreground mb-1">Active Endpoints</h3>
        <p className="text-xs text-muted-foreground mb-4">Model instances currently serving inference</p>

        <div className="overflow-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left font-medium text-muted-foreground py-3 px-2">Model</th>
                <th className="text-left font-medium text-muted-foreground py-3 px-2">Replica</th>
                <th className="text-left font-medium text-muted-foreground py-3 px-2">Node</th>
                <th className="text-left font-medium text-muted-foreground py-3 px-2">API</th>
                <th className="text-left font-medium text-muted-foreground py-3 px-2">Status</th>
                <th className="text-left font-medium text-muted-foreground py-3 px-2">Base URL</th>
              </tr>
            </thead>
            <tbody>
              {overview.endpoints.length === 0 ? (
                <tr>
                  <td colSpan={6} className="text-center text-muted-foreground py-8">No active endpoints</td>
                </tr>
              ) : (
                overview.endpoints.map((ep) => (
                  <tr key={`${ep.model_uid}-${ep.replica_id}`} className="border-b border-border last:border-0 hover:bg-muted/30 transition-colors">
                    <td className="py-3 px-2 font-mono text-xs">{ep.model_uid}</td>
                    <td className="py-3 px-2 text-xs">{ep.replica_id}</td>
                    <td className="py-3 px-2 text-xs text-muted-foreground">{ep.node_id}</td>
                    <td className="py-3 px-2 text-xs">{ep.api_flavor}</td>
                    <td className="py-3 px-2">
                      <Badge className={statusStyle(ep.status)}>{ep.status}</Badge>
                    </td>
                    <td className="py-3 px-2 font-mono text-xs text-muted-foreground">{ep.base_url || ep.grpc_target || "—"}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </>
  );
}
