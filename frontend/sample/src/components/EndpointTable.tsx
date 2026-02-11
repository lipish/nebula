import { Search, Filter, MoreHorizontal } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";

interface Endpoint {
  model: string;
  node: string;
  gpu: string;
  memUsed: string;
  status: "ready" | "loading";
}

const endpoints: Endpoint[] = [
  { model: "qwen2_5_0.5b_instruct", node: "node-gpu-01", gpu: "GPU 0", memUsed: "11,127 MB", status: "ready" },
  { model: "qwen2_5_coder_1.5b", node: "node-gpu-01", gpu: "GPU 1", memUsed: "10,949 MB", status: "ready" },
];

const EndpointTable = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6">
      <div className="flex items-center gap-3 mb-4">
        <div className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 flex-1 max-w-[200px]">
          <Search className="h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search models..."
            className="bg-transparent text-sm outline-none w-full text-foreground placeholder:text-muted-foreground"
          />
        </div>
        <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
          <Filter className="h-4 w-4" />
          All Status
        </button>
        <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
          <MoreHorizontal className="h-4 w-4" />
          More
        </button>
      </div>

      <Table>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="w-10"><Checkbox /></TableHead>
            <TableHead className="font-medium">Model</TableHead>
            <TableHead className="font-medium">Node</TableHead>
            <TableHead className="font-medium">GPU</TableHead>
            <TableHead className="font-medium">Memory</TableHead>
            <TableHead className="font-medium">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {endpoints.map((ep) => (
            <TableRow key={ep.model}>
              <TableCell><Checkbox /></TableCell>
              <TableCell className="font-mono text-sm">{ep.model}</TableCell>
              <TableCell className="text-sm">{ep.node}</TableCell>
              <TableCell className="text-sm text-muted-foreground">{ep.gpu}</TableCell>
              <TableCell className="text-sm font-medium">{ep.memUsed}</TableCell>
              <TableCell>
                <span className="text-xs font-medium px-2 py-1 rounded-full bg-success/10 text-success">
                  {ep.status}
                </span>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
};

export default EndpointTable;
