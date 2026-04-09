import { useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare, BarChart3, Link2, Radio, FileText, Clock,
  Folder, Zap, Monitor, Settings, Bug, ScrollText, Sun, Moon, Laptop,
  Menu, Shield, GitBranch, Orbit, Box, Globe, Network, Workflow, Brain,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { StatusDot, Pill } from "@/components/shell/Primitives";
import { useTheme } from "@/hooks/use-theme";
import { useEventStore } from "@/stores/event-store";

/** OpenClaw-style nav groups */
const NAV_GROUPS = [
  {
    label: "Chat",
    items: [{ title: "Chat", path: "/chat", icon: MessageSquare }],
  },
  {
    label: "Control",
    items: [
      { title: "Overview", path: "/", icon: BarChart3 },
      { title: "Orchestration", path: "/orchestration", icon: Orbit },
      { title: "Services", path: "/services", icon: Link2 },
      { title: "Sessions", path: "/sessions", icon: FileText },
      { title: "LLM", path: "/llm", icon: Radio },
    ],
  },
  {
    label: "Agent",
    items: [
      { title: "Agents", path: "/agents", icon: Folder },
      { title: "Routable Models", path: "/models", icon: Radio },
      { title: "Tools", path: "/tools", icon: Zap },
      { title: "Workflows", path: "/workflows", icon: GitBranch },
      { title: "Security", path: "/security", icon: Shield },
      { title: "Accountability", path: "/accountability", icon: ScrollText },
      { title: "Skills", path: "/skills", icon: ScrollText },
      { title: "Knowledge", path: "/knowledge", icon: Brain },
      { title: "Embedding Pipeline", path: "/embedding", icon: Orbit },
    ],
  },
  {
    label: "Infrastructure",
    items: [
      { title: "Containers", path: "/containers", icon: Box },
      { title: "Privacy Network", path: "/privacy-network", icon: Globe },
      { title: "Open vSwitch", path: "/ovs", icon: Network },
      { title: "OpenFlow", path: "/openflow", icon: Workflow },
      { title: "BTRFS Storage", path: "/btrfs", icon: Monitor },
      { title: "Data Stores", path: "/data-stores", icon: Monitor },
    ],
  },
  {
    label: "Settings",
    items: [
      { title: "Config", path: "/config", icon: Settings },
      { title: "Inspector", path: "/inspector", icon: Bug },
      { title: "State", path: "/state", icon: Monitor },
      { title: "Logs", path: "/logs", icon: ScrollText },
      { title: "gRPC Diagnostics", path: "/grpc", icon: Radio },
    ],
  },
];

const themeIcons = { dark: Moon, light: Sun, system: Laptop } as const;

export function AppShell({ children }: { children: React.ReactNode }) {
  const [navCollapsed, setNavCollapsed] = useState(false);
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, toggleTheme } = useTheme();
  const { connected, health, lastError } = useEventStore();

  const isFullHeight = location.pathname === "/chat" || location.pathname === "/workflows" || location.pathname === "/accountability";
  const ThemeIcon = themeIcons[theme];

  const toggleGroup = (label: string) => {
    setCollapsedGroups((prev) => ({ ...prev, [label]: !prev[label] }));
  };

  return (
    <div
      className="h-screen grid overflow-hidden transition-[grid-template-columns] duration-200 ease-out"
      style={{
        gridTemplateColumns: navCollapsed ? "0px minmax(0,1fr)" : "220px minmax(0,1fr)",
        gridTemplateRows: "56px 1fr",
        gridTemplateAreas: `"topbar topbar" "nav content"`,
      }}
    >
      {/* ── Topbar ── */}
      <header
        className="flex items-center justify-between gap-4 px-5 h-14 border-b border-border bg-background sticky top-0 z-40"
        style={{ gridArea: "topbar" }}
      >
        <div className="flex items-center gap-3">
          <button
            onClick={() => setNavCollapsed(!navCollapsed)}
            className="w-9 h-9 flex items-center justify-center rounded-md border border-transparent hover:border-border hover:bg-muted/30 transition-colors"
            title={navCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            <Menu className="h-[18px] w-[18px] text-muted-foreground" />
          </button>
          <div className="flex items-center gap-2.5">
            <div className="h-7 w-7 rounded-md bg-primary flex items-center justify-center shrink-0">
              <Shield className="h-3.5 w-3.5 text-primary-foreground" />
            </div>
            <div className="flex flex-col">
              <span className="text-base font-bold tracking-tight leading-none text-foreground">OPERATION-DBUS</span>
              <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-widest leading-none mt-0.5">Control Plane</span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Pill variant={connected ? "ok" : "danger"}>
            <StatusDot status={connected ? "ok" : "error"} />
            <span>Health</span>
            <span className="font-mono">{connected ? "OK" : "Offline"}</span>
          </Pill>

          {/* Theme toggle — OpenClaw 3-way */}
          <div className="relative flex rounded-full border border-border bg-secondary p-0.5">
            {(["dark", "light", "system"] as const).map((m, i) => {
              const Icon = themeIcons[m];
              const active = theme === m;
              return (
                <button
                  key={m}
                  onClick={() => { toggleTheme(); }}
                  className={cn(
                    "h-6 w-6 grid place-items-center rounded-full transition-colors relative z-10",
                    active ? "text-primary-foreground" : "text-muted-foreground hover:text-foreground",
                  )}
                  title={m}
                >
                  <Icon className="h-3 w-3" />
                  {active && (
                    <span className="absolute inset-0 rounded-full bg-primary -z-10 transition-transform" />
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </header>

      {/* ── Sidebar Nav ── */}
      <aside
        className={cn(
          "overflow-y-auto overflow-x-hidden py-4 px-3 bg-background transition-all duration-200 ease-out min-h-0",
          navCollapsed && "w-0 min-w-0 p-0 overflow-hidden opacity-0 pointer-events-none",
        )}
        style={{ gridArea: "nav", scrollbarWidth: "none" }}
      >
        {NAV_GROUPS.map((group) => {
          const isCollapsed = collapsedGroups[group.label] ?? false;
          const hasActive = group.items.some((item) =>
            item.path === "/" ? location.pathname === "/" : location.pathname.startsWith(item.path)
          );

          return (
            <div key={group.label} className="mb-5 last:mb-0">
              <button
                onClick={() => toggleGroup(group.label)}
                className="flex items-center justify-between gap-2 w-full px-2.5 py-1.5 text-[11px] font-medium text-muted-foreground rounded-md hover:text-foreground hover:bg-muted/30 transition-colors"
              >
                <span>{group.label}</span>
                <span className="text-[10px] opacity-50">{isCollapsed && !hasActive ? "+" : "−"}</span>
              </button>
              {(!isCollapsed || hasActive) && (
                <div className="mt-1 space-y-px">
                  {group.items.map((item) => {
                    const active = item.path === "/"
                      ? location.pathname === "/"
                      : location.pathname === item.path;
                    return (
                      <button
                        key={item.path}
                        onClick={() => navigate(item.path)}
                        className={cn(
                          "w-full flex items-center gap-2.5 px-2.5 py-2 rounded-md text-[13px] font-medium transition-colors border border-transparent",
                          active
                            ? "bg-primary/10 text-foreground border-primary/20"
                            : "text-muted-foreground hover:text-foreground hover:bg-muted/30",
                        )}
                      >
                        <item.icon className={cn("h-4 w-4 shrink-0", active ? "text-primary" : "opacity-70")} />
                        <span className="whitespace-nowrap">{item.title}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}

        {/* Resources link */}
        <div className="mt-6">
          <div className="px-2.5 py-1.5 text-[11px] font-medium text-muted-foreground">Resources</div>
          <a
            href="https://docs.operation-dbus.dev"
            target="_blank"
            rel="noreferrer"
            className="flex items-center gap-2.5 px-2.5 py-2 rounded-md text-[13px] font-medium text-muted-foreground hover:text-foreground hover:bg-muted/30 transition-colors"
          >
            <FileText className="h-4 w-4 opacity-70" />
            <span>Docs</span>
          </a>
        </div>
      </aside>

      {/* ── Content ── */}
      <main
        className={cn(
          "min-h-0 overflow-y-auto overflow-x-hidden p-3 md:p-4 space-y-6",
          isFullHeight && "flex flex-col overflow-hidden p-0 space-y-0",
        )}
        style={{ gridArea: "content" }}
      >
        {children}
      </main>
    </div>
  );
}
