import { useState } from "react";
import { Search, Download, Key, CheckCircle, XCircle, Loader2 } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";

interface Skill {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  installed: boolean;
  requiresApiKey: boolean;
  hasApiKey: boolean;
  version: string;
  author: string;
}

const initialSkills: Skill[] = [
  { id: "1", name: "web-search", description: "Search the web using Brave, Google, or DuckDuckGo", enabled: true, installed: true, requiresApiKey: true, hasApiKey: true, version: "2.1.0", author: "openclaw" },
  { id: "2", name: "browser", description: "Browse and interact with web pages via Playwright", enabled: true, installed: true, requiresApiKey: false, hasApiKey: false, version: "1.4.2", author: "openclaw" },
  { id: "3", name: "code-runner", description: "Execute code in sandboxed environments", enabled: true, installed: true, requiresApiKey: false, hasApiKey: false, version: "1.2.0", author: "openclaw" },
  { id: "4", name: "email", description: "Send and read emails via IMAP/SMTP", enabled: false, installed: true, requiresApiKey: true, hasApiKey: false, version: "1.0.3", author: "community" },
  { id: "5", name: "calendar", description: "Google Calendar integration for scheduling", enabled: false, installed: true, requiresApiKey: true, hasApiKey: true, version: "0.9.1", author: "community" },
  { id: "6", name: "image-gen", description: "Generate images with DALL-E, Flux, or Stable Diffusion", enabled: false, installed: false, requiresApiKey: true, hasApiKey: false, version: "1.1.0", author: "openclaw" },
  { id: "7", name: "file-manager", description: "Read, write, and manage local files securely", enabled: true, installed: true, requiresApiKey: false, hasApiKey: false, version: "2.0.0", author: "openclaw" },
  { id: "8", name: "shell", description: "Execute shell commands on the host system", enabled: true, installed: true, requiresApiKey: false, hasApiKey: false, version: "1.3.1", author: "openclaw" },
];

export default function SkillsPage() {
  const [skills, setSkills] = useState(initialSkills);
  const [search, setSearch] = useState("");

  const filtered = skills.filter((s) =>
    s.name.toLowerCase().includes(search.toLowerCase()) ||
    s.description.toLowerCase().includes(search.toLowerCase())
  );

  const toggleSkill = (id: string) => {
    setSkills((prev) =>
      prev.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s))
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Skills</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Enable, disable, and manage skill plugins
          </p>
        </div>
        <Badge variant="secondary" className="text-xs">
          {skills.filter((s) => s.enabled).length} / {skills.length} enabled
        </Badge>
      </div>

      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search skills..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-9"
        />
      </div>

      <div className="grid gap-3">
        {filtered.map((skill) => (
          <Card key={skill.id} className="bg-card border-border">
            <CardContent className="p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3 flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    {skill.installed ? (
                      <CheckCircle className="h-4 w-4 text-green-500 shrink-0" />
                    ) : (
                      <XCircle className="h-4 w-4 text-muted-foreground shrink-0" />
                    )}
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-sm font-semibold text-foreground">{skill.name}</span>
                      <Badge variant="outline" className="text-[10px] h-5">v{skill.version}</Badge>
                      <span className="text-[10px] text-muted-foreground">by {skill.author}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-0.5 truncate">{skill.description}</p>
                  </div>
                </div>

                <div className="flex items-center gap-3 shrink-0 ml-4">
                  {skill.requiresApiKey && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className={`h-7 text-xs gap-1 ${skill.hasApiKey ? "text-green-600 dark:text-green-400" : "text-yellow-600 dark:text-yellow-400"}`}
                    >
                      <Key className="h-3 w-3" />
                      {skill.hasApiKey ? "Key set" : "Set key"}
                    </Button>
                  )}
                  {!skill.installed ? (
                    <Button size="sm" className="h-7 text-xs gap-1">
                      <Download className="h-3 w-3" /> Install
                    </Button>
                  ) : (
                    <Switch
                      checked={skill.enabled}
                      onCheckedChange={() => toggleSkill(skill.id)}
                    />
                  )}
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
