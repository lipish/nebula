import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip } from "recharts";

const data = [
  { name: "GPU 0", memUsed: 11127, memFree: 21480 },
  { name: "GPU 1", memUsed: 10949, memFree: 21658 },
];

const requestData = [
  { hour: "00:00", gpu0: 120, gpu1: 80 },
  { hour: "04:00", gpu0: 200, gpu1: 150 },
  { hour: "08:00", gpu0: 350, gpu1: 280 },
  { hour: "12:00", gpu0: 500, gpu1: 420 },
  { hour: "16:00", gpu0: 450, gpu1: 380 },
  { hour: "20:00", gpu0: 300, gpu1: 250 },
  { hour: "Now", gpu0: 340, gpu1: 290 },
];

const GpuUsageChart = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6">
      <div className="flex items-center justify-between mb-6">
        <h3 className="text-lg font-bold text-foreground">Inference Load</h3>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <span>Total Requests</span>
          <span className="font-bold text-foreground text-lg">2,840</span>
        </div>
      </div>

      <div className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={requestData} barGap={2}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
            <XAxis dataKey="hour" axisLine={false} tickLine={false} tick={{ fontSize: 13, fill: 'hsl(var(--muted-foreground))' }} />
            <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }} />
            <Tooltip
              contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 8, fontSize: 13 }}
            />
            <Bar dataKey="gpu0" stackId="a" fill="hsl(var(--chart-1))" name="GPU 0" barSize={32} />
            <Bar dataKey="gpu1" stackId="a" fill="hsl(var(--chart-2))" name="GPU 1" radius={[4, 4, 0, 0]} barSize={32} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default GpuUsageChart;
