// File: crates/op-web/ui/src/pages/AccountabilityPage.tsx

import { useState, useRef, useEffect, useCallback } from "react";
import { Pill, StatusDot } from "@/components/shell/Primitives";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { ScrollArea } from "@/components/ui/scroll-area";
import { eventChainService } from "@/grpc/client";
import { structToObject } from "@/grpc/google/protobuf/struct";
import { useEventStore } from "@/stores/event-store";
import { cn } from "@/lib/utils";
import {
  Send,
  Loader2,
  AlertTriangle,
  ShieldCheck,
  Database,
  RefreshCcw,
} from "lucide-react";

/* ── Types ─────────────────────────────────────────────── */

interface SessionSled {
  traceId: string;
  footprint: string;
  mutationIndex: number;
}

interface ChatMessage {
  id: string;
  role: "system" | "user" | "assistant";
  content: string;
  timestamp: number;
}

type ComplianceStatus = "pass" | "warn" | "fail" | "pending";

interface QdrantTraceEntry {
  id: string;
  pointId: string;
  score: number;
  summary: string;
  decision: string;
  source: string;
  outcomeClass: string;
  toolsConsulted: string[];
  complianceStatus: ComplianceStatus;
  complianceNote: string;
  payload: Record<string, unknown>;
}

/* ── Helpers ───────────────────────────────────────────── */

const COMPLIANCE_STYLE: Record<
  ComplianceStatus,
  { text: string; dot: "ok" | "warn" | "error" | "offline" }
> = {
  pass: { text: "text-ok", dot: "ok" },
  warn: { text: "text-warn", dot: "warn" },
  fail: { text: "text-danger", dot: "error" },
  pending: { text: "text-muted-foreground", dot: "offline" },
};

const DEFAULT_TRACE_LIMIT = 5;

function getString(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return typeof value === "string" ? value : "";
}

function getNumber(
  payload: Record<string, unknown>,
  key: string,
): number | null {
  const value = payload[key];
  return typeof value === "number" ? value : null;
}

function getStringArray(
  payload: Record<string, unknown>,
  key: string,
): string[] {
  const value = payload[key];
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is string => typeof item === "string");
}

function toComplianceStatus(
  payload: Record<string, unknown>,
): ComplianceStatus {
  if (payload.piiFlagged === true || payload.pii_flagged === true) {
    return "warn";
  }

  const outcome =
    getString(payload, "outcomeClass") || getString(payload, "outcome_class");
  switch (outcome.toLowerCase()) {
    case "allow":
    case "pass":
      return "pass";
    case "warn":
    case "review":
      return "warn";
    case "deny":
    case "fail":
      return "fail";
    default:
      return "pending";
  }
}

function toComplianceNote(payload: Record<string, unknown>): string {
  return (
    getString(payload, "complianceNote") ||
    getString(payload, "compliance_note") ||
    getString(payload, "decisionOutput") ||
    getString(payload, "decision_output") ||
    "Compliance detail unavailable."
  );
}

function toTraceEntry(match: {
  pointId: string;
  score: number;
  payload?: Record<string, unknown>;
}): QdrantTraceEntry {
  const payload = match.payload ?? {};
  const summary =
    getString(payload, "reasoningSummary") ||
    getString(payload, "reasoning_summary") ||
    getString(payload, "summary") ||
    "Schema-derived semantic match";
  const decision =
    getString(payload, "decisionOutput") ||
    getString(payload, "decision_output");
  const source =
    getString(payload, "source") ||
    getString(payload, "pluginId") ||
    getString(payload, "plugin_id") ||
    "ctl_plane_reasoning_episodes";
  const outcomeClass =
    getString(payload, "outcomeClass") ||
    getString(payload, "outcome_class") ||
    "pending";
  const toolsConsulted =
    getStringArray(payload, "toolsConsulted").length > 0
      ? getStringArray(payload, "toolsConsulted")
      : getStringArray(payload, "tools_consulted");

  return {
    id: `${match.pointId || "semantic-match"}-${match.score}`,
    pointId: match.pointId,
    score: match.score,
    summary,
    decision,
    source,
    outcomeClass,
    toolsConsulted,
    complianceStatus: toComplianceStatus(payload),
    complianceNote: toComplianceNote(payload),
    payload,
  };
}

/* ── Component ─────────────────────────────────────────── */

export const AccountabilityPage: React.FC = () => {
  const { connected } = useEventStore();

  /* Session Sled — binds to Ghostbridge identity headers */
  const [activeSled, setActiveSled] = useState<SessionSled | null>(null);

  /* Top Pane: Chat state */
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "sys-1",
      role: "system",
      content: "Snowball session established. Awaiting agent interaction.",
      timestamp: Date.now(),
    },
  ]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const threadRef = useRef<HTMLDivElement>(null);

  /* Bottom Pane: Qdrant trace state */
  const [traceResults, setTraceResults] = useState<QdrantTraceEntry[]>([]);
  const [traceLoading, setTraceLoading] = useState(false);
  const [traceError, setTraceError] = useState<string | null>(null);

  /* Scroll chat to bottom on new messages */
  const scrollToBottom = useCallback(() => {
    threadRef.current?.scrollTo({
      top: threadRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, []);
  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  /* Bind the Session Sled from Ghostbridge headers */
  useEffect(() => {
    const traceId = sessionStorage.getItem("X-Ghostbridge-Trace-ID") || "";
    const footprint = sessionStorage.getItem("X-Ghostbridge-Footprint") || "";
    if (traceId || footprint) {
      setActiveSled({ traceId, footprint, mutationIndex: 0 });
    }
  }, []);

  /* ── Chat send handler — wired to the zeroclaw assistant endpoint ── */
  const handleSend = async () => {
    if (!draft.trim() || !connected) return;
    const userMsg: ChatMessage = {
      id: `msg-${Date.now()}`,
      role: "user",
      content: draft.trim(),
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);
    setDraft("");
    setSending(true);

    try {
      const response = await fetch("/api/assistant/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message: userMsg.content,
          session_id: activeSled?.traceId,
        }),
      });

      const data = await response.text();
      let content = data;
      try {
        const parsed = JSON.parse(data);
        content =
          typeof parsed.content === "string"
            ? parsed.content
            : typeof parsed.response === "string"
              ? parsed.response
              : typeof parsed.message === "string"
                ? parsed.message
                : data;
      } catch {
        // Not JSON — display the raw response text.
      }

      const assistantMsg: ChatMessage = {
        id: `msg-${Date.now()}-resp`,
        role: "assistant",
        content,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, assistantMsg]);
      setActiveSled((prev) =>
        prev ? { ...prev, mutationIndex: prev.mutationIndex + 1 } : prev,
      );
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : "Chat request failed";
      setMessages((prev) => [
        ...prev,
        {
          id: `msg-${Date.now()}-err`,
          role: "assistant",
          content: `Error: ${errorMsg}`,
          timestamp: Date.now(),
        },
      ]);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  /* ── Qdrant semantic trace refresh ── */
  const handleTraceSearch = useCallback(async () => {
    setTraceLoading(true);
    setTraceError(null);

    try {
      const response = await eventChainService.searchSemanticTrace({
        limit: DEFAULT_TRACE_LIMIT,
      });
      setActiveSled((prev) => ({
        traceId: response.traceId || prev?.traceId || "",
        footprint: prev?.footprint || "",
        mutationIndex: response.mutationIndex ?? prev?.mutationIndex ?? 0,
      }));
      setTraceResults(
        (response.matches ?? []).map((match) =>
          toTraceEntry({
            pointId: match.pointId,
            score: match.score,
            payload: structToObject(match.payload),
          }),
        ),
      );
    } catch (error) {
      setTraceError(
        error instanceof Error
          ? error.message
          : "Semantic trace request failed",
      );
      setTraceResults([]);
    } finally {
      setTraceLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!activeSled?.traceId) return;
    void handleTraceSearch();
  }, [activeSled?.traceId, handleTraceSearch]);

  /* ── Render ── */
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border shrink-0">
        <div className="flex items-center justify-between gap-4 px-4 py-3">
          <div>
            <h1 className="text-[26px] font-bold tracking-tight leading-tight text-foreground">
              A.N.N.A. Scribe // Accountability Loop
            </h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              Two-pane confrontation: Agent Dialog above, Qdrant Semantic Trace
              below.
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Pill variant={connected ? "ok" : "danger"}>
              <StatusDot status={connected ? "ok" : "error"} />
              <span>Stream</span>
              <span className="font-mono">
                {connected ? "Live" : "Offline"}
              </span>
            </Pill>
            {activeSled ? (
              <Pill variant="default">
                <span className="font-mono text-[11px] truncate max-w-[200px]">
                  Trace: {activeSled.traceId || "—"} | Idx:{" "}
                  {activeSled.mutationIndex}
                </span>
              </Pill>
            ) : (
              <Pill variant="warn">
                <StatusDot status="warn" />
                <span>Awaiting Notary…</span>
              </Pill>
            )}
          </div>
        </div>
      </div>

      {/* Split panes */}
      <div className="flex flex-col flex-1 min-h-0">
        {/* TOP PANE: Agent Dialog */}
        <div className="flex-1 flex flex-col min-h-0 border-b border-border">
          <div className="px-4 py-1.5 border-b border-border bg-[hsl(var(--bg-elevated))] text-xs font-medium text-muted-foreground uppercase tracking-wider">
            Agent Dialog
          </div>
          <div
            ref={threadRef}
            className="flex-1 overflow-y-auto px-4 py-4 space-y-3"
            role="log"
          >
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
                      "h-7 w-7 rounded-full flex items-center justify-center text-[10px] font-bold shrink-0",
                      msg.role === "system"
                        ? "bg-muted text-muted-foreground"
                        : "bg-primary/20 text-primary",
                    )}
                  >
                    {msg.role === "system" ? "SYS" : "AI"}
                  </div>
                )}
                <div
                  className={cn(
                    "rounded-lg px-3 py-2 text-sm max-w-[75%]",
                    msg.role === "user"
                      ? "bg-primary text-primary-foreground"
                      : "bg-card border border-border text-foreground",
                  )}
                >
                  {msg.content}
                </div>
              </div>
            ))}
            {sending && (
              <div className="flex gap-3">
                <div className="h-7 w-7 rounded-full bg-primary/20 flex items-center justify-center text-[10px] font-bold text-primary shrink-0">
                  AI
                </div>
                <div className="rounded-lg bg-card border border-border px-3 py-2 flex items-center gap-2">
                  <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                  <span className="text-sm text-muted-foreground">
                    Processing…
                  </span>
                </div>
              </div>
            )}
          </div>

          {/* Composer */}
          <div className="border-t border-border px-4 py-2.5 shrink-0 bg-background">
            {!connected && (
              <div className="flex items-center gap-2 text-xs text-danger mb-2">
                <AlertTriangle className="h-3.5 w-3.5" />
                Connect to the gRPC-Web stream to interact.
              </div>
            )}
            <div className="flex gap-2">
              <textarea
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={handleKeyDown}
                disabled={!connected}
                placeholder={
                  connected
                    ? "Confront or command the agent… (↩ send)"
                    : "Stream offline…"
                }
                className="flex-1 px-3 py-2 rounded-lg border border-input bg-card text-sm resize-none min-h-[40px] max-h-32 focus:border-ring focus:ring-1 focus:ring-ring outline-none transition-colors font-sans"
                rows={1}
              />
              <button
                onClick={handleSend}
                disabled={!connected || !draft.trim()}
                className="px-3 py-2 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center gap-1.5"
              >
                <Send className="h-3.5 w-3.5" />
                Send
              </button>
            </div>
          </div>
        </div>

        {/* BOTTOM PANE: Qdrant Semantic Memory Trace */}
        <div className="flex-1 flex flex-col min-h-0">
          <div className="px-4 py-1.5 border-b border-border bg-[hsl(var(--bg-elevated))] flex items-center justify-between">
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              Qdrant Semantic Memory Trace
            </span>
            <span className="text-[10px] text-muted-foreground font-mono">
              1:1 Direct Read · SchemaEngine /dev/shm
            </span>
          </div>

          <div className="px-4 py-2 border-b border-border flex items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="text-sm text-foreground">
                Schema-driven semantic retrieval
              </div>
              <div className="text-xs text-muted-foreground font-mono truncate">
                Trace {activeSled?.traceId || "unavailable"} · Limit{" "}
                {DEFAULT_TRACE_LIMIT}
              </div>
            </div>
            <button
              onClick={handleTraceSearch}
              disabled={traceLoading || !activeSled?.traceId}
              className="px-3 py-1.5 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-xs font-medium hover:bg-muted/30 transition-colors disabled:opacity-50 flex items-center gap-1.5"
            >
              {traceLoading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCcw className="h-3.5 w-3.5" />
              )}
              Refresh
            </button>
          </div>

          {/* Trace results */}
          <ScrollArea className="flex-1">
            <div className="px-4 py-3 space-y-2">
              {traceError && (
                <div className="rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-sm text-danger">
                  {traceError}
                </div>
              )}

              {traceLoading && (
                <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Querying Voyage and Qdrant…
                </div>
              )}

              {!traceLoading && !traceError && traceResults.length === 0 && (
                <div className="flex flex-col items-center justify-center py-8 text-sm text-muted-foreground gap-2">
                  <Database className="h-5 w-5" />
                  <span>
                    No semantic trace matches yet. Refresh after the shuttle
                    indexes this session.
                  </span>
                </div>
              )}

              {!traceLoading &&
                traceResults.map((entry) => {
                  const style = COMPLIANCE_STYLE[entry.complianceStatus];
                  return (
                    <div
                      key={entry.id}
                      className="rounded-lg border border-border bg-card px-3 py-2.5 flex flex-col gap-1.5"
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2 text-xs">
                          <StatusDot status={style.dot} />
                          <span
                            className={cn("font-medium uppercase", style.text)}
                          >
                            {entry.complianceStatus}
                          </span>
                          <span className="text-muted-foreground">·</span>
                          <span className="font-mono text-muted-foreground">
                            {entry.outcomeClass}
                          </span>
                          <span className="text-muted-foreground">·</span>
                          <span className="text-muted-foreground">
                            score {entry.score.toFixed(2)}
                          </span>
                        </div>
                        <span className="text-[10px] font-mono text-muted-foreground">
                          {entry.source}
                        </span>
                      </div>
                      <div className="text-sm text-foreground leading-relaxed">
                        {entry.summary}
                      </div>
                      {entry.decision && (
                        <div className="text-xs text-muted-foreground">
                          {entry.decision}
                        </div>
                      )}
                      <div className="flex items-center gap-2 text-[10px] font-mono text-muted-foreground">
                        <span>point {entry.pointId || "unknown"}</span>
                        <span>·</span>
                        <span>trace {activeSled?.traceId || "unknown"}</span>
                        {activeSled && (
                          <>
                            <span>·</span>
                            <span>idx {activeSled.mutationIndex}</span>
                          </>
                        )}
                      </div>
                      {entry.toolsConsulted.length > 0 && (
                        <div className="text-xs text-muted-foreground">
                          Tools: {entry.toolsConsulted.join(", ")}
                        </div>
                      )}
                      <div className="flex items-start gap-1.5 text-xs text-muted-foreground">
                        <ShieldCheck className="h-3 w-3 mt-0.5 shrink-0" />
                        <span>{entry.complianceNote}</span>
                      </div>
                      <details className="rounded-md border border-border/60 bg-background/40">
                        <summary className="cursor-pointer list-none px-3 py-2 text-xs text-muted-foreground">
                          Inspect payload
                        </summary>
                        <div className="px-3 pb-3">
                          <JsonRenderer
                            data={entry.payload}
                            defaultMode="tree"
                            maxDepth={4}
                          />
                        </div>
                      </details>
                    </div>
                  );
                })}
            </div>
          </ScrollArea>
        </div>
      </div>
    </div>
  );
};
