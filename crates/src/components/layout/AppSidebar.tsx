import { useState } from "react";
import {
  Activity,
  Bot,
  Brain,
  Code2,
  Cpu,
  Database,
  Eye,
  HardDrive,
  MessageSquare,
  Search,
  Settings,
  Shield,
  Sparkles,
  Terminal,
  Users,
  Wrench,
  Zap,
} from "lucide-react";
import { NavLink } from "@/components/NavLink";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarHeader,
  SidebarFooter,
} from "@/components/ui/sidebar";
import { cn } from "@/lib/utils";

// Navigation configuration
const dbusMainNav = [
  { title: "Overview", url: "/", icon: Activity },
  { title: "Chat", url: "/chat", icon: MessageSquare },
  { title: "Tools", url: "/tools", icon: Wrench },
  { title: "Agents", url: "/agents", icon: Bot },
  { title: "LLM", url: "/llm", icon: Brain },
];

const dbusSystemNav = [
  { title: "Services", url: "/services", icon: Terminal },
  { title: "Inspector", url: "/inspector", icon: Eye },
  { title: "State & Audit", url: "/state", icon: HardDrive },
  { title: "Security", url: "/security", icon: Shield },
  { title: "Config", url: "/config", icon: Settings },
];

// ── ZeroClaw navigation ──────────────────────────────────────
const clawMainNav = [
  { title: "Dashboard", url: "/claw", icon: Sparkles },
  { title: "Agents", url: "/claw/agents", icon: Users },
  { title: "Models", url: "/claw/models", icon: Cpu },
  { title: "Conversations", url: "/claw/conversations", icon: MessageSquare },
];

const clawToolsNav = [
  { title: "Knowledge Store", url: "/claw/search", icon: Database },
  { title: "Indexer", url: "/claw/indexer", icon: Code2 },
  { title: "Pipelines", url: "/claw/pipelines", icon: Zap },
  { title: "Settings", url: "/claw/settings", icon: Settings },
];

type Pane = "dbus" | "claw";

export function AppSidebar() {
  const [pane, setPane] = useState<Pane>("dbus");

  return (
    <Sidebar className="border-r border-sidebar-border">
      {/* Horizontal pane switcher at top */}
      <div className="flex items-center gap-1 px-2 pt-3 pb-2 border-b border-sidebar-border">
        <button
          onClick={() => setPane("dbus")}
          className={cn(
            "flex items-center justify-center w-8 h-8 rounded-md transition-all",
            pane === "dbus"
              ? "bg-sidebar-accent text-sidebar-foreground shadow-sm"
              : "text-muted-foreground hover:text-sidebar-foreground hover:bg-sidebar-accent/50"
          )}
          title="op-dbus"
        >
          <Terminal className="h-4 w-4" />
        </button>
        <button
          onClick={() => setPane("claw")}
          className={cn(
            "flex items-center justify-center w-8 h-8 rounded-md transition-all",
            pane === "claw"
              ? "bg-sidebar-accent text-sidebar-foreground shadow-sm"
              : "text-muted-foreground hover:text-sidebar-foreground hover:bg-sidebar-accent/50"
          )}
          title="ZeroClaw"
        >
          <Sparkles className="h-4 w-4" />
        </button>
        <span className="text-[10px] font-mono text-muted-foreground ml-auto">
          {pane === "dbus" ? "v1.0.0" : "v0.1.0"}
        </span>
        <div className="h-2 w-2 rounded-full bg-status-online animate-pulse-dot ml-1" title="mail.3tched.com" />
      </div>

      <SidebarContent>
        {pane === "dbus" ? (
          <>
            <SidebarGroup>
              <SidebarGroupLabel className="text-[10px] uppercase tracking-widest text-muted-foreground/60">
                Dashboard
              </SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {dbusMainNav.map((item) => (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          to={item.url}
                          end={item.url === "/"}
                          className="flex items-center gap-2 px-2 py-1.5 text-xs text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
                          activeClassName="bg-sidebar-accent text-primary font-medium"
                        >
                          <item.icon className="h-3.5 w-3.5 shrink-0" />
                          <span>{item.title}</span>
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>

            <SidebarGroup>
              <SidebarGroupLabel className="text-[10px] uppercase tracking-widest text-muted-foreground/60">
                System
              </SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {dbusSystemNav.map((item) => (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          to={item.url}
                          className="flex items-center gap-2 px-2 py-1.5 text-xs text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
                          activeClassName="bg-sidebar-accent text-primary font-medium"
                        >
                          <item.icon className="h-3.5 w-3.5 shrink-0" />
                          <span>{item.title}</span>
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </>
        ) : (
          <>
            <SidebarGroup>
              <SidebarGroupLabel className="text-[10px] uppercase tracking-widest text-muted-foreground/60">
                Platform
              </SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {clawMainNav.map((item) => (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          to={item.url}
                          end={item.url === "/claw"}
                          className="flex items-center gap-2 px-2 py-1.5 text-xs text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
                          activeClassName="bg-sidebar-accent text-primary font-medium"
                        >
                          <item.icon className="h-3.5 w-3.5 shrink-0" />
                          <span>{item.title}</span>
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>

            <SidebarGroup>
              <SidebarGroupLabel className="text-[10px] uppercase tracking-widest text-muted-foreground/60">
                Tools
              </SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {clawToolsNav.map((item) => (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          to={item.url}
                          className="flex items-center gap-2 px-2 py-1.5 text-xs text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
                          activeClassName="bg-sidebar-accent text-primary font-medium"
                        >
                          <item.icon className="h-3.5 w-3.5 shrink-0" />
                          <span>{item.title}</span>
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </>
        )}
      </SidebarContent>
    </Sidebar>
  );
}
