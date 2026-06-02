import { useState, useRef, useEffect, useCallback } from "react";
import { Search, Send, Bot, User, Brain, Clock, Target, ChevronDown, ChevronUp, Loader2, Database, Shield } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { accountabilityService } from "@/grpc/client";
import type { ReasoningEpisode, ChatContextMessage } from "@/grpc/types/accountability";

// ── Chatbot Section (top half) ──────────────────────────────────────────────

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  referencedEpisodes?: string[];
  timestamp: number;
}

function AccountabilityChat({
  selectedEpisodes,
}: {
  selectedEpisodes: ReasoningEpisode[];
}) {
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "sys-1",
      role: "assistant",
      content:
        "I'm your Accountability Assistant. Search for reasoning episodes below, then ask me questions about the chatbot's decisions. I have full context of selected episodes.",
      timestamp: Date.now(),
    },
  ]);
  const [draft, setDraft] = useState("");
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

  const handleSend = async () => {
    if (!draft.trim() || sending) return;
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
      const history: ChatContextMessage[] = messages
        .filter((m) => m.role === "user" || m.role === "assistant")
        .map((m) => ({ role: m.role, content: m.content }));

      const resp = await accountabilityService.chatWithContext({
        message: userMsg.content,
        episodeIds: selectedEpisodes.map((e) => e.episodeId),
        conversationHistory: history,
      });

      setMessages((prev) => [
        ...prev,
        {
          id: `msg-${Date.now()}-resp`,
          role: "assistant",
          content: resp.reply,
          referencedEpisodes: resp.referencedEpisodes,
          timestamp: Date.now(),
        },
      ]);
    } catch {
      // Fallback: summarize selected episodes locally
      const epSummary = selectedEpisodes.length
        ? selectedEpisodes
            .map(
              (e) =>
                `• [${e.outcomeClass}] ${e.reasoningSummary} (tools: ${e.toolsConsulted.join(", ")})`
            )
            .join("\n")
        : "No episodes selected yet.";

      setMessages((prev) => [
        ...prev,
        {
          id: `msg-${Date.now()}-resp`,
          role: "assistant",
          content: `Based on the ${selectedEpisodes.length} selected episode(s):\n\n${epSummary}\n\nFor deeper analysis, ensure the accountability gRPC service is running.`,
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

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border shrink-0">
        <Brain className="h-4 w-4 text-primary" />
        <span className="text-sm font-semibold text-foreground">
          Accountability Chat
        </span>
        {selectedEpisodes.length > 0 && (
          <span className="text-[10px] bg-primary/10 text-primary px-2 py-0.5 rounded-full font-medium">
            {selectedEpisodes.length} episode
            {selectedEpisodes.length !== 1 ? "s" : ""} loaded
          </span>
        )}
      </div>

      <div
        ref={threadRef}
        className="flex-1 overflow-y-auto px-4 py-3 space-y-3"
      >
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={cn("flex gap-2.5", msg.role === "user" && "justify-end")}
          >
            {msg.role === "assistant" && (
              <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center shrink-0 mt-0.5">
                <Bot className="h-4 w-4 text-primary" />
              </div>
            )}
            <div
              className={cn(
                "max-w-[80%] rounded-lg px-3 py-2 text-sm leading-relaxed whitespace-pre-wrap",
                msg.role === "user"
                  ? "bg-primary text-primary-foreground"
                  : "bg-card border border-border"
              )}
            >
              {msg.content}
              {msg.referencedEpisodes && msg.referencedEpisodes.length > 0 && (
                <div className="mt-2 pt-2 border-t border-border/50 text-[10px] text-muted-foreground">
                  Referenced: {msg.referencedEpisodes.join(", ")}
                </div>
              )}
            </div>
            {msg.role === "user" && (
              <div className="h-7 w-7 rounded-md bg-primary/20 flex items-center justify-center shrink-0 mt-0.5">
                <User className="h-4 w-4 text-primary" />
              </div>
            )}
          </div>
        ))}
        {sending && (
          <div className="flex gap-2.5">
            <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
              <Bot className="h-4 w-4 text-primary" />
            </div>
            <div className="bg-card border border-border rounded-lg px-3 py-2">
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            </div>
          </div>
        )}
      </div>

      <div className="border-t border-border px-4 py-2 shrink-0">
        <div className="flex gap-2">
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask about the chatbot's reasoning…"
            rows={1}
            className="flex-1 resize-none bg-card border border-input rounded-md px-3 py-2 text-sm min-h-[36px] max-h-[80px] outline-none focus:border-ring transition-colors"
          />
          <Button
            size="icon"
            className="h-9 w-9 shrink-0"
            onClick={handleSend}
            disabled={!draft.trim() || sending}
          >
            <Send className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
    </div>
  );
}

// ── Qdrant Search Section (bottom half) ─────────────────────────────────────

function EpisodeCard({
  episode,
  selected,
  onToggle,
}: {
  episode: ReasoningEpisode;
  selected: boolean;
  onToggle: () => void;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      className={cn(
        "border rounded-lg transition-colors cursor-pointer",
        selected
          ? "border-primary bg-primary/5"
          : "border-border bg-card hover:border-muted-foreground/30"
      )}
    >
      <div
        className="flex items-start gap-3 px-3 py-2.5"
        onClick={onToggle}
      >
        <input
          type="checkbox"
          checked={selected}
          onChange={onToggle}
          className="mt-1 shrink-0 accent-[hsl(var(--primary))]"
          onClick={(e) => e.stopPropagation()}
        />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span
              className={cn(
                "text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded",
                episode.outcomeClass === "success"
                  ? "bg-accent/50 text-accent-foreground"
                  : episode.outcomeClass === "failure"
                  ? "bg-destructive/10 text-destructive"
                  : "bg-muted text-muted-foreground"
              )}
            >
              {episode.outcomeClass}
            </span>
            <span className="text-[10px] text-muted-foreground font-mono">
              {(episode.similarity * 100).toFixed(1)}% match
            </span>
            {episode.piiFlagged && (
              <Shield className="h-3 w-3 text-warning" />
            )}
          </div>
          <p className="text-sm text-foreground line-clamp-2">
            {episode.reasoningSummary}
          </p>
          <div className="flex items-center gap-3 mt-1.5 text-[10px] text-muted-foreground">
            <span className="flex items-center gap-1">
              <Clock className="h-3 w-3" />
              {new Date(episode.startedAt).toLocaleString()}
            </span>
            <span>{episode.durationMs}ms</span>
            <span className="flex items-center gap-1">
              <Target className="h-3 w-3" />
              {episode.pluginId || "system"}
            </span>
          </div>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(!expanded);
          }}
          className="p-1 rounded hover:bg-muted/30 text-muted-foreground shrink-0"
        >
          {expanded ? (
            <ChevronUp className="h-3.5 w-3.5" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5" />
          )}
        </button>
      </div>

      {expanded && (
        <div className="px-3 pb-3 pt-0 border-t border-border/50 space-y-2">
          <div>
            <span className="text-[10px] font-medium text-muted-foreground uppercase">
              Goal
            </span>
            <p className="text-xs text-foreground">{episode.goalText}</p>
          </div>
          <div>
            <span className="text-[10px] font-medium text-muted-foreground uppercase">
              Decision
            </span>
            <p className="text-xs text-foreground">{episode.decisionOutput}</p>
          </div>
          <div>
            <span className="text-[10px] font-medium text-muted-foreground uppercase">
              Tools Consulted
            </span>
            <div className="flex flex-wrap gap-1 mt-0.5">
              {episode.toolsConsulted.map((t) => (
                <span
                  key={t}
                  className="text-[10px] bg-muted px-1.5 py-0.5 rounded font-mono"
                >
                  {t}
                </span>
              ))}
            </div>
          </div>
          <div className="flex gap-4 text-[10px] text-muted-foreground">
            <span>Trigger: {episode.trigger}</span>
            <span>Exit: {episode.exitReason}</span>
            <span>Confidence: {(episode.confidence * 100).toFixed(0)}%</span>
          </div>
          <div className="text-[10px] text-muted-foreground font-mono break-all">
            ID: {episode.episodeId}
          </div>
        </div>
      )}
    </div>
  );
}

function QdrantSearchPanel({
  selectedEpisodes,
  onSelectionChange,
}: {
  selectedEpisodes: ReasoningEpisode[];
  onSelectionChange: (episodes: ReasoningEpisode[]) => void;
}) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ReasoningEpisode[]>([]);
  const [searching, setSearching] = useState(false);
  const [outcomeFilter, setOutcomeFilter] = useState("");
  const [pluginFilter, setPluginFilter] = useState("");
  const [stats, setStats] = useState<{
    pointCount: number;
    collectionName: string;
  } | null>(null);

  useEffect(() => {
    accountabilityService
      .getCollectionStats()
      .then((s) =>
        setStats({
          pointCount: s.pointCount,
          collectionName: s.collectionName,
        })
      )
      .catch(() =>
        setStats({ pointCount: 0, collectionName: "ctl_plane_reasoning_episodes" })
      );
  }, []);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setSearching(true);
    try {
      const resp = await accountabilityService.searchEpisodes({
        query: query.trim(),
        outcomeClass: outcomeFilter,
        pluginId: pluginFilter,
        limit: 20,
      });
      setResults(resp.episodes);
    } catch {
      // Demo fallback data
      setResults([
        {
          episodeId: "ep-demo-001",
          goalText: "Reconfigure firewall rules for container isolation",
          trigger: "scheduled_policy_check",
          toolsConsulted: ["firewall.set_rules", "ovs.get_bridge_state"],
          reasoningSummary:
            "Detected container network drift from baseline policy. Applied corrective firewall rules to restore isolation between incusbr0 segments.",
          decisionOutput: "Applied 3 iptables rules, blocked 2 cross-segment routes",
          outcomeClass: "success",
          confidence: 0.92,
          pluginId: "firewall",
          conversationId: "conv-abc",
          startedAt: new Date(Date.now() - 3600000).toISOString(),
          endedAt: new Date(Date.now() - 3599000).toISOString(),
          durationMs: 1024,
          exitReason: "goal_achieved",
          piiFlagged: false,
          similarity: 0.89,
        },
        {
          episodeId: "ep-demo-002",
          goalText: "Investigate DNS resolution failure on services container",
          trigger: "alert_triggered",
          toolsConsulted: ["dns.check", "network.ping", "service.status"],
          reasoningSummary:
            "NextDNS service on 10.149.181.10 was unresponsive. Restarted via dinit, confirmed resolution restored.",
          decisionOutput: "Restarted nextdns service, verified with dig query",
          outcomeClass: "success",
          confidence: 0.87,
          pluginId: "dns",
          conversationId: "conv-def",
          startedAt: new Date(Date.now() - 7200000).toISOString(),
          endedAt: new Date(Date.now() - 7198500).toISOString(),
          durationMs: 1500,
          exitReason: "goal_achieved",
          piiFlagged: false,
          similarity: 0.76,
        },
      ]);
    } finally {
      setSearching(false);
    }
  };

  const toggleEpisode = (ep: ReasoningEpisode) => {
    const exists = selectedEpisodes.find(
      (s) => s.episodeId === ep.episodeId
    );
    if (exists) {
      onSelectionChange(
        selectedEpisodes.filter((s) => s.episodeId !== ep.episodeId)
      );
    } else {
      onSelectionChange([...selectedEpisodes, ep]);
    }
  };

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border shrink-0">
        <div className="flex items-center gap-2">
          <Database className="h-4 w-4 text-primary" />
          <span className="text-sm font-semibold text-foreground">
            Reasoning Episode Search
          </span>
          <span className="text-[10px] text-muted-foreground font-mono">
            ctl_plane_reasoning_episodes
          </span>
        </div>
        {stats && (
          <span className="text-[10px] text-muted-foreground">
            {stats.pointCount.toLocaleString()} vectors indexed
          </span>
        )}
      </div>

      {/* Search bar + filters */}
      <div className="px-4 py-3 border-b border-border space-y-2 shrink-0">
        <div className="flex gap-2">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder="Semantic search: 'why did the chatbot reconfigure the firewall at 3am?'"
              className="pl-9 h-9 text-sm"
            />
          </div>
          <Button
            onClick={handleSearch}
            disabled={!query.trim() || searching}
            size="sm"
            className="h-9"
          >
            {searching ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Search className="h-3.5 w-3.5" />
            )}
          </Button>
        </div>
        <div className="flex gap-2">
          <select
            value={outcomeFilter}
            onChange={(e) => setOutcomeFilter(e.target.value)}
            className="px-2 py-1 rounded-md border border-input bg-card text-xs focus:border-ring outline-none"
          >
            <option value="">All outcomes</option>
            <option value="success">Success</option>
            <option value="failure">Failure</option>
            <option value="partial">Partial</option>
            <option value="deferred">Deferred</option>
          </select>
          <select
            value={pluginFilter}
            onChange={(e) => setPluginFilter(e.target.value)}
            className="px-2 py-1 rounded-md border border-input bg-card text-xs focus:border-ring outline-none"
          >
            <option value="">All plugins</option>
            <option value="firewall">firewall</option>
            <option value="dns">dns</option>
            <option value="ovs">ovs</option>
            <option value="wireguard">wireguard</option>
            <option value="mail">mail</option>
          </select>
          {selectedEpisodes.length > 0 && (
            <Button
              variant="outline"
              size="sm"
              className="text-xs h-7 ml-auto"
              onClick={() => onSelectionChange([])}
            >
              Clear selection ({selectedEpisodes.length})
            </Button>
          )}
        </div>
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
        {results.length === 0 && !searching && (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground text-sm gap-2">
            <Search className="h-8 w-8 opacity-30" />
            <p>Search reasoning episodes by natural language query</p>
            <p className="text-xs">
              e.g. "why did the chatbot change DNS settings?"
            </p>
          </div>
        )}
        {results.map((ep) => (
          <EpisodeCard
            key={ep.episodeId}
            episode={ep}
            selected={selectedEpisodes.some(
              (s) => s.episodeId === ep.episodeId
            )}
            onToggle={() => toggleEpisode(ep)}
          />
        ))}
      </div>
    </div>
  );
}

// ── Main Page ───────────────────────────────────────────────────────────────

export default function AccountabilityPage() {
  const [selectedEpisodes, setSelectedEpisodes] = useState<
    ReasoningEpisode[]
  >([]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border shrink-0 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-primary/10 flex items-center justify-center">
            <Brain className="h-5 w-5 text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-bold text-foreground">
              Chatbot Accountability
            </h1>
            <p className="text-xs text-muted-foreground">
              Semantic search over reasoning episodes · Chat with context
              from Qdrant vector results
            </p>
          </div>
        </div>
      </div>

      {/* Split view: Chat on top, Search on bottom */}
      <div className="flex-1 min-h-0 flex flex-col">
        {/* Top: Chat */}
        <div className="flex-1 min-h-0 border-b border-border">
          <AccountabilityChat selectedEpisodes={selectedEpisodes} />
        </div>

        {/* Bottom: Qdrant Search */}
        <div className="flex-1 min-h-0">
          <QdrantSearchPanel
            selectedEpisodes={selectedEpisodes}
            onSelectionChange={setSelectedEpisodes}
          />
        </div>
      </div>
    </div>
  );
}
