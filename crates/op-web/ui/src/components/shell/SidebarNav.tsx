import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";
import {
  LayoutDashboard, MessageSquare, Wrench, Bot, Brain,
  Layers, Shield, Settings, Search, Database, FileText,
} from "lucide-react";

const navGroups = [
  {
    label: "Core",
    items: [
      { to: "/overview", icon: LayoutDashboard, label: "Overview" },
      { to: "/chat", icon: MessageSquare, label: "Chat" },
    ],
  },
  {
    label: "Control",
    items: [
      { to: "/tools", icon: Wrench, label: "Tools" },
      { to: "/services", icon: Layers, label: "Services" },
      { to: "/state", icon: Database, label: "State" },
      { to: "/inspector", icon: Search, label: "Inspector" },
    ],
  },
  {
    label: "Agent",
    items: [
      { to: "/agents", icon: Bot, label: "Agents" },
      { to: "/llm", icon: Brain, label: "LLM" },
    ],
  },
  {
    label: "Ops",
    items: [
      { to: "/logs", icon: FileText, label: "Logs" },
      { to: "/security", icon: Shield, label: "Security" },
      { to: "/config", icon: Settings, label: "Config" },
    ],
  },
];

export function SidebarNav() {
  return (
    <aside className="flex w-48 flex-col bg-sidebar border-r border-sidebar-border shrink-0">
      <div className="flex-1 overflow-auto py-2">
        {navGroups.map((group) => (
          <div key={group.label} className="mb-1">
            <div className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-widest text-sidebar-foreground/60">
              {group.label}
            </div>
            {group.items.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    "flex items-center gap-2.5 px-3 py-1.5 text-xs transition-colors mx-1 rounded-sm",
                    isActive
                      ? "bg-sidebar-accent text-sidebar-primary font-medium"
                      : "text-sidebar-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-accent-foreground"
                  )
                }
              >
                <item.icon className="h-3.5 w-3.5 shrink-0" />
                <span>{item.label}</span>
              </NavLink>
            ))}
          </div>
        ))}
      </div>
    </aside>
  );
}
