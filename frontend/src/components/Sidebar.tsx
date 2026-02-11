import {
    LayoutDashboard, Box, Server, Settings, HelpCircle, MessageSquare,
    MoreHorizontal, ChevronRight, Diamond, Activity, Cpu
} from "lucide-react";
import { cn } from "@/lib/utils";

interface SidebarProps {
    page: string;
    setPage: (page: any) => void;
    clusterHealthy: boolean;
}

const menuItems = [
    { id: 'dashboard', icon: LayoutDashboard, label: "Dashboard" },
    { id: 'models', icon: Box, label: "Models" },
    { id: 'nodes', icon: Server, label: "Nodes & GPUs" },
    { icon: Activity, label: "Inference" },
    { icon: Cpu, label: "Endpoints" },
    { icon: MoreHorizontal, label: "More", hasArrow: true },
];

const generalItems = [
    { id: 'settings', icon: Settings, label: "Settings" },
    { icon: HelpCircle, label: "Help Center" },
    { icon: MessageSquare, label: "Feedback" },
];

const Sidebar = ({ page, setPage, clusterHealthy }: SidebarProps) => {
    return (
        <aside className="fixed left-0 top-0 h-screen w-64 bg-card border-r border-border flex flex-col">
            <div className="px-6 py-6 flex items-center gap-2.5">
                <Diamond className="h-5 w-5 text-foreground" />
                <h1 className="text-xl font-bold text-foreground tracking-tight">Nebula</h1>
            </div>

            <nav className="flex-1 px-4 overflow-y-auto">
                <p className="text-xs font-medium text-muted-foreground px-3 mb-2">Menu</p>
                <div className="space-y-0.5">
                    {menuItems.map((item) => (
                        <button
                            key={item.label}
                            onClick={item.id ? () => setPage(item.id) : undefined}
                            className={cn(
                                "flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                item.id && page === item.id
                                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                    : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                            )}
                        >
                            <div className="flex items-center gap-3">
                                <item.icon className="h-[18px] w-[18px]" />
                                {item.label}
                            </div>
                            {item.hasArrow && <ChevronRight className="h-4 w-4 text-muted-foreground" />}
                        </button>
                    ))}
                </div>

                <p className="text-xs font-medium text-muted-foreground px-3 mt-6 mb-2">General</p>
                <div className="space-y-0.5">
                    {generalItems.map((item) => (
                        <button
                            key={item.label}
                            onClick={item.id ? () => setPage(item.id) : undefined}
                            className={cn(
                                "flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                "text-sidebar-foreground hover:bg-sidebar-accent/50"
                            )}
                        >
                            <item.icon className="h-[18px] w-[18px]" />
                            {item.label}
                        </button>
                    ))}
                </div>
            </nav>

            <div className="px-5 pb-5">
                <p className="text-xs text-muted-foreground">Nebula · v0.1.0</p>
            </div>
        </aside>
    );
};

export default Sidebar;
