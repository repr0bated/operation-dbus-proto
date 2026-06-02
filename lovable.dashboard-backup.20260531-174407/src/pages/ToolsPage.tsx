import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { Search, Wrench, LayoutGrid, List, Play, Clock, ToggleLeft, ToggleRight } from "lucide-react";
import { getBuiltinTools, getMcpTools, getCustomTools, getToolHistory, executeTool, toggleTool } from "@/lib/api";

function ToolCard({ tool, onSelect }: { tool: any; onSelect: () => void }) {
  return (
    <Card className="border-border/50 card-glow cursor-pointer hover:border-primary/30 transition-colors" onClick={onSelect}>
      <CardContent className="p-4">
        <div className="flex items-start justify-between mb-2">
          <div className="p-2 rounded-lg bg-primary/10 text-primary">
            <Wrench className="h-4 w-4" />
          </div>
          <Badge variant="outline" className="text-[10px]">{tool.category ?? "general"}</Badge>
        </div>
        <h3 className="text-sm font-semibold text-foreground mt-2">{tool.name}</h3>
        <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{tool.description}</p>
        <div className="flex items-center gap-3 mt-3 text-xs text-muted-foreground">
          <span className="flex items-center gap-1"><Play className="h-3 w-3" />{tool.usage_count ?? 0} runs</span>
          {tool.last_used && <span className="flex items-center gap-1"><Clock className="h-3 w-3" />{tool.last_used}</span>}
        </div>
        <div className="mt-2 flex items-center gap-1">
          {tool.enabled !== false ? (
            <Badge className="text-[10px] bg-success/10 text-success border-success/20" variant="outline">Enabled</Badge>
          ) : (
            <Badge className="text-[10px]" variant="secondary">Disabled</Badge>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function ToolDetailDialog({ tool, open, onClose }: { tool: any; open: boolean; onClose: () => void }) {
  const [testInput, setTestInput] = useState("{}");
  const [testOutput, setTestOutput] = useState<string | null>(null);
  const history = useQuery({ queryKey: ["toolHistory", tool?.id], queryFn: () => getToolHistory(tool?.id), enabled: !!tool?.id });
  const execMut = useMutation({ mutationFn: (input: any) => executeTool(tool?.id, input) });

  const handleExecute = () => {
    try {
      const parsed = JSON.parse(testInput);
      execMut.mutate(parsed, { onSuccess: (d) => setTestOutput(JSON.stringify(d, null, 2)) });
    } catch { setTestOutput("Invalid JSON input"); }
  };

  if (!tool) return null;
  const historyData = Array.isArray(history.data) ? history.data : history.data?.history ?? [];

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {tool.name}
            <Badge variant="outline" className="text-xs">{tool.category ?? "general"}</Badge>
          </DialogTitle>
        </DialogHeader>
        <Tabs defaultValue="overview">
          <TabsList className="bg-secondary mb-3">
            <TabsTrigger value="overview" className="text-xs">Overview</TabsTrigger>
            <TabsTrigger value="history" className="text-xs">History</TabsTrigger>
            <TabsTrigger value="test" className="text-xs">Test</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="space-y-3">
            <p className="text-sm text-muted-foreground">{tool.description}</p>
            {tool.parameters && (
              <div>
                <p className="text-xs font-medium text-muted-foreground uppercase mb-1">Parameters</p>
                <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto">{JSON.stringify(tool.parameters, null, 2)}</pre>
              </div>
            )}
          </TabsContent>
          <TabsContent value="history">
            <Table>
              <TableHeader>
                <TableRow className="border-border/50"><TableHead className="text-xs">Time</TableHead><TableHead className="text-xs">Duration</TableHead><TableHead className="text-xs">Status</TableHead></TableRow>
              </TableHeader>
              <TableBody>
                {historyData.length > 0 ? historyData.slice(0, 20).map((h: any, i: number) => (
                  <TableRow key={i} className="border-border/30">
                    <TableCell className="text-xs font-mono">{h.timestamp}</TableCell>
                    <TableCell className="text-xs">{h.duration}ms</TableCell>
                    <TableCell><Badge variant="outline" className={`text-[10px] ${h.status === "success" ? "text-success" : "text-destructive"}`}>{h.status}</Badge></TableCell>
                  </TableRow>
                )) : <TableRow><TableCell colSpan={3} className="text-center text-sm text-muted-foreground py-6">{history.isLoading ? "Loading..." : "No history"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </TabsContent>
          <TabsContent value="test" className="space-y-3">
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-1">Input (JSON)</p>
              <Textarea value={testInput} onChange={(e) => setTestInput(e.target.value)} className="font-mono text-xs bg-secondary border-none min-h-[100px]" />
            </div>
            <Button size="sm" onClick={handleExecute} disabled={execMut.isPending} className="gap-1.5">
              <Play className="h-3 w-3" />{execMut.isPending ? "Running..." : "Execute"}
            </Button>
            {testOutput && (
              <div>
                <p className="text-xs font-medium text-muted-foreground mb-1">Output</p>
                <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-48">{testOutput}</pre>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

export default function ToolsPage() {
  const [search, setSearch] = useState("");
  const [view, setView] = useState<"grid" | "list">("grid");
  const [selectedTool, setSelectedTool] = useState<any>(null);

  const builtin = useQuery({ queryKey: ["builtinTools"], queryFn: getBuiltinTools });
  const mcpTools = useQuery({ queryKey: ["mcpTools"], queryFn: getMcpTools });
  const custom = useQuery({ queryKey: ["customTools"], queryFn: getCustomTools });

  const normalize = (d: any) => Array.isArray(d) ? d : d?.tools ?? [];
  const allTools = [...normalize(builtin.data), ...normalize(mcpTools.data), ...normalize(custom.data)];
  const filtered = allTools.filter((t) => !search || t.name?.toLowerCase().includes(search.toLowerCase()) || t.description?.toLowerCase().includes(search.toLowerCase()));
  const isLoading = builtin.isLoading || mcpTools.isLoading || custom.isLoading;

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Tools</h1>
          <p className="text-sm text-muted-foreground mt-1">Tool library & management</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input placeholder="Search tools..." value={search} onChange={(e) => setSearch(e.target.value)} className="h-8 w-56 pl-8 bg-secondary border-none text-sm" />
          </div>
          <div className="flex border border-border rounded-md">
            <Button size="sm" variant={view === "grid" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("grid")}><LayoutGrid className="h-3.5 w-3.5" /></Button>
            <Button size="sm" variant={view === "list" ? "secondary" : "ghost"} className="h-8 px-2" onClick={() => setView("list")}><List className="h-3.5 w-3.5" /></Button>
          </div>
        </div>
      </div>

      {view === "grid" ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {filtered.length > 0 ? filtered.map((tool: any) => (
            <ToolCard key={tool.id ?? tool.name} tool={tool} onSelect={() => setSelectedTool(tool)} />
          )) : (
            <div className="col-span-full text-center py-12 text-sm text-muted-foreground">
              {isLoading ? "Loading tools..." : "No tools found"}
            </div>
          )}
        </div>
      ) : (
        <Card className="border-border/50">
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow className="border-border/50"><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Category</TableHead><TableHead className="text-xs">Usage</TableHead><TableHead className="text-xs">Status</TableHead></TableRow>
              </TableHeader>
              <TableBody>
                {filtered.length > 0 ? filtered.map((t: any) => (
                  <TableRow key={t.id ?? t.name} className="border-border/30 cursor-pointer hover:bg-accent/50" onClick={() => setSelectedTool(t)}>
                    <TableCell className="text-sm font-medium">{t.name}</TableCell>
                    <TableCell><Badge variant="outline" className="text-[10px]">{t.category ?? "general"}</Badge></TableCell>
                    <TableCell className="text-sm text-muted-foreground">{t.usage_count ?? 0}</TableCell>
                    <TableCell><Badge variant="outline" className={`text-[10px] ${t.enabled !== false ? "text-success" : "text-muted-foreground"}`}>{t.enabled !== false ? "Enabled" : "Disabled"}</Badge></TableCell>
                  </TableRow>
                )) : <TableRow><TableCell colSpan={4} className="text-center py-8 text-sm text-muted-foreground">{isLoading ? "Loading..." : "No tools"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <ToolDetailDialog tool={selectedTool} open={!!selectedTool} onClose={() => setSelectedTool(null)} />
    </div>
  );
}
