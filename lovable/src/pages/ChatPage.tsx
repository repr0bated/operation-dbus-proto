import { useState, useRef, useEffect, useCallback } from "react";
import { Callout } from "@/components/shell/Primitives";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { SchemaForm } from "@/components/json/SchemaForm";
import {
  MessageBubble,
  type LocalMessage,
} from "@/components/chat/MessageBubble";
import { SystemPromptEditor } from "@/components/chat/SystemPromptEditor";
import { useEventStore } from "@/stores/event-store";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";
import { X, Maximize2, MessageSquare, FileText } from "lucide-react";

type ChatTab = "chat" | "system-prompt";

interface ChatFormData {
  provider?: string;
  model?: string;
  message?: string;
}

interface ZeroclawSchema {
  zeroclaw?: {
    chat_form_schema?: {
      type?: string;
      properties?: Record<string, {
        type?: string;
        enum?: string[];
        oneOf?: Array<{ const?: string; title?: string }>;
        description?: string;
        minLength?: number;
      }>;
      required?: string[];
      title?: string;
    };
  };
}

function historyMessages(payload: unknown): LocalMessage[] {
  const value = payload as { messages?: unknown[] } | unknown[];
  const messages = Array.isArray(value)
    ? value
    : Array.isArray(value?.messages)
      ? value.messages
      : [];

  return messages.map((item, index) => {
    const msg = item as {
      id?: unknown;
      role?: unknown;
      content?: unknown;
      timestamp?: unknown;
    };
    return {
      id: String(msg.id ?? `history-${index}`),
      role: String(msg.role ?? "assistant") as LocalMessage["role"],
      content: String(msg.content ?? ""),
      timestamp: msg.timestamp
        ? Date.parse(String(msg.timestamp)) || Date.now()
        : Date.now(),
    };
  });
}

function assistantMessage(payload: unknown): LocalMessage {
  const value = payload as {
    id?: unknown;
    role?: unknown;
    content?: unknown;
    message?: unknown;
    error?: unknown;
  };
  const body =
    value?.content ?? value?.message ?? value?.error ?? "(no response)";
  return {
    id: String(value?.id ?? `msg-${Date.now()}-resp`),
    role: String(value?.role ?? "assistant") as LocalMessage["role"],
    content: typeof body === "string" ? body : JSON.stringify(body, null, 2),
    timestamp: Date.now(),
  };
}

export default function ChatPage() {
  const { connected } = useEventStore();
  const [activeTab, setActiveTab] = useState<ChatTab>("chat");
  const [messages, setMessages] = useState<LocalMessage[]>([
    {
      id: "sys-1",
      role: "system",
      content: "Connected to Operation-DBUS control plane. Schema-driven chat interface ready.",
      timestamp: Date.now(),
    },
  ]);
  const [formData, setFormData] = useState<ChatFormData>({});
  const [schema, setSchema] = useState<ZeroclawSchema | null>(null);
  const [gatewayError, setGatewayError] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sidebarContent, setSidebarContent] = useState<unknown>(null);
  const [focusMode, setFocusMode] = useState(false);
  const [sending, setSending] = useState(false);
  const threadRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    threadRef.current?.scrollTo({
      top: threadRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  // Fetch schema once on mount — this drives the entire UI via JSON Schema
  useEffect(() => {
    let cancelled = false;
    api.zeroclaw
      .schema()
      .then((payload) => {
        if (cancelled) return;
        setSchema(payload as ZeroclawSchema);
        
        // Initialize form with defaults from schema
        const chatSchema = (payload as ZeroclawSchema)?.zeroclaw?.chat_form_schema;
        const defaults: ChatFormData = {};
        if (chatSchema?.properties) {
          // Set first enum value as default for provider
          const providerProp = chatSchema.properties.provider;
          if (providerProp?.enum && providerProp.enum.length > 0) {
            defaults.provider = providerProp.enum[0];
          }
          // Model will be set based on provider selection
        }
        setFormData(defaults);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setGatewayError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Update model options when provider changes (filter oneOf by provider hint)
  useEffect(() => {
    if (!formData.provider || !schema?.zeroclaw?.chat_form_schema?.properties) return;
    
    const modelProp = schema.zeroclaw.chat_form_schema.properties.model;
    if (modelProp?.oneOf) {
      // Filter models that match the selected provider
      const providerPrefix = formData.provider.toLowerCase();
      const filtered = modelProp.oneOf.filter(opt => {
        const title = opt.title?.toLowerCase() || '';
        const constVal = opt.const?.toLowerCase() || '';
        return title.includes(providerPrefix) || 
               constVal.includes(providerPrefix) ||
               (formData.provider === 'factory' && title.includes('factory'));
      });
      
      // If we have filtered models and current model doesn't match, pick first
      if (filtered.length > 0) {
        const currentValid = filtered.find(m => m.const === formData.model);
        if (!currentValid) {
          setFormData(prev => ({ ...prev, model: filtered[0].const }));
        }
      }
    }
  }, [formData.provider, schema]);

  useEffect(() => {
    api.chat
      .history("default")
      .then((payload) => {
        const loaded = historyMessages(payload);
        if (loaded.length > 0) setMessages(loaded);
      })
      .catch(() => {
        // History is best-effort.
      });
  }, []);

  const handleSend = async () => {
    const message = formData.message?.trim();
    if (!message || sending) return;
    
    const userMsg: LocalMessage = {
      id: `msg-${Date.now()}`,
      role: "user",
      content: message,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);
    setFormData((prev) => ({ ...prev, message: "" }));
    setSending(true);

    try {
      const response = await api.chat.send(
        "default",
        userMsg.content,
        formData.provider,
        formData.model,
      );
      setMessages((prev) => [...prev, assistantMessage(response)]);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setGatewayError(msg);
      setMessages((prev) => [
        ...prev,
        {
          id: `msg-${Date.now()}-error`,
          role: "assistant",
          content: `Error: ${msg}`,
          timestamp: Date.now(),
        },
      ]);
    } finally {
      setSending(false);
    }
  };

  const openInSidebar = (data: unknown) => {
    setSidebarContent(data);
    setSidebarOpen(true);
  };

  const chatSchema = schema?.zeroclaw?.chat_form_schema;

  const tabs: { id: ChatTab; label: string; icon: React.ReactNode }[] = [
    {
      id: "chat",
      label: "Chat",
      icon: <MessageSquare className="h-3.5 w-3.5" />,
    },
    {
      id: "system-prompt",
      label: "System Prompt",
      icon: <FileText className="h-3.5 w-3.5" />,
    },
  ];

  return (
    <div className="flex flex-col h-full">
      {/* Header with tabs */}
      {!focusMode && (
        <div className="border-b border-border shrink-0">
          <div className="flex items-end justify-between gap-4 px-4 py-3">
            <div>
              <h1 className="text-[26px] font-bold tracking-tight leading-tight text-foreground">
                Antigravity Chat
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                Schema-driven interface · Zeroclaw model router
              </p>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setFocusMode(!focusMode)}
                className="p-2 rounded-md hover:bg-muted/30 text-muted-foreground hover:text-foreground transition-colors"
                title="Toggle focus mode"
              >
                <Maximize2 className="h-4 w-4" />
              </button>
            </div>
          </div>
          {/* Tab bar */}
          <div className="flex gap-0 px-4">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "inline-flex items-center gap-1.5 px-4 py-2 text-xs font-medium border-b-2 transition-colors -mb-px",
                  activeTab === tab.id
                    ? "border-primary text-foreground"
                    : "border-transparent text-muted-foreground hover:text-foreground hover:border-muted-foreground/30",
                )}
              >
                {tab.icon}
                {tab.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {focusMode && (
        <button
          onClick={() => setFocusMode(false)}
          className="absolute top-2 right-2 z-10 p-2 rounded-full bg-card border border-border hover:bg-muted/30 text-muted-foreground"
        >
          <X className="h-4 w-4" />
        </button>
      )}

      {/* Tab content */}
      {activeTab === "system-prompt" ? (
        <SystemPromptEditor />
      ) : (
        <div className={cn("flex flex-1 min-h-0", sidebarOpen && "gap-0")}>
          {/* Thread */}
          <div
            className={cn(
              "flex flex-col flex-1 min-w-0",
              sidebarOpen && "flex-[0_0_60%]",
            )}
          >
            <div
              ref={threadRef}
              className="flex-1 overflow-y-auto px-4 py-4 space-y-4"
              role="log"
            >
              {messages.map((msg) => (
                <MessageBubble
                  key={msg.id}
                  message={msg}
                  onInspect={openInSidebar}
                  onAction={(action, payload) => {
                    const actionMsg: LocalMessage = {
                      id: `msg-${Date.now()}-action`,
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
                  <div className="h-8 w-8 rounded-full bg-primary/20 flex items-center justify-center text-xs font-bold text-primary shrink-0">
                    AI
                  </div>
                  <div className="rounded-lg bg-card border border-border px-4 py-3 animate-[pulse-dot_1.5s_ease-in-out_infinite]">
                    <span className="text-sm text-muted-foreground">
                      Thinking…
                    </span>
                  </div>
                </div>
              )}
            </div>

            {/* Composer - Schema Driven */}
            <div className="border-t border-border px-4 py-3 shrink-0 bg-background space-y-3">
              {!connected && (
                <Callout variant="default" className="mb-3">
                  Event stream offline; REST chat can still send through the gateway.
                </Callout>
              )}
              {gatewayError && (
                <Callout variant="danger" className="mb-3">
                  {gatewayError}
                </Callout>
              )}
              
              {/* Schema-driven form - ALL fields rendered from JSON Schema */}
              {chatSchema ? (
                <SchemaForm
                  schema={chatSchema}
                  value={formData}
                  onChange={setFormData}
                  onSubmit={handleSend}
                  disabled={sending}
                />
              ) : (
                <div className="text-sm text-muted-foreground">
                  Loading schema...
                </div>
              )}
              
              {/* Actions - positioned below schema form */}
              <div className="flex gap-2 pt-2">
                <button
                  onClick={() =>
                    setMessages([
                      {
                        id: "sys-new",
                        role: "system",
                        content: "New session started.",
                        timestamp: Date.now(),
                      },
                    ])
                  }
                  className="px-3 py-2 rounded-md border border-border bg-[hsl(var(--bg-elevated))] text-xs font-medium hover:bg-muted/30 transition-colors"
                >
                  New session
                </button>
                <button
                  onClick={handleSend}
                  disabled={sending || !formData.message?.trim()}
                  className="px-3 py-2 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center gap-1.5"
                >
                  {sending ? "Sending..." : "Send"}
                  <kbd className="text-[10px] bg-primary-foreground/20 px-1 rounded">
                    ↵
                  </kbd>
                </button>
              </div>
            </div>
          </div>

          {/* Sidebar — tool output inspector */}
          {sidebarOpen && (
            <div className="flex-[0_0_40%] border-l border-border bg-card flex flex-col min-w-0">
              <div className="flex items-center justify-between px-4 py-2 border-b border-border">
                <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  Inspector
                </span>
                <button
                  onClick={() => setSidebarOpen(false)}
                  className="p-1 rounded hover:bg-muted/30 text-muted-foreground"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
              <div className="flex-1 overflow-auto p-3">
                {sidebarContent !== null ? (
                  <JsonRenderer data={sidebarContent} />
                ) : (
                  <div className="text-sm text-muted-foreground">
                    Click a tool result to inspect.
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
