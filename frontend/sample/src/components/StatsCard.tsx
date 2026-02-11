import { LucideIcon } from "lucide-react";

interface StatsCardProps {
  title: string;
  value: string | number;
  subtitle: string;
  icon: LucideIcon;
}

const StatsCard = ({ title, value, subtitle, icon: Icon }: StatsCardProps) => {
  return (
    <div className="bg-card border border-border rounded-xl p-5 flex flex-col justify-between min-h-[130px]">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-muted-foreground">{title}</span>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </div>
      <div>
        <p className="text-3xl font-bold text-foreground">{value}</p>
        <p className="text-sm text-muted-foreground mt-0.5">{subtitle}</p>
      </div>
    </div>
  );
};

export default StatsCard;
