import { useState, useMemo } from "react";
import { PageHeader, Card, Pill } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { useEventStore } from "@/stores/event-store";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Search, FlaskConical } from "lucide-react";

interface Skill {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  tags: string[];
  enabled: boolean;
  configSchema: Record<string, unknown>;
  configData: Record<string, unknown>;
}

export default function SkillsPage() {
  const latestState = useEventStore((s) => s.latestState);
  const [skillOverrides, setSkillOverrides] = useState<Record<string, Partial<Skill>>>({});
  const [search, setSearch] = useState("");
  const [testOpen, setTestOpen] = useState(false);
  const [testInput, setTestInput] = useState("");
  const [testMessages, setTestMessages] = useState<{ role: "user" | "assistant"; text: string }[]>([]);

  const skills = useMemo(() => {
    const raw = latestState["skills"] ?? latestState["skills.list"] ?? latestState["skills:list"];
    if (!Array.isArray(raw)) return [] as Skill[];
    return (raw as Skill[]).map((skill) => {
      const overrides = skillOverrides[skill.id] ?? {};
      return {
        ...skill,
        enabled: (overrides.enabled ?? skill.enabled) as boolean,
        configData: { ...(skill.configData ?? {}), ...(overrides.configData ?? {}) },
      };
    });
  }, [latestState, skillOverrides]);

  const [selectedId, setSelectedId] = useState<string>("");

  const filtered = useMemo(
    () => skills.filter((s) =>
      s.name.toLowerCase().includes(search.toLowerCase()) ||
      (s.tags ?? []).some((t) => t.toLowerCase().includes(search.toLowerCase()))
    ),
    [skills, search]
  );

  const selected = skills.find((s) => s.id === selectedId);

  const toggleEnabled = (id: string) => {
    setSkillOverrides((prev) => ({
      ...prev,
      [id]: { ...prev[id], enabled: !(skills.find((s) => s.id === id)?.enabled ?? false) },
    }));
  };

  const updateConfig = (id: string, data: unknown) => {
    setSkillOverrides((prev) => ({
      ...prev,
      [id]: { ...prev[id], configData: data as Record<string, unknown> },
    }));
  };

  const handleTestSend = () => {
    if (!testInput.trim() || !selected) return;
    const userMsg = testInput.trim();
    setTestMessages((prev) => [...prev, { role: "user", text: userMsg }]);
    setTestInput("");
    setTimeout(() => {
      setTestMessages((prev) => [
        ...prev,
        { role: "assistant", text: `[${selected.name}] Simulated response for: "${userMsg}"\n\nConfig: ${JSON.stringify(selected.configData, null, 2)}` },
      ]);
    }, 600);
  };

  return (
    <>
      <PageHeader title="Cognitive Skills" subtitle="Browse, configure, and test domain-specific knowledge augmentations." />

      <div className="grid grid-cols-1 lg:grid-cols-[300px_1fr] gap-4 min-h-0">
        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input placeholder="Search skills…" value={search} onChange={(e) => setSearch(e.target.value)} className="pl-8 h-8 text-xs bg-[hsl(var(--bg-elevated))]" />
          </div>
          <div className="space-y-1 max-h-[calc(100vh-220px)] overflow-y-auto pr-1" style={{ scrollbarWidth: "thin" }}>
            {filtered.length === 0 && (
              <p className="text-xs text-muted-foreground text-center py-8">
                {skills.length === 0 ? "No skills detected. Waiting for live data…" : `No skills match "${search}"`}
              </p>
            )}
            {filtered.map((skill) => (
              <button key={skill.id} onClick={() => setSelectedId(skill.id)}
                className={`w-full text-left p-3 rounded-lg border transition-colors ${selectedId === skill.id ? "border-primary/30 bg-primary/5" : "border-transparent hover:bg-muted/30"}`}>
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm font-medium text-foreground truncate">{skill.name}</span>
                  <Switch checked={skill.enabled} onCheckedChange={() => toggleEnabled(skill.id)} onClick={(e) => e.stopPropagation()} className="shrink-0 scale-75" />
                </div>
                <p className="text-[11px] text-muted-foreground mt-0.5 line-clamp-1">{skill.description}</p>
                <div className="flex gap-1 mt-1.5 flex-wrap">
                  {(skill.tags ?? []).slice(0, 3).map((t) => (
                    <Badge key={t} variant="secondary" className="text-[9px] px-1 py-0">{t}</Badge>
                  ))}
                </div>
              </button>
            ))}
          </div>
        </div>

        {selected ? (
          <div className="space-y-4">
            <Card>
              <div className="flex items-center justify-between gap-3 mb-3">
                <div>
                  <h2 className="text-lg font-bold text-foreground">{selected.name}</h2>
                  <p className="text-xs text-muted-foreground mt-0.5">{selected.description}</p>
                </div>
                <Pill variant={selected.enabled ? "ok" : "default"}>{selected.enabled ? "enabled" : "disabled"}</Pill>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline" className="text-[10px]">v{selected.version}</Badge>
                <Badge variant="outline" className="text-[10px]">by {selected.author}</Badge>
                {(selected.tags ?? []).map((t) => <Badge key={t} variant="secondary" className="text-[10px]">{t}</Badge>)}
              </div>
            </Card>
            <Card title="Configuration">
              <div className="mt-2">
                <SchemaRenderer schema={selected.configSchema ?? {}} data={selected.configData ?? {}} onChange={(d) => updateConfig(selected.id, d)} />
              </div>
            </Card>
            <button onClick={() => { setTestMessages([]); setTestOpen(true); }}
              className="flex items-center gap-2 px-4 py-2 rounded-md border border-border text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-muted/30 transition-colors">
              <FlaskConical className="h-4 w-4" /> Test Skill
            </button>
          </div>
        ) : (
          <div className="flex items-center justify-center text-sm text-muted-foreground h-40">
            {skills.length === 0 ? "Waiting for skills data…" : "Select a skill to configure"}
          </div>
        )}
      </div>

      <Dialog open={testOpen} onOpenChange={setTestOpen}>
        <DialogContent className="max-w-lg max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2"><FlaskConical className="h-4 w-4 text-primary" />Test: {selected?.name}</DialogTitle>
            <DialogDescription>Chat with the LLM using this skill's current configuration.</DialogDescription>
          </DialogHeader>
          <div className="flex-1 min-h-0 overflow-y-auto space-y-2 py-2 max-h-[40vh]">
            {testMessages.length === 0 && <p className="text-xs text-muted-foreground text-center py-6">Send a message to test the "{selected?.name}" skill.</p>}
            {testMessages.map((m, i) => (
              <div key={i} className={`p-2.5 rounded-lg text-xs whitespace-pre-wrap ${m.role === "user" ? "bg-primary/10 text-foreground ml-8" : "bg-muted/30 text-foreground mr-8 font-mono"}`}>{m.text}</div>
            ))}
          </div>
          <div className="flex gap-2 pt-2 border-t border-border">
            <Input placeholder="Test this skill…" value={testInput} onChange={(e) => setTestInput(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleTestSend()} className="flex-1 h-8 text-xs" />
            <button onClick={handleTestSend} className="px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors">Send</button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
