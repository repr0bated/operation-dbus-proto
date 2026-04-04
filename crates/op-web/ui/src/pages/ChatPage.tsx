import { PageHeader } from "@/components/shell/PageHeader";
import { useState, useRef, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { JsonTree, JsonCodeBlock } from "@/components/json/JsonTree";
import { Badge } from "@/components/ui/badge";
import { Send, X, PanelRightOpen, Bot, User, Wrench } from "lucide-react";
import type { ChatMessage } from "@/api/types";
import { cn } from "@/lib/utils";

export default function ChatPage() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sessionId] = useState(() => crypto.randomUUID().slice(0, 8));
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [inspectorData, setInspectorData] = useState<unknown>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages]);

  const sendMessage = () => {
    if (!input.trim()) return;
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: input.trim(),
      timestamp: new Date().toISOString(),
      session_id: sessionId,
    };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");

    // Simulate assistant response
    setTimeout(() => {
      const assistantMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: "assistant",
        content: "Processing your request...",
        timestamp: new Date().toISOString(),
        session_id: sessionId,
        tool_calls: [{ id: "tc-1", name: "system.inspect", arguments: { target: "org.freedesktop.systemd1" } }],
      };
      setMessages((prev) => [...prev, assistantMsg]);
    }, 600);
  };

  const roleIcon = (role: string) => {
    switch (role) {
      case "user": return <User className="h-3.5 w-3.5" />;
      case "assistant": return <Bot className="h-3.5 w-3.5" />;
      case "tool": return <Wrench className="h-3.5 w-3.5" />;
      default: return null;
    }
  };

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <PageHeader title="Chat" description={`Session: ${sessionId}`}>
          <Button variant="ghost" size="sm" onClick={() => setInspectorOpen(!inspectorOpen)}>
            <PanelRightOpen className="h-4 w-4" />
          </Button>
        </PageHeader>

        {/* Transcript */}
        <div ref={scrollRef} className="flex-1 overflow-auto p-4 space-y-3">
          {messages.length === 0 && (
            <div className="text-center text-muted-foreground text-xs py-20">
              Start a conversation with the control plane
            </div>
          )}
          {messages.map((msg) => (
            <div key={msg.id} className={cn("flex gap-2 max-w-2xl", msg.role === "user" ? "ml-auto flex-row-reverse" : "")}>
              <div className={cn("shrink-0 h-6 w-6 rounded flex items-center justify-center text-xs",
                msg.role === "user" ? "bg-primary/20 text-primary" : "bg-accent text-accent-foreground"
              )}>
                {roleIcon(msg.role)}
              </div>
              <div className={cn("rounded border p-3 space-y-2 text-sm", msg.role === "user" ? "bg-primary/10 border-primary/20" : "bg-card")}>
                <p className="text-xs whitespace-pre-wrap">{msg.content}</p>
                {msg.tool_calls?.map((tc) => (
                  <button
                    key={tc.id}
                    onClick={() => { setInspectorData(tc); setInspectorOpen(true); }}
                    className="flex items-center gap-1.5 rounded border px-2 py-1 text-[10px] font-mono hover:bg-accent transition-colors"
                  >
                    <Wrench className="h-3 w-3" />
                    {tc.name}
                  </button>
                ))}
                <div className="text-[10px] text-muted-foreground">
                  {new Date(msg.timestamp).toLocaleTimeString()}
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* Composer */}
        <div className="border-t p-3">
          <form onSubmit={(e) => { e.preventDefault(); sendMessage(); }} className="flex gap-2">
            <Input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Send a command or ask a question..."
              className="font-mono text-xs"
            />
            <Button type="submit" size="sm"><Send className="h-4 w-4" /></Button>
          </form>
        </div>
      </div>

      {/* Inspector panel */}
      {inspectorOpen && (
        <div className="w-80 border-l bg-card flex flex-col shrink-0">
          <div className="flex items-center justify-between border-b px-3 py-2">
            <span className="text-xs font-semibold">Inspector</span>
            <button onClick={() => setInspectorOpen(false)} className="text-muted-foreground hover:text-foreground">
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
          <div className="flex-1 overflow-auto p-3">
            {inspectorData ? (
              <JsonTree data={inspectorData} defaultExpanded />
            ) : (
              <p className="text-xs text-muted-foreground">Click a tool call to inspect</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
