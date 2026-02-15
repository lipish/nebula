import { useMemo, useState } from "react";
import { Search, Filter, ChevronRight, Server, Cpu, HardDrive } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import type { ClusterStatus, EndpointStats } from "@/lib/types";
import { useI18n } from "@/lib/i18n";

interface EndpointsProps {
  overview: ClusterStatus;
  pct: (used: number, total: number) => number;
  engineStats: EndpointStats[];
}

const statusStyle = (s: string): string => {
  const n = s.toLowerCase();
  if (n.includes("run") || n.includes("ready")) return "bg-success/10 text-success border-success/20 hover:bg-success/10";
  if (n.includes("load")) return "bg-yellow-500/10 text-yellow-600 border-yellow-500/20 hover:bg-yellow-500/10";
  if (n.includes("fail") || n.includes("error")) return "bg-destructive/10 text-destructive border-destructive/20 hover:bg-destructive/10";
  return "bg-muted text-muted-foreground";
};

export function EndpointsView({ overview, pct, engineStats }: EndpointsProps) {
  const { t } = useI18n();
  const [search, setSearch] = useState("");

  // Build stats lookup: (model_uid, replica_id) -> EndpointStats
  const statsMap = useMemo(() => {
    const m = new Map<string, EndpointStats>();
    for (const s of engineStats) {
      m.set(`${s.model_uid}-${s.replica_id}`, s);
    }
    return m;
  }, [engineStats]);

  // Build enriched endpoint rows from real data
  const rows = useMemo(() => {
    return overview.endpoints.map((ep) => {
      // Find the GPU memory info from nodes
      const node = overview.nodes.find((n) => n.node_id === ep.node_id);
      const gpu = node?.gpus.find((g) => {
        // Match by looking at placement assignments
        for (const p of overview.placements) {
          if (p.model_uid !== ep.model_uid) continue;
          for (const a of p.assignments) {
            if (a.node_id === ep.node_id && a.replica_id === ep.replica_id && a.gpu_index != null) {
              return g.index === a.gpu_index;
            }
          }
        }
        return false;
      });

      // Count replicas for this model
      const replicas = overview.endpoints.filter((e) => e.model_uid === ep.model_uid).length;

      // Engine stats for this endpoint
      const es = statsMap.get(`${ep.model_uid}-${ep.replica_id}`);
      const kvUsed = es?.kv_cache_used_bytes ?? 0;
      const kvFree = es?.kv_cache_free_bytes ?? 0;
      const kvTotal = kvUsed + kvFree;
      const kvPct = kvTotal > 0 ? Math.round((kvUsed / kvTotal) * 100) : -1;

      // Resolve engine_type from placement assignment
      let engineType: string | null = null;
      for (const p of overview.placements) {
        if (p.model_uid !== ep.model_uid) continue;
        for (const a of p.assignments) {
          if (a.node_id === ep.node_id && a.replica_id === ep.replica_id) {
            engineType = a.engine_type ?? null;
          }
        }
      }

      return {
        key: `${ep.model_uid}-${ep.replica_id}`,
        model_uid: ep.model_uid,
        replica_id: ep.replica_id,
        node_id: ep.node_id,
        status: ep.status,
        api_flavor: ep.api_flavor,
        gpuIndex: gpu?.index ?? null,
        memUsed: gpu?.memory_used_mb ?? 0,
        memTotal: gpu?.memory_total_mb ?? 0,
        replicas,
        lastHeartbeat: ep.last_heartbeat_ms,
        kvPct,
        pending: es?.pending_requests ?? 0,
        engineType,
      };
    });
  }, [overview, statsMap]);

  const filtered = useMemo(() => {
    if (!search) return rows;
    const q = search.toLowerCase();
    return rows.filter((r) =>
      r.model_uid.toLowerCase().includes(q) ||
      r.node_id.toLowerCase().includes(q)
    );
  }, [rows, search]);

  // Summary stats
  const totalEndpoints = overview.endpoints.length;
  const activeGpus = useMemo(() => {
    let count = 0;
    for (const n of overview.nodes) count += n.gpus.length;
    return count;
  }, [overview.nodes]);
  const totalVram = useMemo(() => {
    let used = 0;
    for (const n of overview.nodes) for (const g of n.gpus) used += g.memory_used_mb;
    return (used / 1024).toFixed(1);
  }, [overview.nodes]);

  const summaryCards = [
    { label: t('endpoints.total'), value: String(totalEndpoints), icon: Server },
    { label: t('endpoints.activeGpus'), value: String(activeGpus), icon: Cpu },
    { label: t('endpoints.totalVram'), value: `${totalVram} GB`, icon: HardDrive },
  ];

  return (
    <>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold text-foreground mb-1">{t('endpoints.title')}</h2>
          <p className="text-sm text-muted-foreground">{t('endpoints.subtitle')}</p>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-3 gap-4 mb-6">
        {summaryCards.map((card) => (
          <div key={card.label} className="bg-card border border-border rounded-xl p-5 flex items-center gap-4">
            <div className="h-10 w-10 rounded-lg bg-muted flex items-center justify-center">
              <card.icon className="h-5 w-5 text-foreground" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">{card.label}</p>
              <p className="text-xl font-bold text-foreground">{card.value}</p>
            </div>
          </div>
        ))}
      </div>

      {/* Endpoint Table */}
      <div className="bg-card border border-border rounded-2xl p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 flex-1 max-w-[240px]">
            <Search className="h-4 w-4 text-muted-foreground" />
            <input
              type="text"
              placeholder={t('endpoints.searchPlaceholder')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="bg-transparent text-sm outline-none w-full text-foreground placeholder:text-muted-foreground"
            />
          </div>
          <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
            <Filter className="h-4 w-4" />
            {t('endpoints.allStatus')}
          </button>
        </div>

        <Table>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead className="w-10"><Checkbox /></TableHead>
              <TableHead className="font-medium">{t('endpoints.endpoint')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.nodeGpu')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.vram')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.kvCache')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.pending')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.engine')}</TableHead>
              <TableHead className="font-medium">{t('endpoints.replicas')}</TableHead>
              <TableHead className="font-medium">{t('common.status')}</TableHead>
              <TableHead className="w-10"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filtered.length === 0 ? (
              <TableRow>
                <TableCell colSpan={10} className="text-center text-muted-foreground py-8">
                  {t('endpoints.notFound')}
                </TableCell>
              </TableRow>
            ) : (
              filtered.map((ep) => {
                const memPercent = ep.memTotal > 0 ? pct(ep.memUsed, ep.memTotal) : 0;
                const kvOverloaded = ep.kvPct > 95;
                return (
                  <TableRow key={ep.key}>
                    <TableCell><Checkbox /></TableCell>
                    <TableCell>
                      <div>
                        <p className="font-mono text-sm font-medium">{ep.model_uid}</p>
                        <p className="text-xs text-muted-foreground">{t('endpoints.replica')} {ep.replica_id}</p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <p className="text-sm">{ep.node_id}</p>
                      <p className="text-xs text-muted-foreground">{ep.gpuIndex != null ? `GPU ${ep.gpuIndex}` : "—"}</p>
                    </TableCell>
                    <TableCell>
                      {ep.memTotal > 0 ? (
                        <div className="w-24">
                          <div className="flex justify-between text-xs mb-1">
                            <span className="text-muted-foreground">{(ep.memUsed / 1024).toFixed(1)} GB</span>
                            <span className="text-muted-foreground">{memPercent}%</span>
                          </div>
                          <Progress value={memPercent} className="h-1.5 [&>div]:bg-chart-1" />
                        </div>
                      ) : (
                        <span className="text-xs text-muted-foreground">—</span>
                      )}
                    </TableCell>
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
                      <Badge variant="outline" className="text-[10px] font-bold">
                        {ep.engineType === "sglang" ? "SGLang" : "vLLM"}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-sm font-medium">{ep.replicas}</TableCell>
                    <TableCell>
                      <Badge className={statusStyle(ep.status)}>{ep.status}</Badge>
                    </TableCell>
                    <TableCell>
                      <button className="p-1 hover:bg-accent rounded transition-colors">
                        <ChevronRight className="h-4 w-4 text-muted-foreground" />
                      </button>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>
    </>
  );
}
