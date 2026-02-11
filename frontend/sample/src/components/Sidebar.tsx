import {
  LayoutDashboard, Box, Server, Settings, HelpCircle, MessageSquare,
  MoreHorizontal, ChevronRight, Timer, Diamond, Activity, Cpu
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";

const menuItems = [
  { icon: LayoutDashboard, label: "Dashboard", active: true },
  { icon: Box, label: "Models", active: false },
  { icon: Server, label: "Nodes & GPUs", active: false },
  { icon: Activity, label: "Inference", active: false },
  { icon: Cpu, label: "Endpoints", active: false },
  { icon: MoreHorizontal, label: "More", active: false, hasArrow: true },
];

const generalItems = [
  { icon: Settings, label: "Settings", active: false },
  { icon: HelpCircle, label: "Help Center", active: false },
  { icon: MessageSquare, label: "Feedback", active: false },
];

const Sidebar = () => {
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
              className={cn(
                "flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm font-medium transition-colors",
                item.active
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

      <div className="px-4 pb-5">
        <div className="bg-sidebar-accent rounded-xl p-4">
          <div className="flex items-center gap-2 mb-2">
            <Timer className="h-4 w-4 text-foreground" />
            <span className="text-sm font-semibold text-foreground">BFF Connected</span>
          </div>
          <Progress value={100} className="h-1.5 mb-3 [&>div]:bg-success" />
          <p className="text-xs text-muted-foreground mb-3">All nodes healthy · v0.1.0</p>
          <button className="w-full bg-primary text-primary-foreground text-sm font-semibold py-2.5 rounded-lg hover:bg-primary/90 transition-colors">
            Manage Cluster
          </button>
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
