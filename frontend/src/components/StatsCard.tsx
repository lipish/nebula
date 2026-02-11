import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

interface StatsCardProps {
    title: string;
    value: string | number;
    subtitle?: string;
    icon: LucideIcon;
    className?: string;
    iconClassName?: string;
}

const StatsCard = ({ title, value, subtitle, icon: Icon, className, iconClassName }: StatsCardProps) => {
    return (
        <div className={cn("bg-card border border-border rounded-2xl p-6 flex flex-col justify-between min-h-[140px] shadow-sm hover:shadow-md transition-shadow duration-200", className)}>
            <div className="flex items-center justify-between">
                <span className="text-sm font-semibold text-muted-foreground/80 uppercase tracking-tight">{title}</span>
                <div className={cn("p-2 rounded-lg bg-accent/50", iconClassName)}>
                    <Icon className="h-4 w-4 text-muted-foreground" />
                </div>
            </div>
            <div className="mt-4">
                <p className="text-3xl font-bold text-foreground tracking-tight">{value}</p>
                {subtitle && <p className="text-xs font-medium text-muted-foreground mt-1">{subtitle}</p>}
            </div>
        </div>
    );
};

export default StatsCard;
