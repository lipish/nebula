import { useState } from "react";
import {
    LayoutDashboard, Box, Server, Settings, HelpCircle, MessageSquare,
    MoreHorizontal, ChevronRight, ChevronDown, Diamond, Activity, Cpu, Shield, Container, Layers
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
    { id: 'inference', icon: Activity, label: "Inference" },
    { id: 'endpoints', icon: Cpu, label: "Endpoints" },
    {
        icon: MoreHorizontal, label: "More", hasArrow: true,
        children: [
            { id: 'images', icon: Container, label: "Images" },
            { id: 'templates', icon: Layers, label: "Templates" },
            { id: 'audit', icon: Shield, label: "Audit Logs" },
        ],
    },
];

const generalItems = [
    { id: 'settings', icon: Settings, label: "Settings" },
    { icon: HelpCircle, label: "Help Center" },
    { icon: MessageSquare, label: "Feedback" },
];

const Sidebar = ({ page, setPage }: SidebarProps) => {
    const [moreOpen, setMoreOpen] = useState(false);
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
                        <div key={item.label}>
                            <button
                                onClick={item.children ? () => setMoreOpen(!moreOpen) : item.id ? () => setPage(item.id) : undefined}
                                className={cn(
                                    "flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                    item.id && page === item.id
                                        ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                        : item.children && moreOpen
                                            ? "text-sidebar-accent-foreground"
                                            : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                )}
                            >
                                <div className="flex items-center gap-3">
                                    <item.icon className="h-[18px] w-[18px]" />
                                    {item.label}
                                </div>
                                {item.hasArrow && (
                                    moreOpen
                                        ? <ChevronDown className="h-4 w-4 text-muted-foreground" />
                                        : <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                )}
                            </button>
                            {item.children && moreOpen && (
                                <div className="ml-4 mt-0.5 space-y-0.5">
                                    {item.children.map((child) => (
                                        <button
                                            key={child.label}
                                            onClick={() => setPage(child.id)}
                                            className={cn(
                                                "flex items-center gap-3 w-full px-3 py-2 rounded-lg text-sm font-medium transition-colors",
                                                page === child.id
                                                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                                    : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                            )}
                                        >
                                            <child.icon className="h-[16px] w-[16px]" />
                                            {child.label}
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
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
                <p className="text-xs text-muted-foreground">v0.1.0</p>
            </div>
        </aside>
    );
};

export default Sidebar;
