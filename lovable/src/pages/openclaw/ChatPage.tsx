import { useState, useRef, useEffect } from "react";
import { Send, Square, Loader2, Bot, User, Wrench, ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface Message {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  toolName?: string;
  timestamp: Date;
  status?: "streaming" | "done" | "aborted";
}

const initialMessages: Message[] = [
  {
    id: "1",
    role: "assistant",
    content: "Hello! I'm your OpenClaw assistant. I can help you manage tasks, search the web, control your system, and more. What would you like to do?",
    timestamp: new Date(Date.now() - 300000),
    status: "done",
  },
  {
    id: "2",
    role: "user",
    content: "What's the current system status?",
    timestamp: new Date(Date.now() - 240000),
  },
  {
    id: "3",
    role: "tool",
    content: JSON.stringify({ gateway: "running", uptime: "4d 12h", sessions: 3, activeCrons: 7, nodes: 2 }, null, 2),
    toolName: "system.status",
    timestamp: new Date(Date.now() - 239000),
  },
  {
    id: "4",
    role: "assistant",
    content: "Everything looks good! Gateway is running with 4 days uptime. You have 3 active sessions, 7 cron jobs running, and 2 connected nodes.",
    timestamp: new Date(Date.now() - 238000),
    status: "done",
  },
];

export default function OpenClawChatPage() {
  const [messages, setMessages] = useState<Message[]>(initialMessages);
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSend = () => {
    if (!input.trim() || isStreaming) return;
    const userMsg: Message = {
      id: Date.now().toString(),
      role: "user",
      content: input.trim(),
      timestamp: new Date(),
    };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setIsStreaming(true);

    // Simulate streaming response
    setTimeout(() => {
      const toolMsg: Message = {
        id: (Date.now() + 1).toString(),
        role: "tool",
        content: JSON.stringify({ result: "ok", details: "Processed successfully" }, null, 2),
        toolName: "task.process",
        timestamp: new Date(),
      };
      setMessages((prev) => [...prev, toolMsg]);

      setTimeout(() => {
        const assistantMsg: Message = {
          id: (Date.now() + 2).toString(),
          role: "assistant",
          content: "Done! I've processed your request. Let me know if you need anything else.",
          timestamp: new Date(),
          status: "done",
        };
        setMessages((prev) => [...prev, assistantMsg]);
        setIsStreaming(false);
      }, 800);
    }, 600);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col h-[calc(100vh-theme(spacing.12)-theme(spacing.12))] max-w-4xl mx-auto">
      {/* Chat header */}
      <div className="flex items-center justify-between pb-4 border-b border-border mb-4">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-primary/10 flex items-center justify-center">
            <Bot className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-semibold text-foreground">OpenClaw Chat</h1>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span className="flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
                Connected
              </span>
              <span>·</span>
              <span>Session: default</span>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" className="text-xs h-7">
            Thinking: Auto
          </Button>
          <Button variant="outline" size="sm" className="text-xs h-7">
            Verbose
          </Button>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-auto scrollbar-thin space-y-4 pr-2">
        {messages.map((msg) => (
          <div key={msg.id} className={cn("flex gap-3", msg.role === "user" && "justify-end")}>
            {msg.role !== "user" && (
              <div className={cn(
                "h-7 w-7 rounded-md flex items-center justify-center shrink-0 mt-0.5",
                msg.role === "assistant" ? "bg-primary/10" : "bg-muted"
              )}>
                {msg.role === "assistant" ? (
                  <Bot className="h-4 w-4 text-primary" />
                ) : (
                  <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
                )}
              </div>
            )}
            <div className={cn(
              "max-w-[80%] rounded-lg px-4 py-2.5",
              msg.role === "user"
                ? "bg-primary text-primary-foreground"
                : msg.role === "tool"
                ? "bg-muted border border-border font-mono text-xs"
                : "bg-card border border-border"
            )}>
              {msg.role === "tool" && (
                <div className="text-[10px] text-muted-foreground mb-1.5 font-sans flex items-center gap-1">
                  <Wrench className="h-3 w-3" />
                  {msg.toolName}
                </div>
              )}
              <div className={cn(
                "text-sm leading-relaxed whitespace-pre-wrap",
                msg.role === "tool" && "text-muted-foreground"
              )}>
                {msg.content}
              </div>
              <div className={cn(
                "text-[10px] mt-1.5",
                msg.role === "user" ? "text-primary-foreground/60" : "text-muted-foreground"
              )}>
                {msg.timestamp.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
              </div>
            </div>
            {msg.role === "user" && (
              <div className="h-7 w-7 rounded-md bg-primary/20 flex items-center justify-center shrink-0 mt-0.5">
                <User className="h-4 w-4 text-primary" />
              </div>
            )}
          </div>
        ))}

        {isStreaming && (
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
        <div className="relative flex items-end gap-2 bg-card border border-border rounded-lg p-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Message OpenClaw... (Shift+Enter for new line)"
            rows={1}
            className="flex-1 resize-none bg-transparent text-sm text-foreground placeholder:text-muted-foreground outline-none min-h-[36px] max-h-[120px] py-2 px-2"
          />
          {isStreaming ? (
            <Button size="icon" variant="destructive" className="h-8 w-8 shrink-0" onClick={() => setIsStreaming(false)}>
              <Square className="h-3.5 w-3.5" />
            </Button>
          ) : (
            <Button size="icon" className="h-8 w-8 shrink-0" onClick={handleSend} disabled={!input.trim()}>
              <Send className="h-3.5 w-3.5" />
            </Button>
          )}
        </div>
        <p className="text-[10px] text-muted-foreground text-center mt-2">
          Type /stop to abort · Connected to Gateway on ws://localhost:18789
        </p>
      </div>
    </div>
  );
}
