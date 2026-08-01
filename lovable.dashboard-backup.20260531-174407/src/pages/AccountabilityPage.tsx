import { useState, useRef, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Search,
  MessageSquare,
  Bot,
  User,
  Send,
  Loader2,
  FileText,
  Clock,
  Fingerprint,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { searchSemanticTraces, sendChatMessage } from "@/lib/api";

interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
}

interface SearchResult {
  trace_id: string;
  timestamp: string;
  schema_name: string;
  content: string;
  relevance_score: number;
}

const SYSTEM_PROMPT =
  "You are an accountability auditor. Analyze traces and answer questions about system state, decisions, and compliance.";

function generateId() {
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export default function AccountabilityPage() {
  // ── Chat state ──
  const [messages, setMessages] = useState<Message[]>([
    {
      id: generateId(),
      role: "assistant",
      content: SYSTEM_PROMPT,
      timestamp: new Date(),
    },
  ]);
  const [chatInput, setChatInput] = useState("");
  const [isSending, setIsSending] = useState(false);
  const chatScrollRef = useRef<HTMLDivElement>(null);

  // ── Search state ──
  const [searchQuery, setSearchQuery] = useState("");
  const [activeQuery, setActiveQuery] = useState("");

  const searchQueryResult = useQuery({
    queryKey: ["semanticSearch", activeQuery],
    queryFn: () => searchSemanticTraces(activeQuery),
    enabled: activeQuery.length > 0,
  });

  const searchResults: SearchResult[] =
    searchQueryResult.data && Array.isArray(searchQueryResult.data)
      ? searchQueryResult.data
      : [];

  // Auto-scroll chat to bottom when messages change
  useEffect(() => {
    if (chatScrollRef.current) {
      chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight;
    }
  }, [messages]);

  // ── Chat handlers ──
  const handleSend = async () => {
    if (!chatInput.trim() || isSending) return;

    const userMsg: Message = {
      id: generateId(),
      role: "user",
      content: chatInput.trim(),
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMsg]);
    setChatInput("");
    setIsSending(true);

    try {
      // Build context from recent search results if available
      const contextParts: string[] = [];
      if (searchResults.length > 0) {
        contextParts.push(
          "Recent accountability traces:\n" +
            searchResults
              .slice(0, 5)
              .map(
                (r) =>
                  `- [${r.trace_id}] ${r.schema_name} (${r.timestamp}): ${r.content.slice(0, 200)}...`,
              )
              .join("\n"),
        );
      }

      const fullMessage =
        contextParts.length > 0
          ? `${contextParts.join("\n\n")}\n\nUser question: ${userMsg.content}`
          : userMsg.content;

      const res = await sendChatMessage({ message: fullMessage });
      const assistantContent =
        typeof res?.response === "string"
          ? res.response
          : typeof res?.content === "string"
            ? res.content
            : JSON.stringify(res);

      setMessages((prev) => [
        ...prev,
        {
          id: generateId(),
          role: "assistant",
          content: assistantContent,
          timestamp: new Date(),
        },
      ]);
    } catch (err) {
      setMessages((prev) => [
        ...prev,
        {
          id: generateId(),
          role: "assistant",
          content:
            err instanceof Error
              ? `Error: ${err.message}`
              : "An unexpected error occurred.",
          timestamp: new Date(),
        },
      ]);
    } finally {
      setIsSending(false);
    }
  };

  const handleSummarizeResults = () => {
    if (searchResults.length === 0) return;

    const summaryText = `Please summarize the following ${searchResults.length} accountability trace(s):\n\n${searchResults
      .map(
        (r, i) =>
          `${i + 1}. [${r.trace_id}] ${r.schema_name} (score: ${(r.relevance_score * 100).toFixed(1)}%)\n   ${r.content.slice(0, 300)}${r.content.length > 300 ? "..." : ""}`,
      )
      .join("\n\n")}`;

    const userMsg: Message = {
      id: generateId(),
      role: "user",
      content: summaryText,
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMsg]);
    setIsSending(true);

    sendChatMessage({ message: summaryText })
      .then((res) => {
        const assistantContent =
          typeof res?.response === "string"
            ? res.response
            : typeof res?.content === "string"
              ? res.content
              : JSON.stringify(res);

        setMessages((prev) => [
          ...prev,
          {
            id: generateId(),
            role: "assistant",
            content: assistantContent,
            timestamp: new Date(),
          },
        ]);
      })
      .catch((err) => {
        setMessages((prev) => [
          ...prev,
          {
            id: generateId(),
            role: "assistant",
            content:
              err instanceof Error
                ? `Error: ${err.message}`
                : "An unexpected error occurred.",
            timestamp: new Date(),
          },
        ]);
      })
      .finally(() => setIsSending(false));
  };

  const handleSearch = () => {
    if (!searchQuery.trim()) return;
    setActiveQuery(searchQuery.trim());
  };

  return (
    <div className="animate-slide-in flex flex-col gap-4 h-[calc(100vh-6rem)]">
      <div className="mb-2">
        <h1 className="text-2xl font-bold text-foreground flex items-center gap-2">
          <MessageSquare className="h-6 w-6" />
          Accountability
        </h1>
        <p className="text-sm text-muted-foreground mt-1">
          Chat with the accountability auditor and search semantic traces
        </p>
      </div>

      {/* ── Top panel: Chat (~60%) ── */}
      <Card className="flex-[3] border-border/50 flex flex-col min-h-0">
        <CardHeader className="pb-2 shrink-0">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Bot className="h-4 w-4 text-primary" />
              <CardTitle className="text-sm font-medium text-muted-foreground">
                Auditor Chat
              </CardTitle>
            </div>
            <Badge variant="outline" className="text-xs">
              {messages.filter((m) => m.role === "user").length} messages
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="flex-1 min-h-0 flex flex-col p-0">
          <ScrollArea className="flex-1 px-4 py-3">
            <div ref={chatScrollRef} className="space-y-4">
              {messages.map((msg) => (
                <div
                  key={msg.id}
                  className={cn(
                    "flex w-full",
                    msg.role === "user" ? "justify-end" : "justify-start",
                  )}
                >
                  <div
                    className={cn(
                      "max-w-[80%] rounded-lg px-4 py-2.5 text-sm",
                      msg.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "bg-secondary text-secondary-foreground",
                    )}
                  >
                    <div className="flex items-center gap-1.5 mb-1 opacity-70">
                      {msg.role === "user" ? (
                        <User className="h-3 w-3" />
                      ) : (
                        <Bot className="h-3 w-3" />
                      )}
                      <span className="text-[10px] uppercase tracking-wider font-medium">
                        {msg.role}
                      </span>
                    </div>
                    <pre className="whitespace-pre-wrap font-sans text-sm leading-relaxed">
                      {msg.content}
                    </pre>
                  </div>
                </div>
              ))}
              {isSending && (
                <div className="flex justify-start">
                  <div className="bg-secondary text-secondary-foreground rounded-lg px-4 py-2.5 text-sm flex items-center gap-2">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    <span>Auditor is analyzing...</span>
                  </div>
                </div>
              )}
            </div>
          </ScrollArea>

          {/* Chat input */}
          <div className="p-3 border-t border-border/50 shrink-0">
            <div className="flex gap-2">
              <Textarea
                placeholder="Ask the accountability auditor..."
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                  }
                }}
                className="min-h-[40px] max-h-32 bg-secondary border-none resize-none text-sm"
                rows={1}
              />
              <Button
                onClick={handleSend}
                disabled={isSending || !chatInput.trim()}
                size="icon"
                className="shrink-0"
              >
                {isSending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Send className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* ── Bottom panel: Semantic Search (~40%) ── */}
      <Card className="flex-[2] border-border/50 flex flex-col min-h-0">
        <CardHeader className="pb-2 shrink-0">
          <div className="flex items-center justify-between gap-2 flex-wrap">
            <div className="flex items-center gap-2">
              <Search className="h-4 w-4 text-primary" />
              <CardTitle className="text-sm font-medium text-muted-foreground">
                Semantic Trace Search
              </CardTitle>
            </div>
            <div className="flex items-center gap-2">
              {searchResults.length > 0 && (
                <Button
                  size="sm"
                  variant="outline"
                  className="text-xs h-7 gap-1"
                  onClick={handleSummarizeResults}
                >
                  <MessageSquare className="h-3 w-3" />
                  Summarize results
                </Button>
              )}
              <Badge variant="outline" className="text-xs">
                {searchResults.length} results
              </Badge>
            </div>
          </div>
        </CardHeader>
        <CardContent className="flex-1 min-h-0 flex flex-col p-0">
          {/* Search bar */}
          <div className="p-3 border-b border-border/50 shrink-0">
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="Search accountability traces..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSearch();
                  }}
                  className="pl-9 bg-secondary border-none text-sm"
                />
              </div>
              <Button
                onClick={handleSearch}
                disabled={searchQueryResult.isLoading || !searchQuery.trim()}
                size="sm"
                className="shrink-0 gap-1"
              >
                {searchQueryResult.isLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Search className="h-4 w-4" />
                )}
                Search
              </Button>
            </div>
          </div>

          {/* Results */}
          <ScrollArea className="flex-1 px-4 py-3">
            <div className="space-y-3">
              {searchResults.length > 0 ? (
                searchResults.map((result, idx) => (
                  <Card
                    key={result.trace_id ?? idx}
                    className="border-border/40 bg-background/50 hover:bg-accent/30 transition-colors"
                  >
                    <CardContent className="p-3">
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <Fingerprint className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                            <span className="text-xs font-mono text-foreground truncate">
                              {result.trace_id}
                            </span>
                            <Badge
                              variant="secondary"
                              className="text-[10px] shrink-0"
                            >
                              {result.schema_name}
                            </Badge>
                          </div>
                          <div className="flex items-center gap-1.5 text-muted-foreground mb-1.5">
                            <Clock className="h-3 w-3" />
                            <span className="text-[11px]">
                              {result.timestamp}
                            </span>
                          </div>
                          <div className="flex items-start gap-1.5">
                            <FileText className="h-3.5 w-3.5 text-muted-foreground shrink-0 mt-0.5" />
                            <p className="text-xs text-foreground/80 line-clamp-2 leading-relaxed">
                              {result.content}
                            </p>
                          </div>
                        </div>
                        <div className="shrink-0 text-right">
                          <Badge
                            variant="outline"
                            className={cn(
                              "text-[10px]",
                              result.relevance_score > 0.8
                                ? "border-success/30 text-success"
                                : result.relevance_score > 0.5
                                  ? "border-warning/30 text-warning"
                                  : "border-muted-foreground/30 text-muted-foreground",
                            )}
                          >
                            {(result.relevance_score * 100).toFixed(0)}%
                          </Badge>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                ))
              ) : activeQuery ? (
                <div className="text-center py-8 text-muted-foreground text-sm">
                  {searchQueryResult.isLoading
                    ? "Searching traces..."
                    : "No traces found for your query."}
                </div>
              ) : (
                <div className="text-center py-8 text-muted-foreground text-sm flex flex-col items-center gap-2">
                  <Search className="h-8 w-8 opacity-20" />
                  <p>Enter a query above to search accountability traces.</p>
                </div>
              )}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}
