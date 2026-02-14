import { useEffect, useMemo, useState } from "react";
import {
    LayoutDashboard, Box, Server, Settings, HelpCircle, MessageSquare,
    ChevronRight, ChevronDown, Diamond, Activity, Cpu, Shield, Container, Layers, BookOpen
} from "lucide-react";
import { cn } from "@/lib/utils";

interface SidebarProps {
    page: string;
    setPage: (page: any) => void;
    clusterHealthy: boolean;
}

const menuItems = [
    { id: 'dashboard', icon: LayoutDashboard, label: "Dashboard" },
    { id: 'models', icon: Box, label: "Model Service" },
    { id: 'inference', icon: Activity, label: "Inference" },
    { id: 'endpoints', icon: Cpu, label: "Endpoints" },
];

const infrastructureItems = [
    { id: 'nodes', icon: Server, label: "Nodes & GPUs" },
    { id: 'images', icon: Container, label: "Images" },
    { id: 'templates', icon: Layers, label: "Templates" },
];

const resourceItems = [
    { id: 'model-catalog', icon: BookOpen, label: "Model Catalog" },
    { id: 'model-library', icon: Layers, label: "Model Library" },
    { id: 'audit', icon: Shield, label: "Audit Logs" },
];

const systemItems = [
    { id: 'settings', icon: Settings, label: "Settings" },
    { icon: HelpCircle, label: "Help" },
    { icon: MessageSquare, label: "Feedback" },
];

const Sidebar = ({ page, setPage }: SidebarProps) => {
    const infrastructureIds = useMemo(() => infrastructureItems.map((item) => item.id), []);
    const resourceIds = useMemo(() => resourceItems.map((item) => item.id), []);
    const systemIds = useMemo(() => systemItems.map((item) => item.id).filter(Boolean) as string[], []);

    const [menuOpen, setMenuOpen] = useState(true);
    const [infraOpen, setInfraOpen] = useState(true);
    const [resourcesOpen, setResourcesOpen] = useState(true);
    const [systemOpen, setSystemOpen] = useState(true);

    useEffect(() => {
        if (infrastructureIds.includes(page)) {
            setInfraOpen(true);
        }
        if (resourceIds.includes(page)) {
            setResourcesOpen(true);
        }
        if (systemIds.includes(page)) {
            setSystemOpen(true);
        }
    }, [page, infrastructureIds, resourceIds, systemIds]);

    return (
        <aside className="fixed left-0 top-0 h-screen w-64 bg-card border-r border-border flex flex-col">
            <div className="px-6 py-6 flex items-center gap-2.5">
                <Diamond className="h-5 w-5 text-foreground" />
                <h1 className="text-xl font-bold text-foreground tracking-tight">Nebula</h1>
            </div>

            <nav className="flex-1 px-4 overflow-y-auto">
                <div className="flex items-center justify-between px-3 mb-2">
                    <p className="text-xs font-medium text-muted-foreground">Workbench</p>
                    <button onClick={() => setMenuOpen(!menuOpen)} className="text-muted-foreground/60 hover:text-muted-foreground/80 transition-colors">
                        {menuOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                    </button>
                </div>
                {menuOpen && (
                    <div className="space-y-0.5">
                        {menuItems.map((item) => (
                            <button
                                key={item.label}
                                onClick={item.id ? () => setPage(item.id) : undefined}
                                className={cn(
                                    "flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                    item.id && page === item.id
                                        ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                        : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                )}
                            >
                                <item.icon className="h-[18px] w-[18px]" />
                                {item.label}
                            </button>
                        ))}
                    </div>
                )}

                <div className="flex items-center justify-between px-3 mt-6 mb-2">
                    <p className="text-xs font-medium text-muted-foreground">Infrastructure</p>
                    <button onClick={() => setInfraOpen(!infraOpen)} className="text-muted-foreground/60 hover:text-muted-foreground/80 transition-colors">
                        {infraOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                    </button>
                </div>
                {infraOpen && (
                    <div className="space-y-0.5">
                        {infrastructureItems.map((item) => (
                            <button
                                key={item.label}
                                onClick={item.id ? () => setPage(item.id) : undefined}
                                className={cn(
                                    "flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                    item.id && page === item.id
                                        ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                        : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                )}
                            >
                                <item.icon className="h-[18px] w-[18px]" />
                                {item.label}
                            </button>
                        ))}
                    </div>
                )}

                <div className="flex items-center justify-between px-3 mt-6 mb-2">
                    <p className="text-xs font-medium text-muted-foreground">Resources</p>
                    <button onClick={() => setResourcesOpen(!resourcesOpen)} className="text-muted-foreground/60 hover:text-muted-foreground/80 transition-colors">
                        {resourcesOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                    </button>
                </div>
                {resourcesOpen && (
                    <div className="space-y-0.5">
                        {resourceItems.map((item) => (
                            <button
                                key={item.label}
                                onClick={item.id ? () => setPage(item.id) : undefined}
                                className={cn(
                                    "flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                    item.id && page === item.id
                                        ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                        : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                )}
                            >
                                <item.icon className="h-[18px] w-[18px]" />
                                {item.label}
                            </button>
                        ))}
                    </div>
                )}

                <div className="flex items-center justify-between px-3 mt-6 mb-2">
                    <p className="text-xs font-medium text-muted-foreground">System</p>
                    <button onClick={() => setSystemOpen(!systemOpen)} className="text-muted-foreground/60 hover:text-muted-foreground/80 transition-colors">
                        {systemOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                    </button>
                </div>
                {systemOpen && (
                    <div className="space-y-0.5">
                        {systemItems.map((item) => (
                            <button
                                key={item.label}
                                onClick={item.id ? () => setPage(item.id) : undefined}
                                className={cn(
                                    "flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                                    item.id && page === item.id
                                        ? "bg-sidebar-accent text-sidebar-accent-foreground"
                                        : "text-sidebar-foreground hover:bg-sidebar-accent/50"
                                )}
                            >
                                <item.icon className="h-[18px] w-[18px]" />
                                {item.label}
                            </button>
                        ))}
                    </div>
                )}
            </nav>

            <div className="px-5 pb-5">
                <p className="text-xs text-muted-foreground">v0.1.0</p>
            </div>
        </aside>
    );
};

export default Sidebar;
