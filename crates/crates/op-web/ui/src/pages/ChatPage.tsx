import { useState, useEffect, useRef, useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { MessageSquare, Send, Plus, FileText, Terminal, Download, Search, Lock, Pencil } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getChatSessions, getChatMessages, sendChatMessage, getSystemPrompt, updateSystemPrompt, getLogsStream, createChatSession } from "@/lib/api";

function ChatView() {
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const queryClient = useQueryClient();

  const sessions = useQuery({ queryKey: ["chatSessions"], queryFn: getChatSessions });
  const messages = useQuery({
    queryKey: ["chatMessages", selectedSession],
    queryFn: () => getChatMessages(selectedSession!),
    enabled: !!selectedSession,
  });

  const createSession = useMutation({
    mutationFn: () => createChatSession(),
    onSuccess: (newSession) => {
      queryClient.invalidateQueries({ queryKey: ["chatSessions"] });
      setSelectedSession(newSession.id);
    },
  });

  const sendMsg = useMutation({
    mutationFn: (message: string) => sendChatMessage({ session_id: selectedSession ?? undefined, message }),
    onSuccess: (data) => {
      // If no session was selected, use the session_id from response
      if (!selectedSession && data.session_id) {
        setSelectedSession(data.session_id);
      }
      queryClient.invalidateQueries({ queryKey: ["chatMessages", selectedSession || data.session_id] });
      queryClient.invalidateQueries({ queryKey: ["chatSessions"] });
      setInput("");
    },
  });

  const handleSend = () => {
    if (!input.trim()) return;
    sendMsg.mutate(input);
  };

  return (
    <div className="flex h-[calc(100vh-8rem)] gap-4">
      {/* Sessions sidebar */}
      <Card className="w-64 shrink-0 border-border/50 hidden lg:flex flex-col">
        <CardHeader className="pb-2 px-3 pt-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Sessions</CardTitle>
            <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground" onClick={() => createSession.mutate()}>
              <Plus className="h-3.5 w-3.5" />
            </Button>
          </div>
        </CardHeader>
        <ScrollArea className="flex-1 px-2 pb-2">
          {sessions.data && Array.isArray(sessions.data) ? (
            sessions.data.map((s: any) => (
              <button
                key={s.id}
                onClick={() => setSelectedSession(s.id)}
                className={`w-full text-left p-2.5 rounded-md text-sm mb-1 transition-colors ${
                  selectedSession === s.id ? "bg-primary/10 text-primary" : "text-muted-foreground hover:bg-accent hover:text-foreground"
                }`}
              >
                <p className="font-medium truncate text-xs">{s.title}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{s.date} • {s.messages} msgs</p>
              </button>
            ))
          ) : (
            <p className="text-xs text-muted-foreground p-2">
              {sessions.isLoading ? "Loading..." : "No sessions"}
            </p>
          )}
        </ScrollArea>
      </Card>

      {/* Chat area */}
      <Card className="flex-1 border-border/50 flex flex-col">
        <ScrollArea className="flex-1 p-4">
          {messages.data?.messages && Array.isArray(messages.data.messages) ? (
            messages.data.messages.map((msg: any, i: number) => (
              <div key={msg.id ?? i} className={`mb-4 flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
                <div className={`max-w-[75%] rounded-lg px-4 py-2.5 text-sm ${
                  msg.role === "user"
                    ? "bg-primary text-primary-foreground"
                    : "bg-secondary text-secondary-foreground"
                }`}>
                  <pre className="whitespace-pre-wrap font-sans text-sm">{msg.content}</pre>
                </div>
              </div>
            ))
          ) : (
            <div className="h-full flex items-center justify-center text-muted-foreground text-sm">
              {selectedSession ? (messages.isLoading ? "Loading..." : "No messages") : "Select a session or start a new chat"}
            </div>
          )}
        </ScrollArea>
        <div className="p-3 border-t border-border/50">
          <div className="flex gap-2">
            <Textarea
              placeholder="Type a message..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
              className="min-h-[40px] max-h-32 bg-secondary border-none resize-none text-sm"
              rows={1}
            />
            <Button onClick={handleSend} disabled={sendMsg.isPending || !input.trim()} size="icon" className="shrink-0">
              <Send className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}

function LogsView() {
  const [logs, setLogs] = useState<{ level: string; service: string; message: string; timestamp: string }[]>([]);
  const [paused, setPaused] = useState(false);
  const [filter, setFilter] = useState("all");
  const [serviceFilter, setServiceFilter] = useState("all");
  const [search, setSearch] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const pausedRef = useRef(paused);
  const eventSourceRef = useRef<EventSource | null>(null);

  pausedRef.current = paused;

  useEffect(() => {
    const url = getLogsStream();
    const es = new EventSource(url);
    eventSourceRef.current = es;

    es.onmessage = (event) => {
      if (pausedRef.current) return;
      try {
        const data = JSON.parse(event.data);
        setLogs((prev) => [...prev.slice(-500), data]);
      } catch {
        // plain text fallback
        setLogs((prev) => [
          ...prev.slice(-500),
          { level: "INFO", service: "system", message: event.data, timestamp: new Date().toISOString() },
        ]);
      }
    };

    es.onerror = () => {
      // will auto-reconnect
    };

    return () => es.close();
  }, []);

  useEffect(() => {
    if (!paused && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, paused]);

  const levelColor: Record<string, string> = {
    ERROR: "text-destructive",
    WARN: "text-warning",
    INFO: "text-info",
    DEBUG: "text-muted-foreground",
  };

  const filtered = logs.filter((l) => {
    if (filter !== "all" && l.level?.toUpperCase() !== filter) return false;
    if (serviceFilter !== "all" && l.service !== serviceFilter) return false;
    if (search && !l.message?.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const services = Array.from(new Set(logs.map((l) => l.service).filter(Boolean)));

  const downloadLogs = () => {
    const text = filtered.map((l) => `[${l.timestamp}] [${l.level}] [${l.service}] ${l.message}`).join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `logs-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
  };

  return (
    <Card className="border-border/50 h-[calc(100vh-8rem)] flex flex-col">
      <CardHeader className="pb-2 shrink-0">
        <div className="flex items-center justify-between flex-wrap gap-2">
          <div className="flex items-center gap-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">System Logs</CardTitle>
            <Badge variant="outline" className={`text-xs ${!paused ? "text-success border-success/30" : "text-warning border-warning/30"}`}>
              {paused ? "Paused" : "Live"}
            </Badge>
            <span className="text-xs text-muted-foreground">{filtered.length} entries</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
              <Input
                placeholder="Search logs..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-7 w-40 pl-7 bg-secondary border-none text-xs"
              />
            </div>
            <Select value={filter} onValueChange={setFilter}>
              <SelectTrigger className="h-7 w-24 bg-secondary border-none text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All levels</SelectItem>
                <SelectItem value="ERROR">Error</SelectItem>
                <SelectItem value="WARN">Warn</SelectItem>
                <SelectItem value="INFO">Info</SelectItem>
                <SelectItem value="DEBUG">Debug</SelectItem>
              </SelectContent>
            </Select>
            <Select value={serviceFilter} onValueChange={setServiceFilter}>
              <SelectTrigger className="h-7 w-28 bg-secondary border-none text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All services</SelectItem>
                {services.map((s) => (
                  <SelectItem key={s} value={s}>{s}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button size="sm" variant="outline" onClick={() => setPaused(!paused)} className="text-xs h-7">
              {paused ? "Resume" : "Pause"}
            </Button>
            <Button size="sm" variant="outline" onClick={downloadLogs} className="text-xs h-7">
              <Download className="h-3 w-3" />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex-1 overflow-hidden p-0">
        <div ref={scrollRef} className="h-full overflow-auto scrollbar-thin px-4 pb-4">
          <div className="font-mono text-xs space-y-0">
            {filtered.length > 0 ? filtered.map((line, i) => (
              <div key={i} className="py-0.5 flex gap-2 hover:bg-accent/30 px-1 rounded">
                <span className="text-muted-foreground/60 shrink-0">{line.timestamp?.slice(11, 19) ?? ""}</span>
                <span className={`shrink-0 w-12 font-semibold ${levelColor[line.level?.toUpperCase()] ?? "text-muted-foreground"}`}>
                  {line.level?.toUpperCase() ?? "LOG"}
                </span>
                <span className="text-primary/70 shrink-0">[{line.service ?? "sys"}]</span>
                <span className="text-foreground/80">{line.message}</span>
              </div>
            )) : (
              <p className="text-muted-foreground py-8 text-center text-sm font-sans">
                {logs.length === 0 ? "Connecting to log stream..." : "No logs match filters"}
              </p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
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

  // Load tunable section from API
  useEffect(() => {
    if (promptQuery.data && !loaded) {
      setTunablePrompt(promptQuery.data?.tunable ?? promptQuery.data?.custom ?? "");
      setLoaded(true);
    }
  }, [promptQuery.data, loaded]);

  const immutablePrompt = promptQuery.data?.immutable ?? promptQuery.data?.system ?? promptQuery.data?.prompt ?? "";
  const charCount = tunablePrompt.length;

  return (
    <Card className="border-border/50 h-[calc(100vh-8rem)] flex flex-col">
      <CardHeader className="pb-2 shrink-0">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm font-medium text-muted-foreground">System Prompt</CardTitle>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">{charCount} chars</span>
            <Button
              size="sm"
              variant="outline"
              className="text-xs h-7"
              onClick={() => { setTunablePrompt(""); }}
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
      </CardHeader>
      <CardContent className="flex-1 overflow-auto scrollbar-thin space-y-4">
        {/* Immutable Section */}
        <div>
          <div className="flex items-center gap-2 mb-2">
            <Lock className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Immutable (read-only)</span>
          </div>
          <div className="p-4 rounded-lg bg-secondary/50 border border-border/30">
            <pre className="whitespace-pre-wrap font-mono text-xs text-muted-foreground leading-relaxed">
              {promptQuery.isLoading ? "Loading..." : (immutablePrompt || "No immutable prompt defined")}
            </pre>
          </div>
        </div>

        {/* Tunable Section */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2">
            <Pencil className="h-3.5 w-3.5 text-primary" />
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Tunable (editable)</span>
          </div>
          <Textarea
            value={tunablePrompt}
            onChange={(e) => setTunablePrompt(e.target.value)}
            className="min-h-[200px] bg-secondary border-none resize-none font-mono text-sm"
            placeholder="Add custom instructions, persona adjustments, or context here..."
          />
        </div>
      </CardContent>
    </Card>
  );
}

export default function ChatPage() {
  return (
    <div className="animate-slide-in">
      <div className="mb-4">
        <h1 className="text-2xl font-bold text-foreground">Chat</h1>
        <p className="text-sm text-muted-foreground mt-1">AI assistant & system tools</p>
      </div>
      <Tabs defaultValue="chat">
        <TabsList className="bg-secondary mb-4">
          <TabsTrigger value="chat" className="text-xs gap-1.5"><MessageSquare className="h-3.5 w-3.5" /> Chat</TabsTrigger>
          <TabsTrigger value="logs" className="text-xs gap-1.5"><Terminal className="h-3.5 w-3.5" /> Logs</TabsTrigger>
          <TabsTrigger value="prompt" className="text-xs gap-1.5"><FileText className="h-3.5 w-3.5" /> System Prompt</TabsTrigger>
        </TabsList>
        <TabsContent value="chat"><ChatView /></TabsContent>
        <TabsContent value="logs"><LogsView /></TabsContent>
        <TabsContent value="prompt"><SystemPromptView /></TabsContent>
      </Tabs>
    </div>
  );
}
