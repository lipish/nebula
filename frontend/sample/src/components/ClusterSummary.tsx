import { Monitor, Activity } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { LineChart, Line, ResponsiveContainer } from "recharts";

const sparkData = [
  { v: 30 }, { v: 45 }, { v: 34 }, { v: 50 }, { v: 38 }, { v: 42 }, { v: 34 }, { v: 55 }, { v: 40 }, { v: 34 },
];

const ClusterSummary = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6 flex items-center justify-between">
      <div className="flex items-center gap-4">
        <div className="h-12 w-12 rounded-xl bg-accent flex items-center justify-center">
          <Monitor className="h-5 w-5 text-muted-foreground" />
        </div>
        <div>
          <p className="text-sm text-muted-foreground mb-1">GPU Utilization</p>
          <div className="flex items-center gap-3">
            <span className="text-3xl font-bold text-foreground">34%</span>
            <Badge className="bg-success/10 text-success border-0 text-xs font-medium hover:bg-success/10">
              ↑ Healthy
            </Badge>
          </div>
        </div>
      </div>
      <div className="flex items-center gap-4">
        <div className="w-28 h-12">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={sparkData}>
              <Line type="monotone" dataKey="v" stroke="hsl(var(--muted-foreground))" strokeWidth={1.5} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="h-12 w-12 rounded-xl border border-border flex items-center justify-center">
          <Activity className="h-5 w-5 text-muted-foreground" />
        </div>
      </div>
    </div>
  );
};

export default ClusterSummary;
