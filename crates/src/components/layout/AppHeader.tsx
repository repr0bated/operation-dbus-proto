import { SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";

interface AppHeaderProps {
  title: string;
  subtitle?: string;
}

export function AppHeader({ title, subtitle }: AppHeaderProps) {
  return (
    <header className="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-background/80 backdrop-blur-sm px-4">
      <SidebarTrigger className="text-muted-foreground hover:text-foreground" />
      <Separator orientation="vertical" className="h-5" />
      <div className="flex items-baseline gap-2">
        <h1 className="text-sm font-medium text-foreground">{title}</h1>
        {subtitle && (
          <span className="text-xs font-mono text-muted-foreground">
            {subtitle}
          </span>
        )}
      </div>
    </header>
  );
}
