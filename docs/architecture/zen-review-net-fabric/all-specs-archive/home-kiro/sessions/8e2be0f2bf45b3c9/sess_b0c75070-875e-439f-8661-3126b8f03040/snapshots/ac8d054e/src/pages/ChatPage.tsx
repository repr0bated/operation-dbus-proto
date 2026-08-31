import { useState, useRef, useEffect, useCallback } from "react";
import { Callout } from "@/components/shell/Primitives";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { MessageBubble, type LocalMessage } from "@/components/chat/MessageBubble";
import { ProviderSelect } from "@/components/shell/ProviderSelect";
import { useEventStore } from "@/stores/event-store";
import { chatService } from "@/grpc/client";
import { unwrapContentBlocks } from "@/lib/chat-content";
import { resolveChatSelection } from "@/lib/plugin-ui-surfaces";
import type { ChatFrame, UIMessagePart, StreamError, ApprovalRequired } from "@/grpc/gen/chat";
import { cn } from "@/lib/utils";
import { X, Maximize2 } from "lucide-react";

/** Coerce arbitrary backend content into a renderable string. */
function normalizeContent(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function mapBackendRole(role: string | number): LocalMessage["role"] {
  if (role === "assistant") return "assistant";
  if (role === "tool") return "tool";
  if (role === "system") return "system";
  if (role === "user") return "user";
  return "system";
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

/**
 * Main Chat — conversation only.
 * System prompt / schema: Assistant (config) / Gallery.
 * Live log: Logs page (`/logs` → LiveLogPanel).
 */
export default function ChatPage() {
  const { connected } = useEventStore();
  const [messages, setMessages] = useState<LocalMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [sessionKey, setSessionKey] = useState("default");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sidebarContent, setSidebarContent] = useState<unknown>(null);
  const [focusMode, setFocusMode] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const threadRef = useRef<HTMLDivElement>(null);
  const streamRef = useRef<{ abort: () => void } | null>(null);

  const scrollToBottom = useCallback(() => {
    threadRef.current?.scrollTo({ top: threadRef.current.scrollHeight, behavior: "smooth" });
  }, []);

  useEffect(() => { scrollToBottom(); }, [messages, scrollToBottom]);

  /** Reset the thread for a new conversation id. */
  const resetThread = useCallback((sid: string) => {
    streamRef.current?.abort();
    streamRef.current = null;
    setMessages([
      { id: makeId("sys"), role: "system", content: `Conversation "${sid}" ready.`, timestamp: Date.now() },
    ]);
  }, []);

  // Reset thread whenever the conversation id changes
  useEffect(() => {
    resetThread(sessionKey);
  }, [sessionKey, resetThread]);

  // Abort any active stream on unmount
  useEffect(() => () => streamRef.current?.abort(), []);

  const handleSend = async () => {
    const text = draft.trim();
    if (!text || sending) return;

    const userMsg: LocalMessage = {
      id: makeId("msg"),
      role: "user",
      content: text,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);
    setDraft("");
    setSending(true);
    setError(null);
    streamRef.current?.abort();

    // Inference via zeroclaw selected_provider/model (OD-28) — not hardwired Gemini.
    // System prompt comes from Assistant config (shared storage via chatService.send).
    const route = await resolveZeroclawChatSelection();
    const { stream, abort } = chatService.send({
      conversationId: sessionKey,
      content: text,
      provider: route.provider,
      model: route.model,
    });
    streamRef.current = { abort };

    try {
      const reader = stream.getReader();
      let partsReceived = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        const frame = value as ChatFrame;
        const body = frame.body;
        if (body.oneofKind === "part") {
          partsReceived += 1;
          const part = (body as { oneofKind: "part"; part: UIMessagePart }).part;
          const payload = new TextDecoder().decode(part.payload);
          // Text parts arrive as a serialized content block; show the message,
          // not its envelope. Other kinds stay raw behind a [kind] prefix.
          let content = unwrapContentBlocks(payload);
          if (part.kind !== "text") {
            content = `[${part.kind}] ${payload}`;
          }
          const assistantMsg: LocalMessage = {
            id: makeId("part"),
            role: mapBackendRole(part.role),
            content: normalizeContent(content),
            timestamp: Date.now(),
          };
          setMessages((prev) => [...prev, assistantMsg]);
        } else if (body.oneofKind === "error") {
          const err = (body as { oneofKind: "error"; error: StreamError }).error;
          setError(err.message);
          setMessages((prev) => [
            ...prev,
            { id: makeId("err"), role: "system", content: err.message, timestamp: Date.now() },
          ]);
        } else if (body.oneofKind === "approval") {
          const approval = (body as { oneofKind: "approval"; approval: ApprovalRequired }).approval;
          const input = new TextDecoder().decode(approval.toolInput);
          setMessages((prev) => [
            ...prev,
            {
              id: makeId("approval"),
              role: "system",
              content: `Approval required: ${approval.toolName}(${input}) — ${approval.description}`,
              timestamp: Date.now(),
            },
          ]);
        } else if (body.oneofKind === "done") {
          const done = (body as { oneofKind: "done"; done: { totalParts?: string } }).done;
          if (partsReceived === 0 && done.totalParts === "0") {
            setMessages((prev) => [...prev, { id: makeId("empty"), role: "system", content: "The gateway completed without an assistant response.", timestamp: Date.now() }]);
          }
        }
      }
      if (partsReceived === 0) {
        setMessages((prev) => prev.some((message) => message.id.startsWith("empty-")) ? prev : [...prev, { id: makeId("empty"), role: "system", content: "The gateway closed without an assistant response.", timestamp: Date.now() }]);
      }
    } catch (err) {
      const msg = (err as Error).message;
      if (msg !== "AbortError") {
        setError(msg);
        setMessages((prev) => [
          ...prev,
          { id: makeId("err"), role: "system", content: `Request failed: ${msg}`, timestamp: Date.now() },
        ]);
      }
    } finally {
      setSending(false);
      streamRef.current = null;
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  };

  const handleNewSession = () => {
    setError(null);
    setSessionKey(makeId("conv"));
  };

  const openInSidebar = (data: unknown) => {
    setSidebarContent(data);
    setSidebarOpen(true);
  };

  return (
    <div className="flex flex-col h-full">
      {!focusMode && (
        <div className="border-b border-border shrink-0">
          <div className="flex items-end justify-between gap-4 px-4 py-3">
            <div>
              <h1 className="text-[26px] font-bold tracking-tight leading-tight text-foreground">Chat</h1>
              <p className="text-sm text-muted-foreground mt-0.5">Direct gateway chat session for quick interventions.</p>
            </div>
            <div className="flex items-center gap-2">
              <ProviderSelect />
              <input
                value={sessionKey}
                onChange={(e) => setSessionKey(e.target.value)}
                className="px-3 py-1.5 rounded-md border border-input bg-card text-sm font-mono focus:border-ring outline-none w-64"
                placeholder="session id"
              />
              <button onClick={() => setFocusMode(!focusMode)} className="p-2 rounded-md hover:bg-muted/30 text-muted-foreground hover:text-foreground transition-colors" title="Toggle focus mode">
                <Maximize2 className="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>
      )}

      {focusMode && (
        <button onClick={() => setFocusMode(false)} className="absolute top-2 right-2 z-10 p-2 rounded-full bg-card border border-border hover:bg-muted/30 text-muted-foreground">
          <X className="h-4 w-4" />
        </button>
      )}

      <div className={cn("flex flex-1 min-h-0", sidebarOpen && "gap-0")}>
        <div className={cn("flex flex-col flex-1 min-w-0", sidebarOpen && "flex-[0_0_60%]")}>
          <div ref={threadRef} className="flex-1 overflow-y-auto px-4 py-4 space-y-4" role="log">
            {messages.map((msg) => (
              <MessageBubble
                key={msg.id}
                message={msg}
                onInspect={openInSidebar}
                onAction={(action, payload) => {
                  const actionMsg: LocalMessage = {
                    id: makeId("action"),
                    role: "user",
                    content: `[Action: ${action}] ${JSON.stringify(payload)}`,
                    timestamp: Date.now(),
                  };
                  setMessages((prev) => [...prev, actionMsg]);
                }}
              />
            ))}
            {sending && (
              <div className="flex gap-3">
                <div className="h-8 w-8 rounded-full bg-primary/20 flex items-center justify-center text-xs font-bold text-primary shrink-0">AI</div>
                <div className="rounded-lg bg-card border border-border px-4 py-3 animate-[pulse-dot_1.5s_ease-in-out_infinite]">
                  <span className="text-sm text-muted-foreground">Thinking…</span>
                </div>
              </div>
            )}
          </div>

          <div className="border-t border-border px-4 py-3 shrink-0 bg-background">
            {error && <Callout variant="danger" className="mb-3">{error}</Callout>}
            {!connected && !error && (
              <Callout variant="default" className="mb-3">
                Event stream is idle — chat still works through the REST gateway.
              </Callout>
            )}
            <div className="flex gap-2">
              <div className="flex-1 relative">
                <textarea
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={handleKeyDown}
                  disabled={sending}
                  placeholder="Message (↩ to send, Shift+↩ for line breaks)"
                  className="w-full px-3 py-2.5 rounded-lg border border-input bg-card text-sm resize-none min-h-[44px] max-h-40 focus:border-ring focus:ring-1 focus:ring-ring outline-none transition-colors font-sans"
                  rows={1}
                />
              </div>
              <div className="flex flex-col gap-1.5 shrink-0">
                <button onClick={handleNewSession} className="px-3 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-xs font-medium hover:bg-muted/30 transition-colors">
                  New session
                </button>
                <button onClick={() => void handleSend()} disabled={sending || !draft.trim()} className="px-3 py-2 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center gap-1.5">
                  {sending ? "Sending" : "Send"}<kbd className="text-[10px] bg-primary-foreground/20 px-1 rounded">↵</kbd>
                </button>
              </div>
            </div>
          </div>
        </div>

        {sidebarOpen && (
          <div className="flex-[0_0_40%] border-l border-border bg-card flex flex-col min-w-0">
            <div className="flex items-center justify-between px-4 py-2 border-b border-border">
              <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Inspector</span>
              <button onClick={() => setSidebarOpen(false)} className="p-1 rounded hover:bg-muted/30 text-muted-foreground"><X className="h-3.5 w-3.5" /></button>
            </div>
            <div className="flex-1 overflow-auto p-3">
              {sidebarContent !== null ? (
                <JsonRenderer data={sidebarContent} />
              ) : (
                <div className="text-sm text-muted-foreground">Click a tool result to inspect.</div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
