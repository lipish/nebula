import { useMemo } from "react";
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip } from "recharts";
import { Activity, Zap, TrendingUp, Gauge } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { ClusterStatus } from "@/lib/types";

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

function parseMetrics(raw: string): GatewayMetrics {
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
    if (key === "nebula_gateway_requests_total") m.requests_total = n;
    else if (key === "nebula_gateway_requests_inflight") m.requests_inflight = n;
    else if (key === "nebula_gateway_responses_2xx") m.responses_2xx = n;
    else if (key === "nebula_gateway_responses_4xx") m.responses_4xx = n;
    else if (key === "nebula_gateway_responses_5xx") m.responses_5xx = n;
    else if (key === "nebula_gateway_auth_missing") m.auth_missing = n;
    else if (key === "nebula_gateway_auth_invalid") m.auth_invalid = n;
    else if (key === "nebula_gateway_auth_forbidden") m.auth_forbidden = n;
    else if (key === "nebula_gateway_auth_rate_limited") m.auth_rate_limited = n;
  }
  return m;
}

interface InferenceProps {
  overview: ClusterStatus;
  metricsRaw: string;
}

export function InferenceView({ overview, metricsRaw }: InferenceProps) {
  const gw = useMemo(() => parseMetrics(metricsRaw), [metricsRaw]);

  const successRate = gw.requests_total > 0
    ? ((gw.responses_2xx / gw.requests_total) * 100).toFixed(1)
    : "—";

  const stats = [
    { label: "Total Requests", value: gw.requests_total.toLocaleString(), icon: Activity },
    { label: "In-Flight", value: gw.requests_inflight.toLocaleString(), icon: Gauge },
    { label: "Active Endpoints", value: String(overview.endpoints.length), icon: Zap },
    { label: "Success Rate", value: successRate === "—" ? "—" : `${successRate}%`, icon: TrendingUp },
  ];

  // Response status breakdown for bar chart
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

  return (
    <>
      <h2 className="text-2xl font-bold text-foreground mb-1">Inference</h2>
      <p className="text-sm text-muted-foreground mb-6">Monitor real-time inference performance and gateway metrics</p>

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
