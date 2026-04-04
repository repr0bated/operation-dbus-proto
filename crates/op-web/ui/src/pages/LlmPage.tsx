import { PageHeader } from "@/components/shell/PageHeader";
import { useLlmStatus, useLlmModels, useSetLlmModel } from "@/hooks/useApi";
import { StatusBadge } from "@/components/shell/StatusBadge";
import { MetricCard } from "@/components/shell/MetricCard";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Brain, CheckCircle } from "lucide-react";

export default function LlmPage() {
  const { data: status } = useLlmStatus();
  const { data: models } = useLlmModels();
  const setModel = useSetLlmModel();

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="LLM" description="Language model configuration and status" />
      <div className="flex-1 overflow-auto p-4 space-y-4">
        {/* Current status */}
        <div className="rounded border bg-card p-4 space-y-3">
          <div className="flex items-center gap-3">
            <Brain className="h-5 w-5 text-primary" />
            <h2 className="text-sm font-semibold">Active Model</h2>
            <StatusBadge status={status?.available ? "active" : "offline"} />
          </div>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <MetricCard label="Provider" value={status?.provider || "—"} />
            <MetricCard label="Model" value={status?.model || "—"} />
            <MetricCard label="Route" value={status?.route || "—"} />
            <MetricCard label="Available" value={status?.available ? "Yes" : "No"} />
          </div>
        </div>

        {/* Available models */}
        <div className="space-y-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Available Models</h2>
          <div className="grid gap-2">
            {models?.map((model) => (
              <div key={model.id} className="rounded border bg-card p-3 flex items-center justify-between">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs font-medium">{model.name}</span>
                    <Badge variant="outline" className="text-[9px]">{model.provider}</Badge>
                    {status?.model === model.id && (
                      <Badge className="text-[9px] bg-success/20 text-success border-success/30">
                        <CheckCircle className="h-2.5 w-2.5 mr-0.5" /> active
                      </Badge>
                    )}
                  </div>
                  <div className="flex gap-1">
                    {model.capabilities.map((c) => (
                      <Badge key={c} variant="secondary" className="text-[9px]">{c}</Badge>
                    ))}
                  </div>
                  <span className="text-[10px] text-muted-foreground font-mono">ctx: {model.context_length.toLocaleString()}</span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={status?.model === model.id || setModel.isPending}
                  onClick={() => setModel.mutate(model.id)}
                >
                  Switch
                </Button>
              </div>
            ))}
            {!models?.length && <div className="text-xs text-muted-foreground">No models available</div>}
          </div>
        </div>
      </div>
    </div>
  );
}
