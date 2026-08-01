import { AppHeader } from "@/components/layout/AppHeader";
import { useConfig } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Settings } from "lucide-react";

export default function ConfigPage() {
  const { data: config, isLoading, isError } = useConfig();

  return (
    <>
      <AppHeader title="Config" subtitle="system configuration" />
      <ScrollArea className="flex-1">
        <div className="p-4 md:p-6 max-w-3xl">
          <Card className="bg-card border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <Settings className="h-4 w-4 text-muted-foreground" />
                /api/admin/config
              </CardTitle>
            </CardHeader>
            <CardContent>
              {isLoading ? (
                <div className="space-y-2">
                  {Array.from({ length: 8 }).map((_, i) => (
                    <Skeleton key={i} className="h-4 w-full" />
                  ))}
                </div>
              ) : isError ? (
                <p className="text-sm text-destructive font-mono">
                  Failed to load configuration
                </p>
              ) : (
                <pre className="font-mono text-[11px] text-foreground whitespace-pre-wrap break-all max-h-[70vh] overflow-auto scrollbar-thin">
                  {JSON.stringify(config, null, 2)}
                </pre>
              )}
            </CardContent>
          </Card>
        </div>
      </ScrollArea>
    </>
  );
}
