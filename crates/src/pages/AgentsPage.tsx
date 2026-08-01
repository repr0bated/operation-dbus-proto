import { useState, useMemo } from "react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { AppHeader } from "@/components/layout/AppHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Slider } from "@/components/ui/slider";
import {
  Search,
  Circle,
  Bot,
  Brain,
  Cpu,
  Zap,
  ArrowRight,
  ArrowLeft,
  MemoryStick,
  Clock,
} from "lucide-react";

/* ── Types ────────────────────────────────────────────────── */

interface AgentTemplate {
  id: string;
  name: string;
  description: string;
  category: string;
  capabilities: string[];
  defaultModel: string;
  icon: "bot" | "brain" | "cpu" | "zap";
}

interface ActiveAgent {
  id: string;
  templateId: string;
  name: string;
  status: "running" | "idle" | "paused" | "error";
  model: string;
  uptime: string;
  memoryEntries: number;
  tokensUsed: number;
  tokenBudget: number;
  temperature: number;
  maxTokens: number;
  topP: number;
}

/* ── Mock data ────────────────────────────────────────────── */

const allTemplates: AgentTemplate[] = [
  { id: "infra-ops", name: "InfraOps", description: "Infrastructure monitoring via D-Bus", category: "operations", capabilities: ["dbus-call", "service-restart", "metric-alert", "log-query"], defaultModel: "mistral-7b-instruct", icon: "cpu" },
  { id: "code-review", name: "CodeReview", description: "Autonomous code review with repo context", category: "development", capabilities: ["git-diff", "lint", "suggest-fix", "pr-comment"], defaultModel: "codellama-13b", icon: "brain" },
  { id: "security-audit", name: "SecAudit", description: "Security posture scanning and CVE correlation", category: "security", capabilities: ["cve-scan", "rls-check", "audit-log", "alert"], defaultModel: "mistral-7b-instruct", icon: "zap" },
  { id: "chat-assistant", name: "ChatAssist", description: "General-purpose conversational assistant", category: "assistant", capabilities: ["chat", "tool-call", "memory", "summarize"], defaultModel: "mistral-7b-instruct", icon: "bot" },
  { id: "data-pipeline", name: "DataPipe", description: "ETL pipeline orchestrator with schema inference", category: "data", capabilities: ["extract", "transform", "load", "validate"], defaultModel: "mistral-7b-instruct", icon: "cpu" },
];

const initialActive: ActiveAgent[] = [
  { id: "a-001", templateId: "infra-ops", name: "infra-ops-primary", status: "running", model: "mistral-7b-instruct", uptime: "4d 12h", memoryEntries: 342, tokensUsed: 128400, tokenBudget: 500000, temperature: 0.3, maxTokens: 2048, topP: 0.9 },
  { id: "a-002", templateId: "security-audit", name: "sec-audit-continuous", status: "running", model: "mistral-7b-instruct", uptime: "2d 8h", memoryEntries: 89, tokensUsed: 45200, tokenBudget: 200000, temperature: 0.1, maxTokens: 4096, topP: 0.95 },
  { id: "a-003", templateId: "chat-assistant", name: "chat-main", status: "idle", model: "mistral-7b-instruct", uptime: "6d 1h", memoryEntries: 1204, tokensUsed: 312000, tokenBudget: 500000, temperature: 0.7, maxTokens: 2048, topP: 0.9 },
];

/* ── Helpers ──────────────────────────────────────────────── */

const iconMap = { bot: Bot, brain: Brain, cpu: Cpu, zap: Zap };
const statusColors: Record<string, string> = { running: "text-status-online", idle: "text-status-unknown", paused: "text-status-degraded", error: "text-status-offline" };
const catColors: Record<string, string> = { operations: "text-accent", development: "text-primary", security: "text-warning", assistant: "text-muted-foreground", data: "text-[hsl(var(--log-critical))]" };

/* ── Page ─────────────────────────────────────────────────── */

export default function AgentsPage() {
  const [agents, setAgents] = useState<ActiveAgent[]>(initialActive);
  const [search, setSearch] = useState("");

  const activeIds = useMemo(() => new Set(agents.map((a) => a.templateId)), [agents]);

  const available = useMemo(() => {
    const q = search.toLowerCase();
    return allTemplates
      .filter((t) => !activeIds.has(t.id))
      .filter((t) => !q || t.name.toLowerCase().includes(q) || t.category.includes(q));
  }, [search, activeIds]);

  const activate = (t: AgentTemplate) => {
    setAgents((prev) => [
      ...prev,
      {
        id: `a-${Date.now()}`,
        templateId: t.id,
        name: t.name.toLowerCase(),
        status: "idle",
        model: t.defaultModel,
        uptime: "0s",
        memoryEntries: 0,
        tokensUsed: 0,
        tokenBudget: 500000,
        temperature: 0.5,
        maxTokens: 2048,
        topP: 0.9,
      },
    ]);
  };

  const deactivate = (id: string) => {
    setAgents((prev) => prev.filter((a) => a.id !== id));
  };

  const updateAgent = (id: string, patch: Partial<ActiveAgent>) => {
    setAgents((prev) => prev.map((a) => (a.id === id ? { ...a, ...patch } : a)));
  };

  return (
    <>
      <AppHeader title="Agents" subtitle={`${agents.length} active · ${allTemplates.length} total`} />
      <div className="flex-1 overflow-hidden flex flex-col">
        <Tabs defaultValue="manage" className="flex-1 flex flex-col overflow-hidden">
          <div className="px-4 pt-3 border-b border-border">
            <TabsList className="h-8 bg-muted/50">
              <TabsTrigger value="manage" className="text-xs font-mono px-4">Manage</TabsTrigger>
              <TabsTrigger value="configure" className="text-xs font-mono px-4">Configure</TabsTrigger>
            </TabsList>
          </div>

          {/* ── Tab 1: Manage — two panes, add/remove ──── */}
          <TabsContent value="manage" className="flex-1 overflow-hidden mt-0">
            <div className="flex h-full">
              {/* Available */}
              <div className="flex-1 border-r border-border flex flex-col">
                <div className="p-3 border-b border-border space-y-2">
                  <div className="relative">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
                    <Input
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                      placeholder="Filter…"
                      className="pl-8 h-8 bg-muted border-border font-mono text-xs"
                    />
                  </div>
                  <p className="text-[10px] uppercase tracking-widest text-muted-foreground/50 font-semibold">
                    Available ({available.length})
                  </p>
                </div>
                <ScrollArea className="flex-1">
                  <div className="p-2 space-y-1">
                    {available.length === 0 ? (
                      <p className="text-xs text-muted-foreground/50 font-mono text-center py-8">All agents active</p>
                    ) : (
                      available.map((t) => {
                        const Icon = iconMap[t.icon];
                        return (
                          <div
                            key={t.id}
                            className="flex items-center gap-2.5 p-2.5 rounded border border-transparent hover:bg-muted/50 group"
                          >
                            <Icon className={`h-4 w-4 shrink-0 ${catColors[t.category] || "text-muted-foreground"}`} />
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-1.5">
                                <span className="text-xs font-mono font-medium text-foreground">{t.name}</span>
                                <Badge variant="outline" className={`text-[9px] font-mono ${catColors[t.category] || ""}`}>{t.category}</Badge>
                              </div>
                              <p className="text-[10px] text-muted-foreground mt-0.5 truncate">{t.description}</p>
                            </div>
                            <Button
                              size="sm"
                              variant="ghost"
                              className="h-7 w-7 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
                              onClick={() => activate(t)}
                            >
                              <ArrowRight className="h-3.5 w-3.5 text-primary" />
                            </Button>
                          </div>
                        );
                      })
                    )}
                  </div>
                </ScrollArea>
              </div>

              {/* Active */}
              <div className="flex-1 flex flex-col">
                <div className="p-3 border-b border-border">
                  <p className="text-[10px] uppercase tracking-widest text-muted-foreground/50 font-semibold">
                    Active ({agents.length})
                  </p>
                </div>
                <ScrollArea className="flex-1">
                  <div className="p-2 space-y-1">
                    {agents.map((agent) => {
                      const tpl = allTemplates.find((t) => t.id === agent.templateId);
                      const Icon = tpl ? iconMap[tpl.icon] : Bot;
                      return (
                        <div
                          key={agent.id}
                          className="flex items-center gap-2.5 p-2.5 rounded border border-transparent hover:bg-muted/50 group"
                        >
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 w-7 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
                            onClick={() => deactivate(agent.id)}
                          >
                            <ArrowLeft className="h-3.5 w-3.5 text-destructive" />
                          </Button>
                          <Circle className={`h-2.5 w-2.5 fill-current shrink-0 ${statusColors[agent.status]}`} />
                          <Icon className={`h-4 w-4 shrink-0 ${catColors[tpl?.category || ""] || "text-muted-foreground"}`} />
                          <div className="min-w-0 flex-1">
                            <span className="text-xs font-mono font-medium text-foreground">{agent.name}</span>
                            <div className="flex items-center gap-2 text-[10px] font-mono text-muted-foreground/50 mt-0.5">
                              <span className="flex items-center gap-0.5"><Clock className="h-2.5 w-2.5" />{agent.uptime}</span>
                              <span className="flex items-center gap-0.5"><MemoryStick className="h-2.5 w-2.5" />{agent.memoryEntries}</span>
                              <span>{agent.model}</span>
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </ScrollArea>
              </div>
            </div>
          </TabsContent>

          {/* ── Tab 2: Configure — grid of agent cards ──── */}
          <TabsContent value="configure" className="flex-1 overflow-hidden mt-0">
            <ScrollArea className="h-full">
              <div className="p-4 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                {agents.length === 0 ? (
                  <p className="text-xs text-muted-foreground/50 font-mono text-center py-8 col-span-full">
                    No active agents to configure
                  </p>
                ) : (
                  agents.map((agent) => {
                    const tpl = allTemplates.find((t) => t.id === agent.templateId);
                    const Icon = tpl ? iconMap[tpl.icon] : Bot;
                    return (
                      <div
                        key={agent.id}
                        className="rounded-lg border border-border bg-card p-4 space-y-4"
                      >
                        {/* Card header */}
                        <div className="flex items-center gap-2">
                          <Circle className={`h-2.5 w-2.5 fill-current shrink-0 ${statusColors[agent.status]}`} />
                          <Icon className={`h-4 w-4 shrink-0 ${catColors[tpl?.category || ""] || "text-muted-foreground"}`} />
                          <span className="text-sm font-mono font-medium text-foreground">{agent.name}</span>
                        </div>

                        {/* Token bar */}
                        <div className="flex items-center gap-2">
                          <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
                            <div
                              className="h-full bg-primary/60 rounded-full"
                              style={{ width: `${(agent.tokensUsed / agent.tokenBudget) * 100}%` }}
                            />
                          </div>
                          <span className="text-[9px] font-mono text-muted-foreground/50">
                            {Math.round(agent.tokensUsed / 1000)}k/{Math.round(agent.tokenBudget / 1000)}k
                          </span>
                        </div>

                        {/* Sliders */}
                        <div className="space-y-3">
                          <SliderParam
                            label="Temperature"
                            value={agent.temperature}
                            min={0} max={1} step={0.05}
                            onChange={(v) => updateAgent(agent.id, { temperature: v })}
                          />
                          <SliderParam
                            label="Max Tokens"
                            value={agent.maxTokens}
                            min={256} max={8192} step={256}
                            display={(v) => `${v}`}
                            onChange={(v) => updateAgent(agent.id, { maxTokens: v })}
                          />
                          <SliderParam
                            label="Top P"
                            value={agent.topP}
                            min={0} max={1} step={0.05}
                            onChange={(v) => updateAgent(agent.id, { topP: v })}
                          />
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </ScrollArea>
          </TabsContent>
        </Tabs>
      </div>
    </>
  );
}

/* ── Slider param ─────────────────────────────────────────── */

function SliderParam({
  label, value, min, max, step, display, onChange,
}: {
  label: string; value: number; min: number; max: number; step: number;
  display?: (v: number) => string; onChange: (v: number) => void;
}) {
  const fmt = display ?? ((v: number) => v.toFixed(2));
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono text-muted-foreground">{label}</span>
        <span className="text-[11px] font-mono text-foreground">{fmt(value)}</span>
      </div>
      <Slider
        value={[value]}
        min={min} max={max} step={step}
        onValueChange={([v]) => onChange(v)}
        className="[&_[role=slider]]:h-3 [&_[role=slider]]:w-3"
      />
    </div>
  );
}
