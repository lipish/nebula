import { CreditCard, Focus } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { LineChart, Line, ResponsiveContainer } from "recharts";

const sparkData = [
  { v: 40 }, { v: 30 }, { v: 45 }, { v: 35 }, { v: 50 }, { v: 42 }, { v: 55 }, { v: 48 }, { v: 38 }, { v: 52 },
];

const BalanceCard = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6 flex items-center justify-between">
      <div className="flex items-center gap-4">
        <div className="h-12 w-12 rounded-xl bg-accent flex items-center justify-center">
          <CreditCard className="h-5 w-5 text-muted-foreground" />
        </div>
        <div>
          <p className="text-sm text-muted-foreground mb-1">Total Balance</p>
          <div className="flex items-center gap-3">
            <span className="text-3xl font-bold text-foreground">$ 15,480.80</span>
            <Badge className="bg-success/10 text-success border-0 text-xs font-medium hover:bg-success/10">
              ↑ 8%
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
          <Focus className="h-5 w-5 text-muted-foreground" />
        </div>
      </div>
    </div>
  );
};

export default BalanceCard;
