import { ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";

interface Endpoint {
  name: string;
  node: string;
  replica: string;
  status: "ready" | "loading";
}

const endpoints: Endpoint[] = [
  { name: "qwen2_5_0.5b_instruct", node: "node-gpu-01", replica: "replica 0", status: "ready" },
  { name: "qwen2_5_coder_1.5b", node: "node-gpu-01", replica: "replica 0", status: "ready" },
];

const ActiveEndpoints = () => {
  return (
    <div className="bg-card border border-border rounded-xl p-5">
      <h3 className="text-base font-semibold text-foreground">Active Endpoints</h3>
      <p className="text-sm text-muted-foreground mt-0.5 mb-4">Currently serving model instances</p>

      <div className="space-y-2">
        {endpoints.map((ep) => (
          <button
            key={ep.name}
            className="w-full flex items-center justify-between border border-border rounded-lg p-4 hover:bg-accent/50 transition-colors text-left"
          >
            <div>
              <div className="flex items-center gap-2 mb-1">
                <span className="text-sm font-medium font-mono text-foreground">{ep.name}</span>
                <Badge className="bg-success/10 text-success border-success/20 text-xs hover:bg-success/10">
                  {ep.status}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">{ep.node} · {ep.replica}</p>
            </div>
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          </button>
        ))}
      </div>
    </div>
  );
};

export default ActiveEndpoints;
