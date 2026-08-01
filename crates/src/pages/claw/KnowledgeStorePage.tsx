import { useState } from "react";
import {
  Search,
  GitBranch,
  Database,
  RefreshCw,
  CheckCircle2,
  Clock,
  AlertCircle,
  FileCode2,
  Loader2,
  ArrowUpDown,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type IndexStatus = "indexed" | "indexing" | "queued" | "error";

interface RepoEntry {
  id: string;
  name: string;
  url: string;
  branch: string;
  language: string;
  files: number;
  chunks: number;
  vectors: number;
  status: IndexStatus;
  progress: number;
  lastIndexed: string | null;
  sizeKb: number;
}

const MOCK_REPOS: RepoEntry[] = [
  {
    id: "1",
    name: "op-core",
    url: "git@forge.3tched.com:op/op-core.git",
    branch: "main",
    language: "Rust",
    files: 342,
    chunks: 8_420,
    vectors: 8_420,
    status: "indexed",
    progress: 100,
    lastIndexed: "2026-02-17T08:12:00Z",
    sizeKb: 14_200,
  },
  {
    id: "2",
    name: "op-llm",
    url: "git@forge.3tched.com:op/op-llm.git",
    branch: "main",
    language: "Rust",
    files: 89,
    chunks: 2_140,
    vectors: 2_140,
    status: "indexed",
    progress: 100,
    lastIndexed: "2026-02-17T07:55:00Z",
    sizeKb: 3_800,
  },
  {
    id: "3",
    name: "op-web",
    url: "git@forge.3tched.com:op/op-web.git",
    branch: "develop",
    language: "TypeScript",
    files: 214,
    chunks: 5_680,
    vectors: 5_680,
    status: "indexed",
    progress: 100,
    lastIndexed: "2026-02-17T06:30:00Z",
    sizeKb: 8_900,
  },
  {
    id: "4",
    name: "zeroclaw-agents",
    url: "git@forge.3tched.com:claw/zeroclaw-agents.git",
    branch: "main",
    language: "Python",
    files: 156,
    chunks: 3_910,
    vectors: 2_480,
    status: "indexing",
    progress: 63,
    lastIndexed: null,
    sizeKb: 6_100,
  },
  {
    id: "5",
    name: "infra-nix",
    url: "git@forge.3tched.com:ops/infra-nix.git",
    branch: "main",
    language: "Nix",
    files: 78,
    chunks: 1_200,
    vectors: 0,
    status: "queued",
    progress: 0,
    lastIndexed: null,
    sizeKb: 2_400,
  },
  {
    id: "6",
    name: "mcp-toolkit",
    url: "git@forge.3tched.com:claw/mcp-toolkit.git",
    branch: "main",
    language: "Rust",
    files: 45,
    chunks: 0,
    vectors: 0,
    status: "error",
    progress: 0,
    lastIndexed: null,
    sizeKb: 1_100,
  },
];

const statusConfig: Record<IndexStatus, { icon: React.ElementType; label: string; className: string }> = {
  indexed: { icon: CheckCircle2, label: "Indexed", className: "text-emerald-400 bg-emerald-400/10 border-emerald-400/20" },
  indexing: { icon: Loader2, label: "Indexing", className: "text-blue-400 bg-blue-400/10 border-blue-400/20" },
  queued: { icon: Clock, label: "Queued", className: "text-amber-400 bg-amber-400/10 border-amber-400/20" },
  error: { icon: AlertCircle, label: "Error", className: "text-red-400 bg-red-400/10 border-red-400/20" },
};

function formatSize(kb: number) {
  return kb >= 1000 ? `${(kb / 1000).toFixed(1)} MB` : `${kb} KB`;
}

function formatNumber(n: number) {
  return n.toLocaleString();
}

function timeAgo(iso: string) {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

export default function KnowledgeStorePage() {
  const [filter, setFilter] = useState("");

  const repos = MOCK_REPOS.filter(
    (r) =>
      r.name.toLowerCase().includes(filter.toLowerCase()) ||
      r.language.toLowerCase().includes(filter.toLowerCase())
  );

  const totalFiles = MOCK_REPOS.reduce((s, r) => s + r.files, 0);
  const totalChunks = MOCK_REPOS.reduce((s, r) => s + r.chunks, 0);
  const totalVectors = MOCK_REPOS.reduce((s, r) => s + r.vectors, 0);
  const indexedCount = MOCK_REPOS.filter((r) => r.status === "indexed").length;

  return (
    <div className="flex flex-col gap-6 p-6">
      <div>
        <h1 className="text-xl font-semibold tracking-tight text-foreground">
          Knowledge Store
        </h1>
        <p className="text-sm text-muted-foreground mt-1">
          Repository index for semantic code search via Qdrant
        </p>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <Card className="bg-card border-border">
          <CardHeader className="pb-1 pt-4 px-4">
            <CardTitle className="text-[11px] uppercase tracking-wider text-muted-foreground font-medium">
              Repositories
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-4">
            <div className="text-2xl font-mono font-bold text-foreground">
              {MOCK_REPOS.length}
            </div>
            <p className="text-[11px] text-muted-foreground">
              {indexedCount} fully indexed
            </p>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardHeader className="pb-1 pt-4 px-4">
            <CardTitle className="text-[11px] uppercase tracking-wider text-muted-foreground font-medium">
              Files Scanned
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-4">
            <div className="text-2xl font-mono font-bold text-foreground">
              {formatNumber(totalFiles)}
            </div>
            <p className="text-[11px] text-muted-foreground">across all repos</p>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardHeader className="pb-1 pt-4 px-4">
            <CardTitle className="text-[11px] uppercase tracking-wider text-muted-foreground font-medium">
              Chunks
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-4">
            <div className="text-2xl font-mono font-bold text-foreground">
              {formatNumber(totalChunks)}
            </div>
            <p className="text-[11px] text-muted-foreground">code segments</p>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardHeader className="pb-1 pt-4 px-4">
            <CardTitle className="text-[11px] uppercase tracking-wider text-muted-foreground font-medium">
              Vectors
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-4">
            <div className="text-2xl font-mono font-bold text-foreground">
              {formatNumber(totalVectors)}
            </div>
            <p className="text-[11px] text-muted-foreground">in Qdrant</p>
          </CardContent>
        </Card>
      </div>

      {/* Filter + actions */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Filter repos…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="pl-9 bg-card border-border text-sm"
          />
        </div>
        <Button variant="outline" size="sm" className="gap-1.5">
          <RefreshCw className="h-3.5 w-3.5" />
          Re-index All
        </Button>
      </div>

      {/* Repo table */}
      <div className="rounded-lg border border-border overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow className="bg-muted/30 hover:bg-muted/30">
              <TableHead className="text-[11px] uppercase tracking-wider font-medium">
                Repository
              </TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium">
                <div className="flex items-center gap-1">
                  Branch
                  <GitBranch className="h-3 w-3" />
                </div>
              </TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium">Lang</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium text-right">
                <div className="flex items-center justify-end gap-1">
                  Files
                  <ArrowUpDown className="h-3 w-3" />
                </div>
              </TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium text-right">Chunks</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium text-right">Vectors</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium text-right">Size</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium">Status</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wider font-medium text-right">Last Indexed</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {repos.map((repo) => {
              const sc = statusConfig[repo.status];
              const StatusIcon = sc.icon;
              return (
                <TableRow key={repo.id} className="hover:bg-muted/20">
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <FileCode2 className="h-4 w-4 text-muted-foreground shrink-0" />
                      <div>
                        <p className="text-sm font-medium text-foreground">{repo.name}</p>
                        <p className="text-[11px] font-mono text-muted-foreground truncate max-w-[220px]">
                          {repo.url}
                        </p>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    <code className="text-xs font-mono text-muted-foreground bg-muted/40 px-1.5 py-0.5 rounded">
                      {repo.branch}
                    </code>
                  </TableCell>
                  <TableCell>
                    <span className="text-xs text-muted-foreground">{repo.language}</span>
                  </TableCell>
                  <TableCell className="text-right font-mono text-sm text-foreground">
                    {formatNumber(repo.files)}
                  </TableCell>
                  <TableCell className="text-right font-mono text-sm text-foreground">
                    {formatNumber(repo.chunks)}
                  </TableCell>
                  <TableCell className="text-right font-mono text-sm text-foreground">
                    {formatNumber(repo.vectors)}
                  </TableCell>
                  <TableCell className="text-right text-xs text-muted-foreground">
                    {formatSize(repo.sizeKb)}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-1">
                      <Badge
                        variant="outline"
                        className={`gap-1 text-[11px] w-fit ${sc.className}`}
                      >
                        <StatusIcon className={`h-3 w-3 ${repo.status === "indexing" ? "animate-spin" : ""}`} />
                        {sc.label}
                      </Badge>
                      {repo.status === "indexing" && (
                        <Progress value={repo.progress} className="h-1 w-20" />
                      )}
                    </div>
                  </TableCell>
                  <TableCell className="text-right text-xs text-muted-foreground">
                    {repo.lastIndexed ? timeAgo(repo.lastIndexed) : "—"}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </div>

      {/* Qdrant stats footer */}
      <div className="flex items-center gap-4 text-[11px] text-muted-foreground border-t border-border pt-4">
        <div className="flex items-center gap-1.5">
          <Database className="h-3.5 w-3.5" />
          <span>Qdrant collection: <code className="font-mono text-foreground">zeroclaw_code</code></span>
        </div>
        <span>•</span>
        <span>Embedding model: <code className="font-mono text-foreground">nomic-embed-text</code></span>
        <span>•</span>
        <span>Dimension: <code className="font-mono text-foreground">768</code></span>
      </div>
    </div>
  );
}