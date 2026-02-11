import { Server } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";

interface GpuInfo {
  name: string;
  usage: number;
  memUsed: number;
  memTotal: number;
  model: string;
}

const gpus: GpuInfo[] = [
  { name: "GPU 0", usage: 34, memUsed: 11127, memTotal: 32607, model: "qwen2_5_coder_1.5b" },
  { name: "GPU 1", usage: 34, memUsed: 10949, memTotal: 32607, model: "qwen2_5_0.5b_instruct" },
];

const GpuOverview = () => {
  return (
    <div className="bg-card border border-border rounded-xl p-5">
      <h3 className="text-base font-semibold text-foreground">GPU Overview</h3>
      <p className="text-sm text-muted-foreground mt-0.5 mb-4">Real-time GPU memory usage across all nodes</p>

      <div className="flex items-center gap-2 mb-4">
        <Server className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm font-medium text-foreground">node-gpu-01</span>
        <Badge variant="outline" className="text-xs">2 GPUs</Badge>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        {gpus.map((gpu) => (
          <div key={gpu.name} className="border border-border rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm font-medium text-foreground">{gpu.name}</span>
              <span className="text-sm font-semibold text-foreground">{gpu.usage}%</span>
            </div>
            <Progress value={gpu.usage} className="h-1.5 mb-2" />
            <p className="text-xs text-muted-foreground mb-2">
              {gpu.memUsed.toLocaleString()} / {gpu.memTotal.toLocaleString()} MB
            </p>
            <Badge variant="secondary" className="text-xs font-mono">{gpu.model}</Badge>
          </div>
        ))}
      </div>
    </div>
  );
};

export default GpuOverview;
