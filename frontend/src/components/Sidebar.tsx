import {
    LayoutDashboard, Boxes, Server, Settings, HelpCircle, MessageSquare,
    ChevronRight, Timer, Diamond, Activity, Cpu
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";

interface SidebarProps {
    page: string;
    setPage: (page: any) => void;
    clusterHealthy: boolean;
}

const menuItems = [
    { id: 'dashboard', icon: LayoutDashboard, label: "Dashboard" },
    { id: 'models', icon: Boxes, label: "Models" },
    { id: 'nodes', icon: Server, label: "Nodes & GPUs" },
];

const generalItems = [
    { id: 'settings', icon: Settings, label: "Settings" },
    { icon: HelpCircle, label: "Help Center" },
    { icon: MessageSquare, label: "Feedback" },
];

const Sidebar = ({ page, setPage, clusterHealthy }: SidebarProps) => {
    return (
        <aside className="fixed left-0 top-0 h-screen w-64 bg-card border-r border-border flex flex-col z-30">
            <div className="px-6 py-6 flex items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
                    <Diamond className="h-5 w-5" />
                </div>
                <h1 className="text-xl font-bold text-foreground tracking-tight">Nebula</h1>
            </div>

            <nav className="flex-1 px-4 overflow-y-auto pt-2">
                <p className="text-xs font-semibold text-muted-foreground/60 px-3 mb-2 uppercase tracking-wider">Menu</p>
                <div className="space-y-0.5">
                    {menuItems.map((item) => (
                        <button
                            key={item.label}
                            onClick={() => setPage(item.id)}
                            className={cn(
                                "flex items-center justify-between w-full px-3 py-2.5 rounded-xl text-sm font-medium transition-all duration-200",
                                page === item.id
                                    ? "bg-sidebar-accent text-sidebar-accent-foreground shadow-sm"
                                    : "text-sidebar-foreground hover:bg-sidebar-accent/50 hover:translate-x-0.5"
                            )}
                        >
                            <div className="flex items-center gap-3">
                                <item.icon className={cn("h-[18px] w-[18px]", page === item.id ? "text-primary" : "text-muted-foreground")} />
                                {item.label}
                            </div>
                        </button>
                    ))}
                </div>

                <p className="text-xs font-semibold text-muted-foreground/60 px-3 mt-8 mb-2 uppercase tracking-wider">General</p>
                <div className="space-y-0.5">
                    {generalItems.map((item) => (
                        <button
                            key={item.label}
                            onClick={item.id ? () => setPage(item.id) : undefined}
                            className={cn(
                                "flex items-center gap-3 w-full px-3 py-2.5 rounded-xl text-sm font-medium transition-all duration-200",
                                item.id && page === item.id
                                    ? "bg-sidebar-accent text-sidebar-accent-foreground shadow-sm"
                                    : "text-sidebar-foreground hover:bg-sidebar-accent/50 hover:translate-x-0.5"
                            )}
                        >
                            <item.icon className={cn("h-[18px] w-[18px]", item.id && page === item.id ? "text-primary" : "text-muted-foreground")} />
                            {item.label}
                        </button>
                    ))}
                </div>
            </nav>

            <div className="px-4 pb-6">
                <div className="bg-sidebar-accent/50 rounded-2xl p-4 border border-sidebar-border/50">
                    <div className="flex items-center gap-2 mb-3">
                        <div className={cn("h-2 w-2 rounded-full", clusterHealthy ? "bg-success animate-pulse" : "bg-destructive")} />
                        <span className="text-sm font-bold text-foreground">BFF Connected</span>
                    </div>
                    <Progress value={100} className={cn("h-1.5 mb-3", clusterHealthy ? "[&>div]:bg-success" : "[&>div]:bg-destructive")} />
                    <p className="text-[11px] text-muted-foreground mb-4">
                        {clusterHealthy ? "All endpoints responsive" : "System connectivity issues"} · v0.1.0
                    </p>
                    <button
                        onClick={() => setPage('settings')}
                        className="w-full bg-primary text-primary-foreground text-xs font-bold py-2.5 rounded-xl hover:bg-primary/90 transition-all active:scale-[0.98] shadow-sm"
                    >
                        Manage Cluster
                    </button>
                </div>
            </div>
        </aside>
    );
};

export default Sidebar;
