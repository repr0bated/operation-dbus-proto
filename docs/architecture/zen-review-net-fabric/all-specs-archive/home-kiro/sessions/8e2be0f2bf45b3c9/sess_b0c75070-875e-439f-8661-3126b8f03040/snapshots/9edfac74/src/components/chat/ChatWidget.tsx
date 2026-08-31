import { useState, useRef, useEffect, useCallback } from "react";
import { Callout } from "@/components/shell/Primitives";
import { MessageBubble, type LocalMessage } from "@/components/chat/MessageBubble";
import { useEventStore } from "@/stores/event-store";
import { chatService } from "@/grpc/client";
import { unwrapContentBlocks } from "@/lib/chat-content";
import { resolveChatSelection } from "@/lib/plugin-ui-surfaces";
import type { ChatFrame, UIMessagePart, StreamError, ApprovalRequired } from "@/grpc/gen/chat";
import { cn } from "@/lib/utils";

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

export interface ChatWidgetProps {
  agentId?: string;
  initialSessionId?: string;
  className?: string;
  height?: string | number;
}

export function ChatWidget({ agentId = "default", initialSessionId, className, height = 400 }: ChatWidgetProps) {
  const { connected } = useEventStore();
  const [messages, setMessages] = useState<LocalMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [sessionKey, setSessionKey] = useState<string | null>(initialSessionId || null);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const threadRef = useRef<HTMLDivElement>(null);
  const streamRef = useRef<{ abort: () => void } | null>(null);

  const scrollToBottom = useCallback(() => {
    threadRef.current?.scrollTo({ top: threadRef.current.scrollHeight, behavior: "smooth" });
  }, []);

  useEffect(() => { scrollToBottom(); }, [messages, scrollToBottom]);

  // Create conversation id if we don't have one
  useEffect(() => {
    if (!sessionKey) {
      setSessionKey(makeId("conv"));
    }
  }, [agentId, sessionKey]);

  /** Reset the thread when the conversation id changes. */
  const resetThread = useCallback((sid: string) => {
    streamRef.current?.abort();
    streamRef.current = null;
    setMessages([
      { id: makeId("sys"), role: "system", content: `Conversation "${sid}" ready.`, timestamp: Date.now() },
    ]);
  }, []);

  useEffect(() => {
    if (sessionKey) {
      resetThread(sessionKey);
    }
  }, [sessionKey, resetThread]);

  // Abort any active stream on unmount
  useEffect(() => () => streamRef.current?.abort(), []);

  const handleSend = async () => {
    if (!sessionKey) return;
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

    // Inference via tched_router selected_provider/model (OD-28) — not hardwired Gemini.
    const route = await resolveChatSelection();
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
        }
      }
      if (partsReceived === 0) {
        setMessages((prev) => [...prev, { id: makeId("empty"), role: "system", content: "The gateway closed without an assistant response.", timestamp: Date.now() }]);
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

  if (!sessionKey) {
    return <div className="p-4 text-xs text-muted-foreground animate-pulse">Initializing chat session...</div>;
  }

  return (
    <div className={cn("flex flex-col min-w-0 border border-border rounded-lg bg-card overflow-hidden", className)} style={{ height }}>
      {/* Thread */}
      <div ref={threadRef} className="flex-1 overflow-y-auto px-4 py-4 space-y-4" role="log">
        {messages.map((msg) => (
          <MessageBubble
            key={msg.id}
            message={msg}
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

      {/* Composer */}
      <div className="border-t border-border px-3 py-3 shrink-0 bg-background">
        {error && <Callout variant="danger" className="mb-2">{error}</Callout>}
        <div className="flex gap-2">
          <div className="flex-1 relative">
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={sending}
              placeholder="Send message..."
              className="w-full px-3 py-2 rounded-lg border border-input bg-card text-sm resize-none min-h-[40px] max-h-32 focus:border-ring focus:ring-1 focus:ring-ring outline-none transition-colors font-sans"
              rows={1}
            />
          </div>
          <button onClick={() => void handleSend()} disabled={sending || !draft.trim()} className="px-3 py-2 rounded-lg bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 shrink-0 self-end">
            {sending ? "..." : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}
