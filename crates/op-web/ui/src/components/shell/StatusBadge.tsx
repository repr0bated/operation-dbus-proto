import { cn } from "@/lib/utils";

interface StatusBadgeProps {
  status: "healthy" | "degraded" | "unhealthy" | "online" | "offline" | "active" | "idle" | "error" | "connected" | "disconnected" | string;
  label?: string;
  className?: string;
  pulse?: boolean;
}

const statusColors: Record<string, string> = {
  healthy: "bg-success/20 text-success border-success/30",
  online: "bg-success/20 text-success border-success/30",
  active: "bg-success/20 text-success border-success/30",
  connected: "bg-success/20 text-success border-success/30",
  degraded: "bg-warning/20 text-warning border-warning/30",
  idle: "bg-muted text-muted-foreground border-border",
  unhealthy: "bg-destructive/20 text-destructive border-destructive/30",
  offline: "bg-destructive/20 text-destructive border-destructive/30",
  error: "bg-destructive/20 text-destructive border-destructive/30",
  disconnected: "bg-destructive/20 text-destructive border-destructive/30",
};

export function StatusBadge({ status, label, className, pulse }: StatusBadgeProps) {
  const color = statusColors[status] || "bg-muted text-muted-foreground border-border";
  return (
    <span className={cn("inline-flex items-center gap-1.5 rounded-sm border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider", color, className)}>
      <span className={cn("h-1.5 w-1.5 rounded-full bg-current", pulse && "animate-pulse-dot")} />
      {label || status}
    </span>
  );
}
