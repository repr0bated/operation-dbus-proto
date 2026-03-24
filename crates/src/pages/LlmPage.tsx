import { AppHeader } from "@/components/layout/AppHeader";
import { useLlmStatus, useLlmModels, useSwitchModel } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Brain, Check, Zap } from "lucide-react";

export default function LlmPage() {
  const { data: status, isLoading: statusLoading } = useLlmStatus();
  const { data: models, isLoading: modelsLoading } = useLlmModels();
  const switchMut = useSwitchModel();

  return (
    <>
      <AppHeader title="LLM" subtitle="model management" />
      <div className="flex-1 overflow-hidden flex flex-col">
        <ScrollArea className="flex-1">
          <div className="p-4 md:p-6 space-y-6 max-w-3xl">
            {/* Active model card */}
            <Card className="bg-card border-border">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Brain className="h-4 w-4 text-accent" />
                  Active Configuration
                </CardTitle>
              </CardHeader>
              <CardContent>
                {statusLoading ? (
                  <div className="space-y-2">
                    <Skeleton className="h-4 w-48" />
                    <Skeleton className="h-4 w-32" />
                  </div>
                ) : (
                  <div className="font-mono text-xs space-y-1.5">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">provider</span>
                      <span className="text-foreground">{status?.active_provider ?? "—"}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">model</span>
                      <Badge variant="default" className="text-[10px] font-mono">
                        {status?.active_model ?? "—"}
                      </Badge>
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>

            {/* Available models */}
            <div>
              <h2 className="text-xs text-muted-foreground uppercase tracking-widest mb-3">
                Available Models
              </h2>
              <div className="space-y-2">
                {modelsLoading ? (
                  Array.from({ length: 4 }).map((_, i) => (
                    <Skeleton key={i} className="h-14 w-full rounded-lg" />
                  ))
                ) : models && models.length > 0 ? (
                  models.map((model) => {
                    const isActive = model.id === status?.active_model || model.name === status?.active_model;
                    return (
                      <Card
                        key={model.id}
                        className={`bg-card border-border ${isActive ? "border-primary/40" : ""}`}
                      >
                        <CardContent className="p-3 flex items-center justify-between">
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              {isActive && <Check className="h-3 w-3 text-primary" />}
                              <p className="text-sm font-mono font-medium text-foreground">
                                {model.name || model.id}
                              </p>
                            </div>
                            <div className="flex items-center gap-2 mt-0.5 ml-5">
                              <Badge variant="outline" className="text-[9px] font-mono">
                                {model.provider}
                              </Badge>
                              {model.context_length && (
                                <span className="text-[10px] text-muted-foreground font-mono">
                                  {(model.context_length / 1000).toFixed(0)}k ctx
                                </span>
                              )}
                            </div>
                          </div>
                          {!isActive && (
                            <Button
                              size="sm"
                              variant="ghost"
                              className="gap-1 text-xs"
                              onClick={() => switchMut.mutate(model.id)}
                              disabled={switchMut.isPending}
                            >
                              <Zap className="h-3 w-3" />
                              Switch
                            </Button>
                          )}
                        </CardContent>
                      </Card>
                    );
                  })
                ) : (
                  <p className="text-sm text-muted-foreground font-mono text-center py-8">
                    No models available
                  </p>
                )}
              </div>
            </div>
          </div>
        </ScrollArea>
      </div>
    </>
  );
}
