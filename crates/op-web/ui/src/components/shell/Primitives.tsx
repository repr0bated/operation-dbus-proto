import { cn } from "@/lib/utils";

interface StatusDotProps {
  status: "ok" | "warn" | "error" | "offline";
  className?: string;
}

export function StatusDot({ status, className }: StatusDotProps) {
  return (
    <span className={cn(
      "inline-block h-2 w-2 rounded-full shrink-0",
      status === "ok" && "bg-ok shadow-[0_0_8px_hsl(var(--ok)/0.5)]",
      status === "warn" && "bg-warn shadow-[0_0_8px_hsl(var(--warn)/0.5)] animate-[pulse-dot_2s_ease-in-out_infinite]",
      status === "error" && "bg-danger shadow-[0_0_8px_hsl(var(--danger)/0.5)] animate-[pulse-dot_2s_ease-in-out_infinite]",
      status === "offline" && "bg-muted-foreground",
      className,
    )} />
  );
}

interface PillProps {
  children: React.ReactNode;
  variant?: "default" | "danger" | "ok" | "warn";
  className?: string;
}

export function Pill({ children, variant = "default", className }: PillProps) {
  return (
    <span className={cn(
      "inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium transition-colors",
      variant === "default" && "border-border bg-secondary text-foreground",
      variant === "danger" && "border-danger/20 bg-danger/10 text-danger",
      variant === "ok" && "border-ok/20 bg-ok/10 text-ok",
      variant === "warn" && "border-warn/20 bg-warn/10 text-warn",
      className,
    )}>{children}</span>
  );
}

interface StatCardProps {
  label: string;
  value: string | number;
  sub?: string;
  variant?: "default" | "ok" | "warn" | "danger";
  className?: string;
}

export function StatCard({ label, value, sub, variant = "default", className }: StatCardProps) {
  return (
    <div className={cn(
      "rounded-lg border border-border bg-card p-4 animate-rise transition-all hover:border-muted-foreground/20",
      "shadow-[inset_0_1px_0_hsl(var(--card-foreground)/0.03)]",
      className,
    )}>
      <div className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">{label}</div>
      <div className={cn(
        "mt-1.5 text-2xl font-bold tracking-tight leading-none",
        variant === "ok" && "text-ok",
        variant === "warn" && "text-warn",
        variant === "danger" && "text-danger",
        variant === "default" && "text-foreground",
      )}>{value}</div>
      {sub && <div className="mt-1 text-xs text-muted-foreground">{sub}</div>}
    </div>
  );
}

interface CardProps {
  title?: string;
  subtitle?: string;
  children: React.ReactNode;
  className?: string;
  actions?: React.ReactNode;
}

export function Card({ title, subtitle, children, className, actions }: CardProps) {
  return (
    <div className={cn(
      "rounded-lg border border-border bg-card p-5 animate-rise transition-all",
      "shadow-[0_1px_2px_hsl(0_0%_0%/0.2),inset_0_1px_0_hsl(var(--card-foreground)/0.03)]",
      "hover:border-muted-foreground/20",
      className,
    )}>
      {(title || actions) && (
        <div className="flex items-start justify-between gap-3 mb-3">
          <div>
            {title && <h3 className="text-[15px] font-semibold tracking-tight text-foreground">{title}</h3>}
            {subtitle && <p className="mt-1 text-[13px] text-muted-foreground leading-relaxed">{subtitle}</p>}
          </div>
          {actions}
        </div>
      )}
      {children}
    </div>
  );
}

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

export function PageHeader({ title, subtitle, actions }: PageHeaderProps) {
  return (
    <section className="flex items-end justify-between gap-4 px-2 py-1">
      <div>
        <h1 className="text-[26px] font-bold tracking-tight leading-tight text-foreground">{title}</h1>
        {subtitle && <p className="mt-1.5 text-sm text-muted-foreground">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </section>
  );
}

interface CalloutProps {
  children: React.ReactNode;
  variant?: "default" | "danger" | "warn" | "ok";
  className?: string;
}

export function Callout({ children, variant = "default", className }: CalloutProps) {
  return (
    <div className={cn(
      "rounded-lg border px-4 py-3 text-sm",
      variant === "default" && "border-border bg-muted/30 text-muted-foreground",
      variant === "danger" && "border-danger/20 bg-danger/10 text-danger",
      variant === "warn" && "border-warn/20 bg-warn/10 text-warn",
      variant === "ok" && "border-ok/20 bg-ok/10 text-ok",
      className,
    )}>{children}</div>
  );
}
