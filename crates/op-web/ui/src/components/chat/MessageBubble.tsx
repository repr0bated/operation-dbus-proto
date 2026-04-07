import { Pill } from "@/components/shell/Primitives";
import { GenerativeBlock, parseGenerativeBlocks } from "@/components/chat/GenerativeBlock";
import { cn } from "@/lib/utils";

export interface LocalMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: number;
  toolCalls?: Array<{ id: string; name: string; arguments: Record<string, unknown>; result?: unknown; status: string }>;
}

interface MessageBubbleProps {
  message: LocalMessage;
  onInspect: (data: unknown) => void;
  onAction?: (action: string, payload: unknown) => void;
}

export function MessageBubble({ message, onInspect, onAction }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";
  const time = new Date(message.timestamp).toLocaleTimeString();

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <span className="text-xs text-muted-foreground bg-muted/30 px-3 py-1 rounded-full">{message.content}</span>
      </div>
    );
  }

  // Parse generative UI blocks from assistant messages
  const segments = !isUser ? parseGenerativeBlocks(message.content) : null;

  return (
    <div className={cn("flex gap-3", isUser && "flex-row-reverse")}>
      <div className={cn(
        "h-8 w-8 rounded-full flex items-center justify-center text-xs font-bold shrink-0",
        isUser ? "bg-muted text-foreground" : "bg-primary/20 text-primary",
      )}>
        {isUser ? "OP" : "AI"}
      </div>
      <div className={cn("max-w-[75%] space-y-2", isUser && "text-right")}>
        <div className={cn(
          "rounded-lg px-4 py-2.5 text-sm",
          isUser ? "bg-primary/10 border border-primary/20 text-foreground" : "bg-card border border-border text-foreground",
        )}>
          {isUser || !segments ? (
            <div className="whitespace-pre-wrap">{message.content}</div>
          ) : (
            <div className="space-y-3">
              {segments.map((seg, i) =>
                seg.type === "text" ? (
                  <div key={i} className="whitespace-pre-wrap">{seg.text}</div>
                ) : (
                  <GenerativeBlock key={i} spec={seg.spec} onAction={onAction} />
                )
              )}
            </div>
          )}
        </div>
        {message.toolCalls?.map((tc) => (
          <button
            key={tc.id}
            onClick={() => onInspect(tc)}
            className="w-full text-left rounded-lg border border-border bg-muted/20 px-3 py-2 hover:border-primary/30 transition-colors"
          >
            <div className="flex items-center gap-2">
              <Pill variant={tc.status === "completed" ? "ok" : tc.status === "error" ? "danger" : "default"}>
                {tc.status}
              </Pill>
              <span className="font-mono text-xs text-foreground">{tc.name}</span>
            </div>
            {tc.result && (
              <pre className="mt-1.5 font-mono text-[11px] text-muted-foreground truncate">{JSON.stringify(tc.result).slice(0, 100)}</pre>
            )}
          </button>
        ))}
        <div className="text-[10px] text-muted-foreground">{time}</div>
      </div>
    </div>
  );
}
