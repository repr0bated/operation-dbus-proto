import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  Shield, Bell, Search, Sun, Moon, LogOut, User, Settings,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { useTheme } from "@/hooks/use-theme";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem,
  DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SidebarTrigger } from "@/components/ui/sidebar";

const searchPages = [
  { title: "Dashboard", url: "/" },
  { title: "Chat", url: "/chat" },
  { title: "Users", url: "/users" },
  { title: "VPN", url: "/vpn" },
  { title: "Mail", url: "/mail" },
  { title: "MCP", url: "/mcp" },
  { title: "Tools", url: "/tools" },
  { title: "Agents", url: "/agents" },
  { title: "Workflows", url: "/workflows" },
  { title: "Stacks", url: "/workstacks" },
  { title: "Orchestration", url: "/orchestration" },
  { title: "Analytics", url: "/analytics" },
  { title: "Logs", url: "/logs" },
  { title: "Debugger", url: "/debugger" },
  { title: "Settings", url: "/settings" },
  { title: "OpenClaw", url: "/openclaw" },
  { title: "Models", url: "/openclaw/models" },
  { title: "Plugins", url: "/openclaw/plugins" },
  { title: "Scripts", url: "/openclaw/scripts" },
  { title: "Config", url: "/openclaw/config" },
];

function CommandSearch({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const navigate = useNavigate();
  const filtered = searchPages.filter((p) =>
    p.title.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="p-0 gap-0 max-w-lg">
        <div className="flex items-center border-b border-border px-3">
          <Search className="h-4 w-4 text-muted-foreground shrink-0" />
          <Input
            placeholder="Search pages..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="border-none bg-transparent focus-visible:ring-0 text-sm h-12"
            autoFocus
          />
          <kbd className="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded font-mono">ESC</kbd>
        </div>
        <div className="max-h-64 overflow-auto p-2">
          {filtered.length > 0 ? filtered.map((p) => (
            <button
              key={p.url}
              onClick={() => { navigate(p.url); onClose(); setQuery(""); }}
              className="w-full text-left px-3 py-2 rounded-md text-sm text-foreground hover:bg-accent transition-colors"
            >
              {p.title}
            </button>
          )) : (
            <p className="text-sm text-muted-foreground text-center py-4">No results</p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

export function DashboardLayout({ children }: { children: React.ReactNode }) {
  const [searchOpen, setSearchOpen] = useState(false);
  const { theme, toggleTheme } = useTheme();

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      setSearchOpen(true);
    }
  }, []);

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  return (
    <>
      <div className="flex-1 flex flex-col min-w-0">
        <header className="border-b border-border bg-card/50 backdrop-blur-sm shrink-0">
          <div className="flex items-center justify-between h-12 px-4">
            <div className="flex items-center gap-2">
              <SidebarTrigger className="text-muted-foreground hover:text-foreground" />
            </div>

            <button
              onClick={() => setSearchOpen(true)}
              className="hidden md:flex items-center gap-2 h-8 w-64 px-3 rounded-md bg-secondary text-muted-foreground text-sm hover:bg-accent transition-colors"
            >
              <Search className="h-3.5 w-3.5" />
              <span>Search...</span>
              <kbd className="ml-auto text-xs bg-muted px-1.5 py-0.5 rounded font-mono">⌘K</kbd>
            </button>

            <div className="flex items-center gap-2">
              <button
                onClick={toggleTheme}
                className="p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                aria-label="Toggle theme"
              >
                {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
              </button>

              <button className="relative p-2 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors">
                <Bell className="h-4 w-4" />
                <Badge className="absolute -top-0.5 -right-0.5 h-4 min-w-[16px] px-1 text-[10px] bg-primary text-primary-foreground border-2 border-card flex items-center justify-center">
                  3
                </Badge>
              </button>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button className="h-8 w-8 rounded-full bg-primary/20 flex items-center justify-center text-xs font-semibold text-primary hover:bg-primary/30 transition-colors">
                    OP
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-48">
                  <DropdownMenuItem className="gap-2 text-sm">
                    <User className="h-3.5 w-3.5" /> Profile
                  </DropdownMenuItem>
                  <DropdownMenuItem className="gap-2 text-sm">
                    <Settings className="h-3.5 w-3.5" /> Settings
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem className="gap-2 text-sm text-destructive">
                    <LogOut className="h-3.5 w-3.5" /> Logout
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </header>

        <main className="flex-1 overflow-auto p-6 scrollbar-thin">
          {children}
        </main>
      </div>

      <CommandSearch open={searchOpen} onClose={() => setSearchOpen(false)} />
    </>
  );
}
