import { useState, useRef, useEffect } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Send,
  Square,
  Loader2,
  Bot,
  User,
  Wrench,
  AlertCircle,
  Terminal,
  MessageSquare,
  FileText,
  Lock,
  Pencil,
  Network,
  SlidersHorizontal,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import {
  sendChatMessage,
  getChatSessions,
  getChatMessages,
  getSchemaCatalog,
  getDashboardProjections,
  getSystemPrompt,
  updateSystemPrompt,
  getLlmStatus,
  getLlmProviders,
  getLlmModels,
  switchLlmProvider,
  switchLlmModel,
} from "@/lib/api";
import {
  parseGenerativeBlocks,
  GenerativeBlock,
} from "@/components/chat/GenerativeBlock";
import { JsonTree, tryParseJson } from "@/components/json/JsonTree";

interface LogEntry {
  id: number;
  timestamp: string;
  level: "info" | "warn" | "error" | "debug";
  source: string;
  message: string;
}

const levelColors: Record<string, string> = {
  info: "text-blue-600 dark:text-blue-400 bg-blue-500/10",
  warn: "text-yellow-600 dark:text-yellow-400 bg-yellow-500/10",
  error: "text-red-600 dark:text-red-400 bg-red-500/10",
  debug: "text-muted-foreground bg-muted",
};

interface Message {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  toolName?: string;
  timestamp: Date;
  status?: "streaming" | "done" | "aborted" | "error";
}

interface ChatSession {
  id: string;
  title: string;
  updated_at: string;
}

interface LlmModel {
  id: string;
  name?: string;
  description?: string;
  available?: boolean;
}

type ZeroclawFeature = "chat" | "models" | "tools" | "memory" | "logs" | "health";

const ANTIGRAVITY_PROVIDER = "antigravity";

const zeroclawFeatures: Array<{ id: ZeroclawFeature; label: string }> = [
  { id: "chat", label: "Chat" },
  { id: "models", label: "Models" },
  { id: "tools", label: "Tools" },
  { id: "memory", label: "Memory" },
  { id: "logs", label: "Logs" },
  { id: "health", label: "Health" },
];

function normalizeProviderList(status: any, providers: any): string[] {
  const fromProviders = Array.isArray(providers?.providers) ? providers.providers : [];
  const fromStatus = Array.isArray(status?.available_providers)
    ? status.available_providers
    : Array.isArray(status?.providers)
      ? status.providers
      : [];
  const current = status?.provider ?? providers?.current;
  return Array.from(new Set([current, ...fromProviders, ...fromStatus].filter(Boolean)));
}

function normalizeModelList(data: any): LlmModel[] {
  const models = Array.isArray(data?.models) ? data.models : Array.isArray(data?.data) ? data.data : [];
  return models
    .map((model: any) => {
      const id = model?.id ?? model?.name;
      if (!id) return null;
      return {
        id: String(id),
        name: String(model?.name ?? id),
        description: model?.description ? String(model.description) : undefined,
        available: model?.available ?? true,
      };
    })
    .filter(Boolean) as LlmModel[];
}

function MessageContent({
  content,
  onAction,
}: {
  content: string;
  onAction: (action: string, payload: unknown) => void;
}) {
  return (
    <div className="space-y-3">
      {parseGenerativeBlocks(content).map((seg, i) => {
        if (seg.type === "ui") {
          return <GenerativeBlock key={i} spec={seg.spec} onAction={onAction} />;
        }

        const parsed = tryParseJson(seg.text);
        if (parsed) {
          return (
            <div key={i} className="rounded-lg border border-border/60 bg-secondary/30 p-3">
              <div className="mb-2 flex items-center gap-2 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                <SlidersHorizontal className="h-3 w-3" />
                JSON Render
              </div>
              <JsonTree data={parsed} maxDepth={6} />
            </div>
          );
        }

        return (
          <div key={i} className="text-sm leading-relaxed whitespace-pre-wrap">
            {seg.text}
          </div>
        );
      })}
    </div>
  );
}

function projectionsToLogs(data: any, prev: LogEntry[]): LogEntry[] {
  if (!data || typeof data !== "object") return prev;

  const now = new Date();
  const ts =
    now.toTimeString().split(" ")[0] +
    "." +
    String(now.getMilliseconds()).padStart(3, "0");
  const baseId = prev.length > 0 ? prev[prev.length - 1].id : 0;
  const entries: LogEntry[] = [];

  const mem = data["system.memory"];
  if (mem) {
    const used =
      mem.memtotal && mem.memfree
        ? Math.round(
            ((parseInt(mem.memtotal) - parseInt(mem.memfree)) /
              parseInt(mem.memtotal)) *
              100,
          )
        : "—";
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.memory",
      message: `memtotal=${mem.memtotal ?? "?"} memfree=${mem.memfree ?? "?"} used=${used}%`,
    });
  }

  const load = data["system.load"];
  if (load) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: load.load1 > (load.count || 8) ? "warn" : "info",
      source: "system.load",
      message: `load1=${load.load1 ?? "?"} load5=${load.load5 ?? "?"} procs_running=${load.procs_running ?? "?"} procs_total=${load.procs_total ?? "?"}`,
    });
  }

  const cpu = data["system.cpu"];
  if (cpu) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.cpu",
      message: `cores=${cpu.count ?? "?"} model=${cpu.cpus?.[0]?.model_name ?? "?"}`,
    });
  }

  const net = data["system.network"];
  if (net && Array.isArray(net.interfaces)) {
    const ifaces = net.interfaces
      .map((i: any) => i.name)
      .filter(Boolean)
      .join(", ");
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.network",
      message: `interfaces=[${ifaces}]`,
    });
  }

  const procs = data["system.processes"];
  if (procs) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.processes",
      message: `total=${procs.processes ?? "?"} running=${procs.procs_running ?? "?"} blocked=${procs.procs_blocked ?? "?"}`,
    });
  }

  const fs = data["system.filesystems"];
  if (fs && Array.isArray(fs)) {
    entries.push({
      id: baseId + entries.length + 1,
      timestamp: ts,
      level: "info",
      source: "system.filesystems",
      message: `mounts=${fs.length}`,
    });
  }

  const combined = [...prev, ...entries];
  if (combined.length > 200) return combined.slice(combined.length - 200);
  return combined;
}

function SystemPromptView() {
  const [tunablePrompt, setTunablePrompt] = useState("");
  const [loaded, setLoaded] = useState(false);

  const promptQuery = useQuery({
    queryKey: ["systemPrompt"],
    queryFn: getSystemPrompt,
  });

  const saveMutation = useMutation({
    mutationFn: (p: string) => updateSystemPrompt(p),
  });

  useEffect(() => {
    if (promptQuery.data && !loaded) {
      setTunablePrompt(
        promptQuery.data?.tunable ?? promptQuery.data?.custom ?? "",
      );
      setLoaded(true);
    }
  }, [promptQuery.data, loaded]);

  const immutablePrompt =
    promptQuery.data?.immutable ??
    promptQuery.data?.system ??
    promptQuery.data?.prompt ??
    "";
  const charCount = tunablePrompt.length;

  return (
    <div className="flex flex-col h-full gap-4 overflow-auto">
      {/* Immutable Section */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Lock className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            Immutable (read-only)
          </span>
        </div>
        <div className="p-4 rounded-lg bg-secondary/50 border border-border/30">
          <pre className="whitespace-pre-wrap font-mono text-xs text-muted-foreground leading-relaxed">
            {promptQuery.isLoading
              ? "Loading..."
              : immutablePrompt || "No immutable prompt defined"}
          </pre>
        </div>
      </div>

      {/* Tunable Section */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2">
            <Pencil className="h-3.5 w-3.5 text-primary" />
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              Tunable (editable)
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              {charCount} chars
            </span>
            <Button
              size="sm"
              variant="outline"
              className="text-xs h-7"
              onClick={() => {
                setTunablePrompt("");
              }}
            >
              Reset
            </Button>
            <Button
              size="sm"
              className="text-xs h-7"
              onClick={() => saveMutation.mutate(tunablePrompt)}
              disabled={saveMutation.isPending}
            >
              {saveMutation.isPending ? "Saving..." : "Save"}
            </Button>
          </div>
        </div>
        <textarea
          value={tunablePrompt}
          onChange={(e) => setTunablePrompt(e.target.value)}
          className="flex-1 min-h-[200px] bg-secondary border border-border/30 rounded-lg p-3 resize-none font-mono text-sm outline-none focus:border-ring"
          placeholder="Add custom instructions, persona adjustments, or context here..."
        />
      </div>
    </div>
  );
}

export default function AssistantChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [selectedFeature, setSelectedFeature] = useState<ZeroclawFeature>("chat");
  const [selectedProvider, setSelectedProvider] = useState(ANTIGRAVITY_PROVIDER);
  const [selectedModel, setSelectedModel] = useState("");
  const [didPreferAntigravity, setDidPreferAntigravity] = useState(false);
  const [activeTab, setActiveTab] = useState<"chat" | "logs" | "system-prompt">(
    "chat",
  );
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const sessions = useQuery({
    queryKey: ["chatSessions"],
    queryFn: getChatSessions,
    refetchInterval: 30000,
  });

  const llmStatus = useQuery({
    queryKey: ["llmStatus"],
    queryFn: getLlmStatus,
    refetchInterval: 10000,
  });

  const llmProviders = useQuery({
    queryKey: ["llmProviders"],
    queryFn: getLlmProviders,
    refetchInterval: 30000,
  });

  const providerOptions = normalizeProviderList(llmStatus.data, llmProviders.data);
  const hasAntigravity = providerOptions.includes(ANTIGRAVITY_PROVIDER);
  const effectiveProvider =
    selectedProvider ||
    (hasAntigravity ? ANTIGRAVITY_PROVIDER : "") ||
    llmStatus.data?.provider ||
    llmProviders.data?.current ||
    "";

  const llmModels = useQuery({
    queryKey: ["llmModels", effectiveProvider],
    queryFn: () => getLlmModels(effectiveProvider || undefined),
    enabled: !!effectiveProvider,
    refetchInterval: 30000,
  });

  // Schema — single source of truth from shared memory (/dev/shm/plugin_schemas.json)
  const schemaCatalog = useQuery({
    queryKey: ["schemaCatalog"],
    queryFn: getSchemaCatalog,
    refetchInterval: 5000,
  });

  const history = useQuery({
    queryKey: ["chatMessages", activeSessionId],
    queryFn: () => getChatMessages(activeSessionId!),
    enabled: !!activeSessionId,
  });

  // Live D-Bus projection log feed (shared with LogsPage)
  const projections = useQuery({
    queryKey: ["dashboardProjections"],
    queryFn: getDashboardProjections,
    refetchInterval: 5000,
  });

  useEffect(() => {
    if (projections.data) {
      setLogEntries((prev) => projectionsToLogs(projections.data, prev));
    }
  }, [projections.data]);

  useEffect(() => {
    if (history.data && Array.isArray(history.data.messages)) {
      const mapped: Message[] = history.data.messages.map((m: any) => ({
        id: m.id || String(Date.now() + Math.random()),
        role: m.role || "assistant",
        content: m.content || "",
        toolName: m.tool_name,
        timestamp: new Date(m.timestamp || Date.now()),
        status: "done",
      }));
      setMessages(mapped);
    }
  }, [history.data]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const modelOptions = normalizeModelList(llmModels.data);

  const providerMutation = useMutation({
    mutationFn: switchLlmProvider,
    onSuccess: (_data, provider) => {
      setSelectedProvider(provider);
      setSelectedModel("");
    },
    onSettled: () => {
      llmStatus.refetch();
      llmModels.refetch();
    },
  });

  const modelMutation = useMutation({
    mutationFn: switchLlmModel,
    onSuccess: (_data, model) => setSelectedModel(model),
    onSettled: () => llmStatus.refetch(),
  });

  useEffect(() => {
    if (
      didPreferAntigravity ||
      !hasAntigravity ||
      llmStatus.data?.provider === ANTIGRAVITY_PROVIDER ||
      providerMutation.isPending
    ) {
      return;
    }

    setDidPreferAntigravity(true);
    providerMutation.mutate(ANTIGRAVITY_PROVIDER);
  }, [
    didPreferAntigravity,
    hasAntigravity,
    llmStatus.data?.provider,
    providerMutation,
  ]);

  useEffect(() => {
    if (selectedModel || modelOptions.length === 0 || modelMutation.isPending) {
      return;
    }

    const currentModel = llmModels.data?.current;
    const currentModelIsAvailable = modelOptions.some((model) => model.id === currentModel);
    const nextModel = currentModelIsAvailable ? currentModel : modelOptions[0].id;

    setSelectedModel(nextModel);
    if (nextModel && nextModel !== llmStatus.data?.model) {
      modelMutation.mutate(nextModel);
    }
  }, [
    llmModels.data?.current,
    llmStatus.data?.model,
    modelMutation,
    modelOptions,
    selectedModel,
  ]);

  const sendMessage = useMutation({
    mutationFn: sendChatMessage,
    onMutate: async (variables) => {
      const userMsg: Message = {
        id: Date.now().toString(),
        role: "user",
        content: variables.message,
        timestamp: new Date(),
      };
      setMessages((prev) => [...prev, userMsg]);
      setInput("");
    },
    onSuccess: (data) => {
      if (data?.session_id && !activeSessionId) {
        setActiveSessionId(data.session_id);
      }
      const reply: Message = {
        id: data?.id || (Date.now() + 1).toString(),
        role: data?.role || "assistant",
        content: data?.content || "(no response)",
        toolName: data?.tool_name,
        timestamp: new Date(),
        status: "done",
      };
      setMessages((prev) => [...prev, reply]);
    },
    onError: (err: any) => {
      const errorMsg: Message = {
        id: (Date.now() + 1).toString(),
        role: "assistant",
        content: `Error: ${err.message || "Request failed"}`,
        timestamp: new Date(),
        status: "error",
      };
      setMessages((prev) => [...prev, errorMsg]);
    },
  });

  const handleSend = () => {
    if (!input.trim() || sendMessage.isPending) return;
    sendMessage.mutate({
      session_id: activeSessionId || undefined,
      message: input.trim(),
      model: selectedModel || undefined,
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const isLoading = sendMessage.isPending;
  const isError = sessions.isError || history.isError || llmStatus.isError;
  const isConnected = !llmStatus.isError;

  // Extract actual error messages for display
  const errorMsg =
    (sessions.error instanceof Error ? sessions.error.message : undefined) ||
    (history.error instanceof Error ? history.error.message : undefined) ||
    (projections.error instanceof Error
      ? projections.error.message
      : undefined);

  return (
    <div className="flex flex-col h-[calc(100vh-theme(spacing.12)-theme(spacing.12))] w-full">
      {/* Chat header */}
      <div className="flex items-center justify-between pb-4 border-b border-border mb-4">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-primary/10 flex items-center justify-center">
            <Bot className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-semibold text-foreground">Antigravity Chat</h1>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span className="flex items-center gap-1">
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    isConnected ? "bg-green-500" : "bg-red-500",
                  )}
                />
                {isConnected ? "Connected" : "Offline"}
              </span>
              <span>·</span>
              <span>Zeroclaw wg-xray route · D-Bus authority</span>
              {llmStatus.data?.provider && (
                <>
                  <span>·</span>
                  <span>{llmStatus.data.provider}:{llmStatus.data.model || "model"}</span>
                </>
              )}
              {schemaCatalog.data && (
                <>
                  <span>·</span>
                  <span className="text-xs text-emerald-600 dark:text-emerald-400">
                    Schema: {Object.keys(schemaCatalog.data).length} types
                  </span>
                </>
              )}
              {schemaCatalog.isError && (
                <>
                  <span>·</span>
                  <span className="text-xs text-amber-500">No schema SHM</span>
                </>
              )}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            className="text-xs h-7"
            onClick={() => {
              setActiveSessionId(null);
              setMessages([]);
            }}
          >
            New Session
          </Button>
        </div>
      </div>

      {/* Antigravity control surface */}
      <div className="mb-4 rounded-lg border border-border bg-card/60 p-3">
        <div className="mb-3 flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
          <Network className="h-3.5 w-3.5 text-primary" />
          Antigravity Interface
        </div>
        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1.4fr)]">
          <div>
            <div className="mb-1 text-[10px] text-muted-foreground uppercase tracking-wider">Feature</div>
            <Select value={selectedFeature} onValueChange={(value) => setSelectedFeature(value as ZeroclawFeature)}>
              <SelectTrigger className="h-8 bg-secondary border-border text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {zeroclawFeatures.map((feature) => (
                  <SelectItem key={feature.id} value={feature.id} className="text-xs">
                    {feature.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div>
            <div className="mb-1 text-[10px] text-muted-foreground uppercase tracking-wider">Provider</div>
            <Select
              value={selectedProvider || effectiveProvider}
              onValueChange={(value) => providerMutation.mutate(value)}
              disabled={providerMutation.isPending || providerOptions.length === 0}
            >
              <SelectTrigger className="h-8 bg-secondary border-border text-xs">
                <SelectValue placeholder={llmProviders.isLoading ? "Loading..." : "Provider"} />
              </SelectTrigger>
              <SelectContent>
                {providerOptions.map((provider) => (
                  <SelectItem key={provider} value={provider} className="text-xs">
                    {provider}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div>
            <div className="mb-1 text-[10px] text-muted-foreground uppercase tracking-wider">Model / Route</div>
            <Select
              value={selectedModel || llmStatus.data?.model || ""}
              onValueChange={(value) => modelMutation.mutate(value)}
              disabled={modelMutation.isPending || modelOptions.length === 0}
            >
              <SelectTrigger className="h-8 bg-secondary border-border text-xs">
                <SelectValue placeholder={llmModels.isLoading ? "Loading..." : "Model"} />
              </SelectTrigger>
              <SelectContent>
                {modelOptions.map((model) => (
                  <SelectItem
                    key={model.id}
                    value={model.id}
                    className="text-xs"
                    disabled={model.available === false}
                  >
                    {model.name ?? model.id}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </div>

      {/* Tab bar: Chat | Logs */}
      <div className="flex border-b border-border mb-4">
        <button
          onClick={() => setActiveTab("chat")}
          className={cn(
            "px-4 py-2 text-xs font-medium flex items-center gap-1.5 border-b-2 transition-colors",
            activeTab === "chat"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground",
          )}
        >
          <MessageSquare className="h-3.5 w-3.5" />
          Chat
        </button>
        <button
          onClick={() => setActiveTab("logs")}
          className={cn(
            "px-4 py-2 text-xs font-medium flex items-center gap-1.5 border-b-2 transition-colors",
            activeTab === "logs"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground",
          )}
        >
          <Terminal className="h-3.5 w-3.5" />
          Logs
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              projections.isFetching
                ? "bg-green-500 animate-pulse"
                : "bg-muted-foreground",
            )}
          />
        </button>
        <button
          onClick={() => setActiveTab("system-prompt")}
          className={cn(
            "px-4 py-2 text-xs font-medium flex items-center gap-1.5 border-b-2 transition-colors",
            activeTab === "system-prompt"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground",
          )}
        >
          <FileText className="h-3.5 w-3.5" />
          System Prompt
        </button>
      </div>

      {/* Session selector (if multiple sessions exist) */}
      {sessions.data?.sessions?.length > 0 && (
        <div className="flex gap-2 mb-4 overflow-x-auto pb-1">
          <button
            onClick={() => setActiveSessionId(null)}
            className={cn(
              "px-3 py-1 rounded-md text-xs font-medium border transition-colors shrink-0",
              !activeSessionId
                ? "bg-primary text-primary-foreground border-primary"
                : "bg-card border-border text-muted-foreground hover:text-foreground",
            )}
          >
            New
          </button>
          {sessions.data.sessions.map((s: ChatSession) => (
            <button
              key={s.id}
              onClick={() => setActiveSessionId(s.id)}
              className={cn(
                "px-3 py-1 rounded-md text-xs font-medium border transition-colors shrink-0 truncate max-w-[200px]",
                activeSessionId === s.id
                  ? "bg-primary text-primary-foreground border-primary"
                  : "bg-card border-border text-muted-foreground hover:text-foreground",
              )}
            >
              {s.title || s.id.slice(0, 8)}
            </button>
          ))}
        </div>
      )}

      {activeTab === "chat" && (
        <>
          {/* Messages */}
          <div className="flex-1 overflow-auto scrollbar-thin space-y-4 pr-2">
            {messages.length === 0 && !isLoading && (
              <div className="flex flex-col items-start h-full text-muted-foreground gap-3 px-4">
                <Bot className="h-10 w-10 opacity-30" />
                <p className="text-sm">
                  Start an Antigravity chat session.
                </p>
                <p className="text-xs opacity-60">
                  Provider routes include Antigravity and Factory through Zeroclaw.
                </p>
              </div>
            )}

            {messages.map((msg) => (
              <div
                key={msg.id}
                className={cn(
                  "flex gap-3",
                  msg.role === "user" && "justify-end",
                )}
              >
                {msg.role !== "user" && (
                  <div
                    className={cn(
                      "h-7 w-7 rounded-md flex items-center justify-center shrink-0 mt-0.5",
                      msg.role === "assistant" ? "bg-primary/10" : "bg-muted",
                    )}
                  >
                    {msg.role === "assistant" ? (
                      <Bot className="h-4 w-4 text-primary" />
                    ) : (
                      <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
                    )}
                  </div>
                )}
                <div
                  className={cn(
                    "max-w-[80%] rounded-lg px-4 py-2.5",
                    msg.role === "user"
                      ? "bg-primary text-primary-foreground"
                      : msg.role === "tool"
                        ? "bg-muted border border-border font-mono text-xs"
                        : msg.status === "error"
                          ? "bg-destructive/10 border border-destructive/30 text-destructive"
                          : "bg-card border border-border",
                  )}
                >
                  {msg.role === "tool" && (
                    <div className="text-[10px] text-muted-foreground mb-1.5 font-sans flex items-center gap-1">
                      <Wrench className="h-3 w-3" />
                      {msg.toolName}
                    </div>
                  )}
                  {msg.role === "user" || msg.role === "tool" ? (
                    <div
                      className={cn(
                        "text-sm leading-relaxed whitespace-pre-wrap",
                        msg.role === "tool" && "text-muted-foreground",
                      )}
                    >
                      {msg.content}
                    </div>
                  ) : (
                    <MessageContent
                      content={msg.content}
                      onAction={(action, payload) => {
                        const actionMsg: Message = {
                          id: Date.now().toString(),
                          role: "user",
                          content: `[Action: ${action}] ${JSON.stringify(payload)}`,
                          timestamp: new Date(),
                        };
                        setMessages((prev) => [...prev, actionMsg]);
                      }}
                    />
                  )}
                  <div
                    className={cn(
                      "text-[10px] mt-1.5",
                      msg.role === "user"
                        ? "text-primary-foreground/60"
                        : "text-muted-foreground",
                    )}
                  >
                    {msg.timestamp.toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </div>
                </div>
                {msg.role === "user" && (
                  <div className="h-7 w-7 rounded-md bg-primary/20 flex items-center justify-center shrink-0 mt-0.5">
                    <User className="h-4 w-4 text-primary" />
                  </div>
                )}
              </div>
            ))}

            {isLoading && (
              <div className="flex gap-3">
                <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
                  <Bot className="h-4 w-4 text-primary" />
                </div>
                <div className="bg-card border border-border rounded-lg px-4 py-3">
                  <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                </div>
              </div>
            )}
            <div ref={bottomRef} />
          </div>

          {/* Input */}
          <div className="pt-4 border-t border-border mt-4">
            {isError && (
              <div className="flex items-start gap-2 text-xs text-amber-600 dark:text-amber-400 bg-amber-500/10 rounded-md px-2 py-1.5 mb-2">
                <AlertCircle className="h-3.5 w-3.5 mt-0.5 shrink-0" />
                <span className="text-left">
                  {(sessions.error instanceof Error
                    ? sessions.error.message
                    : "") ||
                    (history.error instanceof Error
                      ? history.error.message
                      : "") ||
                    "Session sync delayed — chat is still functional."}
                </span>
              </div>
            )}
            <div className="relative flex items-end gap-2 bg-card border border-border rounded-lg p-2">
              <textarea
                ref={inputRef}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder={
                  isConnected
                    ? "Message Antigravity... (Shift+Enter for new line)"
                    : "Antigravity gateway unavailable..."
                }
                rows={1}
                disabled={!isConnected}
                className="flex-1 resize-none bg-transparent text-sm text-foreground placeholder:text-muted-foreground outline-none min-h-[36px] max-h-[120px] py-2 px-2 disabled:opacity-50"
              />
              {isLoading ? (
                <Button
                  size="icon"
                  variant="destructive"
                  className="h-8 w-8 shrink-0"
                  onClick={() => sendMessage.reset()}
                >
                  <Square className="h-3.5 w-3.5" />
                </Button>
              ) : (
                <Button
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  onClick={handleSend}
                  disabled={!input.trim() || !isConnected}
                >
                  <Send className="h-3.5 w-3.5" />
                </Button>
              )}
            </div>
            <p className="text-[10px] text-muted-foreground text-center mt-2">
              {isConnected
                ? "Antigravity provider · Zeroclaw wg-xray gateway · schema-driven D-Bus control"
                : "Antigravity gateway unavailable · verify wg-xray and identity sled in /dev/shm"}
            </p>
          </div>
        </>
      )}

      {activeTab === "logs" && (
        <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-auto bg-card border border-border rounded-lg font-mono text-xs">
            <div className="p-1">
              {logEntries.length === 0 && (
                <div className="px-3 py-8 text-center text-muted-foreground">
                  {projections.isLoading
                    ? "Loading projections..."
                    : "No projection data yet. Waiting for D-Bus tree..."}
                </div>
              )}
              {logEntries.map((log) => (
                <div
                  key={log.id}
                  className="flex items-start gap-2 px-3 py-1.5 hover:bg-accent/50 transition-colors rounded"
                >
                  <span className="text-muted-foreground shrink-0 w-20">
                    {log.timestamp}
                  </span>
                  <span
                    className={cn(
                      "text-[10px] h-4 px-1.5 rounded flex items-center shrink-0",
                      levelColors[log.level],
                    )}
                  >
                    {log.level.toUpperCase()}
                  </span>
                  <span className="text-primary shrink-0 w-24 truncate">
                    {log.source}
                  </span>
                  <span className="text-foreground break-all">
                    {log.message}
                  </span>
                </div>
              ))}
            </div>
          </div>
          <div className="flex items-center justify-between text-[10px] text-muted-foreground pt-2">
            <span>
              {logEntries.length} entries ·{" "}
              {projections.data ? Object.keys(projections.data).length : 0}{" "}
              schemas
            </span>
            <span className="flex items-center gap-1">
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  projections.isFetching
                    ? "bg-green-500 animate-pulse"
                    : "bg-muted-foreground",
                )}
              />
              {projections.isFetching ? "Fetching..." : "Live"}
            </span>
          </div>
        </div>
      )}

      {activeTab === "system-prompt" && (
        <div className="flex-1 flex flex-col overflow-hidden">
          <SystemPromptView />
        </div>
      )}
    </div>
  );
}
