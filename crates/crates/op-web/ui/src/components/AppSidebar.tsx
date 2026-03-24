import { useState } from "react";
import {
  Home, MessageSquare, Users, Shield, Mail, Zap, BarChart3, Settings,
  Wrench, Bot, GitBranch, Layers, Radio, ScrollText, Bug,
  Terminal, Blocks, Clock, Cog, Activity, FileText,
} from "lucide-react";
import { NavLink } from "@/components/NavLink";
import { useLocation } from "react-router-dom";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupLabel, SidebarGroupContent,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem, useSidebar,
} from "@/components/ui/sidebar";
import { cn } from "@/lib/utils";

const dbusGroups = [
  {
    label: "Core",
    items: [
      { title: "Dashboard", url: "/", icon: Home },
      { title: "Chat", url: "/chat", icon: MessageSquare },
      { title: "Users", url: "/users", icon: Users },
      { title: "VPN", url: "/vpn", icon: Shield },
      { title: "Mail", url: "/mail", icon: Mail },
      { title: "MCP Services", url: "/mcp", icon: Zap },
    ],
  },
  {
    label: "Automation",
    items: [
      { title: "Tools", url: "/tools", icon: Wrench },
      { title: "Agents", url: "/agents", icon: Bot },
      { title: "Workflows", url: "/workflows", icon: GitBranch },
      { title: "Work Stacks", url: "/workstacks", icon: Layers },
      { title: "Orchestration", url: "/orchestration", icon: Radio },
    ],
  },
  {
    label: "System",
    items: [
      { title: "Analytics", url: "/analytics", icon: BarChart3 },
      { title: "Execution Logs", url: "/logs", icon: ScrollText },
      { title: "Debugger", url: "/debugger", icon: Bug },
      { title: "Settings", url: "/settings", icon: Settings },
    ],
  },
];

const openclawGroups = [
  {
    label: "Assistant",
    items: [
      { title: "Chat", url: "/openclaw", icon: Terminal },
      { title: "Sessions", url: "/openclaw/sessions", icon: MessageSquare },
      { title: "Channels", url: "/openclaw/channels", icon: Radio },
    ],
  },
  {
    label: "Automation",
    items: [
      { title: "Skills", url: "/openclaw/skills", icon: Blocks },
      { title: "Cron Jobs", url: "/openclaw/cron", icon: Clock },
    ],
  },
  {
    label: "System",
    items: [
      { title: "Debug", url: "/openclaw/debug", icon: Activity },
      { title: "Logs", url: "/openclaw/logs", icon: FileText },
      { title: "Config", url: "/openclaw/config", icon: Cog },
    ],
  },
];

type Tab = "dbus" | "openclaw";

export function AppSidebar() {
  const { state } = useSidebar();
  const collapsed = state === "collapsed";
  const location = useLocation();
  const [activeTab, setActiveTab] = useState<Tab>(
    location.pathname.startsWith("/openclaw") ? "openclaw" : "dbus"
  );

  const groups = activeTab === "dbus" ? dbusGroups : openclawGroups;

  return (
    <Sidebar collapsible="icon" className="border-r border-sidebar-border">
      <SidebarContent className="pt-4">
        {/* Logo */}
        <div className={`px-4 mb-2 flex items-center gap-2.5 ${collapsed ? "justify-center" : ""}`}>
          <div className="h-8 w-8 rounded-lg bg-primary flex items-center justify-center shrink-0">
            {activeTab === "dbus" ? (
              <Shield className="h-4 w-4 text-primary-foreground" />
            ) : (
              <Terminal className="h-4 w-4 text-primary-foreground" />
            )}
          </div>
          {!collapsed && (
            <div className="flex flex-col">
              <span className="text-sm font-bold tracking-tight text-foreground leading-none">
                {activeTab === "dbus" ? "Operation-DBUS" : "OpenClaw"}
              </span>
              <span className="text-[10px] text-muted-foreground leading-tight mt-0.5">
                {activeTab === "dbus" ? "Privacy Router" : "AI Control UI"}
              </span>
            </div>
          )}
        </div>

        {/* Tab switcher */}
        {!collapsed && (
          <div className="px-3 mb-3">
            <div className="flex rounded-lg bg-muted p-0.5">
              {([["dbus", "DBUS"], ["openclaw", "OpenClaw"]] as const).map(([key, label]) => (
                <button
                  key={key}
                  onClick={() => setActiveTab(key)}
                  className={cn(
                    "flex-1 text-xs font-medium py-1.5 rounded-md transition-all",
                    activeTab === key
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Nav groups */}
        {groups.map((group) => (
          <SidebarGroup key={group.label}>
            {!collapsed && (
              <SidebarGroupLabel className="text-[10px] uppercase tracking-widest text-muted-foreground/60 px-3 mb-1">
                {group.label}
              </SidebarGroupLabel>
            )}
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map((item) => {
                  const isActive = item.url === "/"
                    ? location.pathname === "/"
                    : location.pathname === item.url || (item.url !== "/openclaw" && location.pathname.startsWith(item.url));
                  return (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          to={item.url}
                          end={item.url === "/" || item.url === "/openclaw"}
                          className={`flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                            isActive
                              ? "bg-primary/10 text-primary font-medium"
                              : "text-muted-foreground hover:text-foreground hover:bg-accent"
                          }`}
                          activeClassName=""
                        >
                          <item.icon className="h-4 w-4 shrink-0" />
                          {!collapsed && <span>{item.title}</span>}
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>
    </Sidebar>
  );
}
