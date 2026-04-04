import { StatusBadge } from "./StatusBadge";
import { useEventStream } from "@/api/eventStream";
import { Moon, Sun, Wifi, WifiOff } from "lucide-react";
import { useState } from "react";

export function TopStatusBar() {
  const connected = useEventStream((s) => s.connected);
  const counters = useEventStream((s) => s.counters);
  const totalEvents = Object.values(counters).reduce((a, b) => a + b, 0);

  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));
  const toggleDark = () => {
    document.documentElement.classList.toggle("dark");
    setDark(!dark);
  };

  return (
    <header className="flex items-center justify-between border-b bg-card px-4 h-10 shrink-0">
      <div className="flex items-center gap-3">
        <span className="text-xs font-bold tracking-wide text-foreground">OP-DBUS</span>
        <StatusBadge status="active" label="DEV" />
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          {connected ? (
            <Wifi className="h-3 w-3 text-success" />
          ) : (
            <WifiOff className="h-3 w-3 text-destructive" />
          )}
          <span>{connected ? "connected" : "disconnected"}</span>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <span className="text-[10px] text-muted-foreground font-mono tabular-nums">{totalEvents} events</span>
        <button onClick={toggleDark} className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors">
          {dark ? <Sun className="h-3.5 w-3.5" /> : <Moon className="h-3.5 w-3.5" />}
        </button>
      </div>
    </header>
  );
}
