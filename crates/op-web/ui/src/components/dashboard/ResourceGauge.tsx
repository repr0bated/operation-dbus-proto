import { cn } from "@/lib/utils";

interface ResourceGaugeProps {
  label: string;
  value: number; // 0–100
  unit?: string;
  variant?: "default" | "ok" | "warn" | "danger";
  className?: string;
}

function autoVariant(v: number): "ok" | "warn" | "danger" {
  if (v < 60) return "ok";
  if (v < 85) return "warn";
  return "danger";
}

export function ResourceGauge({ label, value, unit = "%", variant, className }: ResourceGaugeProps) {
  const v = variant ?? autoVariant(value);
  return (
    <div className={cn("space-y-1.5", className)}>
      <div className="flex items-baseline justify-between">
        <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">{label}</span>
        <span className={cn(
          "text-sm font-mono font-bold",
          v === "ok" && "text-ok",
          v === "warn" && "text-warn",
          v === "danger" && "text-danger",
          v === "default" && "text-foreground",
        )}>
          {value.toFixed(1)}{unit}
        </span>
      </div>
      <div className="h-2 w-full rounded-full bg-muted/40 overflow-hidden">
        <div
          className={cn(
            "h-full rounded-full transition-all duration-700 ease-out",
            v === "ok" && "bg-ok shadow-[0_0_6px_hsl(var(--ok)/0.4)]",
            v === "warn" && "bg-warn shadow-[0_0_6px_hsl(var(--warn)/0.4)]",
            v === "danger" && "bg-danger shadow-[0_0_6px_hsl(var(--danger)/0.4)]",
            v === "default" && "bg-primary",
          )}
          style={{ width: `${Math.min(value, 100)}%` }}
        />
      </div>
    </div>
  );
}
