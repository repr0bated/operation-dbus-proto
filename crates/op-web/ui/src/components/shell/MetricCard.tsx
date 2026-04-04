import { cn } from "@/lib/utils";

interface MetricCardProps {
  label: string;
  value: string | number;
  sub?: string;
  className?: string;
  icon?: React.ReactNode;
}

export function MetricCard({ label, value, sub, className, icon }: MetricCardProps) {
  return (
    <div className={cn("rounded border bg-card p-3 space-y-1", className)}>
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">{label}</span>
        {icon && <span className="text-muted-foreground">{icon}</span>}
      </div>
      <div className="text-xl font-bold font-mono tabular-nums">{value}</div>
      {sub && <div className="text-[10px] text-muted-foreground">{sub}</div>}
    </div>
  );
}
