import { useState, useRef, useEffect, useCallback } from "react";
import { AppHeader } from "@/components/layout/AppHeader";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Send,
  Square,
  Bot,
  User,
  Wrench,
  MessageSquare,
  FileText,
  Radio,
  Lock,
  Pencil,
  Plus,
  MessageCircle,
} from "lucide-react";
import { sendChat, streamChat } from "@/api/client";
import type { ChatMessage, ChatRole } from "@/api/types";

/* ── Session type ─────────────────────────────────────────────── */
interface ChatSessionState {
  id: string;
  title: string;
  messages: ChatMessage[];
  createdAt: string;
}

/* ── Role badge ─────────────────────────────────────────────── */
function RoleBadge({ role }: { role: ChatRole }) {
  const config: Record<ChatRole, { icon: React.ReactNode; label: string; className: string }> = {
    user: { icon: <User className="h-3 w-3" />, label: "You", className: "bg-primary/10 text-primary" },
    assistant: { icon: <Bot className="h-3 w-3" />, label: "AI", className: "bg-accent/10 text-accent" },
    system: { icon: <Bot className="h-3 w-3" />, label: "System", className: "bg-muted text-muted-foreground" },
    tool: { icon: <Wrench className="h-3 w-3" />, label: "Tool", className: "bg-warning/10 text-warning" },
  };
  const c = config[role] ?? config.system;
  return (
    <Badge variant="outline" className={`gap-1 text-[10px] font-mono ${c.className}`}>
      {c.icon} {c.label}
    </Badge>
  );
}

/* ── Mock streaming log entries ─────────────────────────────── */
interface LogEntry {
  id: string;
  timestamp: string;
  level: "info" | "warn" | "error" | "debug";
  source: string;
  message: string;
}

const mockLogs: LogEntry[] = [
  { id: "1", timestamp: "14:32:01.203", level: "info", source: "llm.gateway", message: "Session abc-123 started, model: mistral-7b-instruct" },
  { id: "2", timestamp: "14:32:01.210", level: "debug", source: "prompt.engine", message: "Injecting immutable system prompt (1.2KB)" },
  { id: "3", timestamp: "14:32:01.215", level: "debug", source: "prompt.engine", message: "Appending tunable context: temperature=0.7, top_p=0.9" },
  { id: "4", timestamp: "14:32:01.340", level: "info", source: "llm.stream", message: "First token received in 125ms" },
  { id: "5", timestamp: "14:32:02.102", level: "info", source: "tool.dispatch", message: "Tool call detected: wg_peer_list()" },
  { id: "6", timestamp: "14:32:02.450", level: "info", source: "dbus.proxy", message: "→ org.freedesktop.WireGuard.ListPeers() → 3 peers" },
  { id: "7", timestamp: "14:32:02.455", level: "debug", source: "tool.dispatch", message: "Tool result injected into context (248 tokens)" },
  { id: "8", timestamp: "14:32:03.880", level: "info", source: "llm.stream", message: "Stream complete: 342 tokens in 2.54s (134 t/s)" },
  { id: "9", timestamp: "14:32:03.885", level: "info", source: "audit.chain", message: "Interaction logged → block #44201" },
  { id: "10", timestamp: "14:33:15.001", level: "warn", source: "llm.gateway", message: "Token budget at 78% for session abc-123" },
  { id: "11", timestamp: "14:34:02.100", level: "error", source: "tool.dispatch", message: "Tool timeout: container_stats() exceeded 5000ms" },
  { id: "12", timestamp: "14:34:02.105", level: "info", source: "llm.stream", message: "Retry with fallback response for failed tool call" },
];

/* ── Immutable system prompt ────────────────────────────────── */
const immutablePrompt = `You are the op-dbus AI assistant for the GhostBridge system.

CORE RULES (immutable):
1. Never execute destructive operations without explicit user confirmation.
2. All actions must be logged to the blockchain audit trail.
3. You may only interact with D-Bus objects exposed on the session bus.
4. Respect human-in-the-loop: suggest actions, never auto-execute.
5. Do not disclose internal system paths, keys, or credentials.
6. Limit tool calls to registered D-Bus methods only.
7. If a tool call fails, report the error — do not retry silently.`;

/* ── Default tunable prompt ─────────────────────────────────── */
const defaultTunablePrompt = `CONTEXT:
- System: GhostBridge privacy infrastructure
- Services: dinit-managed (WireGuard, Incus, OVS)
- Available tools: All registered D-Bus objects

BEHAVIOR:
- Tone: concise, technical, professional
- Temperature: 0.7
- Max tokens: 4096
- Prefer structured output (tables, lists) when presenting data
- Include D-Bus object paths when referencing system objects`;

/* ── Tab: Chat ──────────────────────────────────────────────── */
function ChatTab({
  sessions,
  activeSessionId,
  onSessionChange,
  onNewSession,
  onUpdateSessionMessages
}: {
  sessions: ChatSessionState[];
  activeSessionId: string | null;
  onSessionChange: (id: string) => void;
  onNewSession: () => void;
  onUpdateSessionMessages: (sessionId: string, messages: ChatMessage[]) => void;
}) {
  const activeSession = sessions.find(s => s.id === activeSessionId);
  const messages = activeSession?.messages ?? [];
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [streamText, setStreamText] = useState("");
  const abortRef = useRef<AbortController | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamText]);

  const updateMessages = useCallback((newMessages: ChatMessage[]) => {
    if (activeSessionId) {
      onUpdateSessionMessages(activeSessionId, newMessages);
    }
  }, [activeSessionId, onUpdateSessionMessages]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    if (!text || streaming || !activeSessionId) return;

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: text,
      timestamp: new Date().toISOString(),
    };

    const newMessages = [...messages, userMsg];
    updateMessages(newMessages);
    setInput("");
    setStreaming(true);
    setStreamText("");

    try {
      let chunks = "";
      abortRef.current = streamChat(
        text,
        activeSessionId,
        (chunk) => {
          chunks += chunk;
          setStreamText(chunks);
        },
        () => {
          if (chunks) {
            const assistantMsg: ChatMessage = {
              id: crypto.randomUUID(),
              role: "assistant",
              content: chunks,
              timestamp: new Date().toISOString(),
            };
            updateMessages([...newMessages, assistantMsg]);
          }
          setStreamText("");
          setStreaming(false);
        }
      );
    } catch {
      try {
        const res = await sendChat(text, activeSessionId);
        updateMessages([...newMessages, res.message]);
      } catch (err) {
        const errorMsg: ChatMessage = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: `Error: ${err instanceof Error ? err.message : "Request failed"}`,
          timestamp: new Date().toISOString(),
        };
        updateMessages([...newMessages, errorMsg]);
      }
      setStreaming(false);
    }
  }, [input, streaming, activeSessionId, messages, updateMessages]);

  const handleStop = () => {
    abortRef.current?.abort();
    setStreaming(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Session selector bar */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border bg-muted/30 overflow-x-auto">
        <div className="flex items-center gap-1">
          <MessageCircle className="h-4 w-4 text-muted-foreground shrink-0" />
          <span className="text-xs font-mono text-muted-foreground shrink-0">Sessions</span>
        </div>
        <div className="flex-1 flex gap-1 min-w-0">
          {sessions.map((session) => (
            <button
              key={session.id}
              onClick={() => onSessionChange(session.id)}
              className={`px-2.5 py-1 rounded text-xs font-mono truncate max-w-[150px] transition-colors ${activeSessionId === session.id
                ? "bg-primary/10 text-primary border border-primary/30"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
                }`}
              title={session.title}
            >
              {session.title}
            </button>
          ))}
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 shrink-0"
          onClick={onNewSession}
          title="New session"
        >
          <Plus className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* Chat messages area */}
      <div className="flex-1 overflow-hidden">
        <ScrollArea className="h-full p-4">
          <div className="w-[85%] mx-auto space-y-4">
            {messages.length === 0 && !streaming && (
              <div className="text-center py-20">
                <Bot className="h-10 w-10 text-muted-foreground/30 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground font-mono">
                  Send a message to start a conversation
                </p>
              </div>
            )}
            {messages.map((msg) => (
              <div key={msg.id} className="flex gap-3">
                <div className="shrink-0 pt-0.5">
                  <RoleBadge role={msg.role} />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-foreground whitespace-pre-wrap break-words">
                    {msg.content}
                  </p>
                  {msg.tool_calls && msg.tool_calls.length > 0 && (
                    <div className="mt-2 space-y-1">
                      {msg.tool_calls.map((tc) => (
                        <div
                          key={tc.id}
                          className="font-mono text-[11px] rounded-md bg-muted px-2 py-1 text-muted-foreground"
                        >
                          <span className="text-accent">{tc.tool_name}</span>
                          {tc.result && (
                            <span className={tc.result.success ? "text-success" : "text-destructive"}>
                              {" "}→ {tc.result.success ? "ok" : "err"} ({tc.result.execution_time_ms}ms)
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
            {streaming && streamText && (
              <div className="flex gap-3">
                <div className="shrink-0 pt-0.5">
                  <RoleBadge role="assistant" />
                </div>
                <p className="text-sm text-foreground whitespace-pre-wrap break-words flex-1">
                  {streamText}
                  <span className="inline-block w-1.5 h-4 bg-primary/60 animate-pulse ml-0.5 align-text-bottom" />
                </p>
              </div>
            )}
            <div ref={scrollRef} />
          </div>
        </ScrollArea>
      </div>

      {/* Input area */}
      <div className="border-t border-border bg-background p-3">
        <div className="max-w-3xl mx-auto flex gap-2">
          <Textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Send a message…"
            className="min-h-[44px] max-h-32 resize-none bg-muted border-border font-mono text-sm"
            disabled={streaming}
          />
          {streaming ? (
            <Button
              variant="destructive"
              size="icon"
              className="shrink-0 h-[44px] w-[44px]"
              onClick={handleStop}
            >
              <Square className="h-4 w-4" />
            </Button>
          ) : (
            <Button
              size="icon"
              className="shrink-0 h-[44px] w-[44px]"
              onClick={handleSend}
              disabled={!input.trim()}
            >
              <Send className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

/* ── Tab: System Prompt ─────────────────────────────────────── */
function SystemPromptTab() {
  const [tunablePrompt, setTunablePrompt] = useState(defaultTunablePrompt);
  const [saved, setSaved] = useState(false);

  const handleSave = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex-1 overflow-auto p-4">
      <div className="max-w-3xl mx-auto space-y-6">
        {/* Immutable section */}
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Lock className="h-4 w-4 text-muted-foreground" />
            <h3 className="text-sm font-semibold text-foreground">Immutable Rules</h3>
            <Badge variant="outline" className="text-[10px] font-mono text-destructive border-destructive/30">
              read-only
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            Core safety constraints. Cannot be modified at runtime.
          </p>
          <div className="relative">
            <pre className="rounded-lg border border-border bg-muted/50 p-4 text-sm font-mono text-foreground/80 whitespace-pre-wrap select-text">
              {immutablePrompt}
            </pre>
            <div className="absolute inset-0 rounded-lg border-2 border-dashed border-destructive/10 pointer-events-none" />
          </div>
        </div>

        {/* Tunable section */}
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Pencil className="h-4 w-4 text-muted-foreground" />
            <h3 className="text-sm font-semibold text-foreground">Tunable Context</h3>
            <Badge variant="outline" className="text-[10px] font-mono text-primary border-primary/30">
              editable
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            Adjustable context, behavior hints, and parameters. Changes apply to the next message.
          </p>
          <Textarea
            value={tunablePrompt}
            onChange={(e) => setTunablePrompt(e.target.value)}
            className="min-h-[240px] resize-y bg-muted/30 border-border font-mono text-sm leading-relaxed"
          />
          <div className="flex items-center gap-3">
            <Button size="sm" onClick={handleSave}>
              {saved ? "Saved ✓" : "Save Changes"}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="text-muted-foreground"
              onClick={() => setTunablePrompt(defaultTunablePrompt)}
            >
              Reset to Default
            </Button>
            <span className="text-[10px] font-mono text-muted-foreground ml-auto">
              {tunablePrompt.length} chars · ~{Math.ceil(tunablePrompt.length / 4)} tokens
            </span>
          </div>
        </div>

        {/* Combined preview */}
        <div className="space-y-2 border-t border-border pt-4">
          <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-widest">
            Final Prompt Preview
          </h3>
          <div className="rounded-lg border border-border bg-card p-4 space-y-3">
            <div>
              <span className="text-[10px] font-mono text-destructive">IMMUTABLE</span>
              <p className="text-xs font-mono text-foreground/60 mt-1 whitespace-pre-wrap line-clamp-4">
                {immutablePrompt}
              </p>
            </div>
            <div className="border-t border-dashed border-border" />
            <div>
              <span className="text-[10px] font-mono text-primary">TUNABLE</span>
              <p className="text-xs font-mono text-foreground/60 mt-1 whitespace-pre-wrap line-clamp-4">
                {tunablePrompt}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ── Tab: Streaming Log ─────────────────────────────────────── */
function StreamingLogTab() {
  const [filter, setFilter] = useState<"all" | "info" | "warn" | "error" | "debug">("all");

  const levelColors: Record<string, string> = {
    info: "text-[hsl(var(--log-info))]",
    warn: "text-[hsl(var(--log-warn))]",
    error: "text-[hsl(var(--log-error))]",
    debug: "text-[hsl(var(--log-debug))]",
  };

  const levelBg: Record<string, string> = {
    info: "bg-[hsl(var(--log-info)/0.1)]",
    warn: "bg-[hsl(var(--log-warn)/0.1)]",
    error: "bg-[hsl(var(--log-error)/0.1)]",
    debug: "bg-[hsl(var(--log-debug)/0.1)]",
  };

  const filtered = filter === "all" ? mockLogs : mockLogs.filter((l) => l.level === filter);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Filter bar */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
        {(["all", "info", "warn", "error", "debug"] as const).map((lvl) => (
          <button
            key={lvl}
            onClick={() => setFilter(lvl)}
            className={`px-2.5 py-1 rounded text-[11px] font-mono transition-colors ${filter === lvl
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground hover:text-foreground"
              }`}
          >
            {lvl}
            {lvl !== "all" && (
              <span className="ml-1 text-muted-foreground/60">
                ({mockLogs.filter((l) => l.level === lvl).length})
              </span>
            )}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1.5">
          <div className="h-2 w-2 rounded-full bg-status-online animate-pulse" />
          <span className="text-[10px] font-mono text-muted-foreground">live</span>
        </div>
      </div>

      {/* Log entries */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-0.5">
          {filtered.map((log) => (
            <div
              key={log.id}
              className={`flex items-start gap-3 px-3 py-1.5 rounded text-xs font-mono ${levelBg[log.level]}`}
            >
              <span className="text-muted-foreground/60 shrink-0 w-[90px]">
                {log.timestamp}
              </span>
              <span className={`shrink-0 w-[42px] uppercase font-semibold ${levelColors[log.level]}`}>
                {log.level}
              </span>
              <span className="text-muted-foreground shrink-0 w-[120px] truncate">
                {log.source}
              </span>
              <span className="text-foreground/80 break-words min-w-0">
                {log.message}
              </span>
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

/* ── Main Page ──────────────────────────────────────────────── */
export default function ChatPage() {
  const [sessions, setSessions] = useState<ChatSessionState[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  // Initialize with a default session if none exist
  useEffect(() => {
    if (sessions.length === 0) {
      const newSession: ChatSessionState = {
        id: crypto.randomUUID(),
        title: "New Chat",
        messages: [],
        createdAt: new Date().toISOString(),
      };
      setSessions([newSession]);
      setActiveSessionId(newSession.id);
    }
  }, []);

  const handleNewSession = useCallback(() => {
    const newSession: ChatSessionState = {
      id: crypto.randomUUID(),
      title: `Chat ${sessions.length + 1}`,
      messages: [],
      createdAt: new Date().toISOString(),
    };
    setSessions((prev) => [...prev, newSession]);
    setActiveSessionId(newSession.id);
  }, [sessions.length]);

  const handleSessionChange = useCallback((id: string) => {
    setActiveSessionId(id);
  }, []);

  const handleUpdateSessionMessages = useCallback((sessionId: string, messages: ChatMessage[]) => {
    setSessions((prev) =>
      prev.map((s) =>
        s.id === sessionId ? { ...s, messages } : s
      )
    );
  }, []);

  return (
    <>
      <AppHeader title="Chat" subtitle="ai assistant" />
      <Tabs defaultValue="chat" className="flex-1 flex flex-col overflow-hidden">
        <div className="border-b border-border px-4">
          <TabsList className="bg-transparent h-10 gap-1">
            <TabsTrigger
              value="chat"
              className="gap-1.5 text-xs font-mono data-[state=active]:bg-muted data-[state=active]:text-foreground"
            >
              <MessageSquare className="h-3.5 w-3.5" />
              Chat
            </TabsTrigger>
            <TabsTrigger
              value="prompt"
              className="gap-1.5 text-xs font-mono data-[state=active]:bg-muted data-[state=active]:text-foreground"
            >
              <FileText className="h-3.5 w-3.5" />
              System Prompt
            </TabsTrigger>
            <TabsTrigger
              value="log"
              className="gap-1.5 text-xs font-mono data-[state=active]:bg-muted data-[state=active]:text-foreground"
            >
              <Radio className="h-3.5 w-3.5" />
              Stream Log
            </TabsTrigger>
          </TabsList>
        </div>

        <TabsContent value="chat" className="flex-1 h-0 flex flex-col overflow-hidden mt-0">
          <ChatTab
            sessions={sessions}
            activeSessionId={activeSessionId}
            onSessionChange={handleSessionChange}
            onNewSession={handleNewSession}
            onUpdateSessionMessages={handleUpdateSessionMessages}
          />
        </TabsContent>
        <TabsContent value="prompt" className="flex-1 h-0 overflow-auto mt-0">
          <SystemPromptTab />
        </TabsContent>
        <TabsContent value="log" className="flex-1 h-0 flex flex-col overflow-hidden mt-0">
          <StreamingLogTab />
        </TabsContent>
      </Tabs>
    </>
  );
}
