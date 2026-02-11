import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, CartesianGrid, Tooltip, Cell, ReferenceLine } from "recharts";

const data = [
  { month: "Jan", primary: 8000, secondary: 3000 },
  { month: "Feb", primary: 10000, secondary: 4000 },
  { month: "Mar", primary: 6000, secondary: 2500 },
  { month: "Apr", primary: 9000, secondary: 3500 },
  { month: "May", primary: 11000, secondary: 4500 },
  { month: "Jun", primary: 12000, secondary: 10000 },
  { month: "Jul", primary: 9500, secondary: 3000 },
];

const EarningReport = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6">
      <div className="flex items-center justify-between mb-6">
        <h3 className="text-lg font-bold text-foreground">Earning Report</h3>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <span>Total Earnings</span>
          <span className="font-bold text-foreground text-lg">21,640.00</span>
        </div>
      </div>

      <div className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} barGap={2}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="hsl(var(--border))" />
            <XAxis dataKey="month" axisLine={false} tickLine={false} tick={{ fontSize: 13, fill: 'hsl(var(--muted-foreground))' }} />
            <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }} tickFormatter={(v) => `${v / 1000}k`} />
            <Tooltip
              contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 8, fontSize: 13 }}
              formatter={(value: number) => [`$${value.toLocaleString()}`, '']}
            />
            <Bar dataKey="primary" stackId="a" fill="hsl(var(--chart-1))" radius={[0, 0, 0, 0]} barSize={32} />
            <Bar dataKey="secondary" stackId="a" fill="hsl(var(--chart-2))" radius={[4, 4, 0, 0]} barSize={32} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default EarningReport;
