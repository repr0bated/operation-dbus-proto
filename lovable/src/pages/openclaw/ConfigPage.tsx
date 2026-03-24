import { useState } from "react";
import { Save, RotateCcw, FileJson, Code2, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

const defaultConfig = `{
  "name": "My OpenClaw",
  "system_prompt": "You are a helpful personal AI assistant.",
  "provider": "anthropic",
  "model": "claude-4-opus",
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-***"
    },
    "openai": {
      "apiKey": "sk-***"
    }
  },
  "gateway": {
    "port": 18789,
    "bind": "loopback",
    "auth": {
      "mode": "token",
      "allowTailscale": true
    },
    "controlUi": {
      "basePath": "/",
      "allowInsecureAuth": false
    }
  },
  "cron": {
    "webhookToken": ""
  },
  "skills": {
    "web-search": { "enabled": true },
    "browser": { "enabled": true },
    "shell": { "enabled": true }
  }
}`;

export default function ConfigPage() {
  const [rawConfig, setRawConfig] = useState(defaultConfig);
  const [hasChanges, setHasChanges] = useState(false);

  const handleChange = (val: string) => {
    setRawConfig(val);
    setHasChanges(true);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Configuration</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Edit ~/.openclaw/openclaw.json — apply & restart with validation
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" className="gap-1.5" onClick={() => { setRawConfig(defaultConfig); setHasChanges(false); }}>
            <RotateCcw className="h-4 w-4" /> Reset
          </Button>
          <Button size="sm" className="gap-1.5" disabled={!hasChanges}>
            <Save className="h-4 w-4" /> Apply & Restart
          </Button>
        </div>
      </div>

      {hasChanges && (
        <div className="flex items-center gap-2 text-sm text-yellow-600 dark:text-yellow-400 bg-yellow-500/10 border border-yellow-500/20 rounded-lg px-3 py-2">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          Unsaved changes. Apply to restart the gateway with the new config.
        </div>
      )}

      <Tabs defaultValue="form">
        <TabsList>
          <TabsTrigger value="form" className="gap-1.5 text-xs">
            <Code2 className="h-3.5 w-3.5" /> Form
          </TabsTrigger>
          <TabsTrigger value="json" className="gap-1.5 text-xs">
            <FileJson className="h-3.5 w-3.5" /> Raw JSON
          </TabsTrigger>
        </TabsList>

        <TabsContent value="form" className="space-y-4 mt-4">
          <Card className="bg-card border-border">
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">General</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <Label className="text-xs">Instance Name</Label>
                  <Input defaultValue="My OpenClaw" className="text-sm" onChange={() => setHasChanges(true)} />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">Default Model</Label>
                  <Select defaultValue="claude-4-opus" onValueChange={() => setHasChanges(true)}>
                    <SelectTrigger className="text-sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="claude-4-opus">claude-4-opus</SelectItem>
                      <SelectItem value="claude-4-sonnet">claude-4-sonnet</SelectItem>
                      <SelectItem value="gpt-5">gpt-5</SelectItem>
                      <SelectItem value="gpt-5-mini">gpt-5-mini</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
              <div className="space-y-2">
                <Label className="text-xs">System Prompt</Label>
                <textarea
                  defaultValue="You are a helpful personal AI assistant."
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm min-h-[80px] resize-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onChange={() => setHasChanges(true)}
                />
              </div>
            </CardContent>
          </Card>

          <Card className="bg-card border-border">
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Gateway</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-3">
                <div className="space-y-2">
                  <Label className="text-xs">Port</Label>
                  <Input type="number" defaultValue="18789" className="text-sm font-mono" onChange={() => setHasChanges(true)} />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">Bind</Label>
                  <Select defaultValue="loopback" onValueChange={() => setHasChanges(true)}>
                    <SelectTrigger className="text-sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="loopback">loopback</SelectItem>
                      <SelectItem value="tailnet">tailnet</SelectItem>
                      <SelectItem value="0.0.0.0">0.0.0.0</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">Auth Mode</Label>
                  <Select defaultValue="token" onValueChange={() => setHasChanges(true)}>
                    <SelectTrigger className="text-sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="token">Token</SelectItem>
                      <SelectItem value="password">Password</SelectItem>
                      <SelectItem value="none">None</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <Label className="text-xs">Allow Tailscale Auth</Label>
                  <p className="text-[10px] text-muted-foreground">Trust Tailscale identity headers</p>
                </div>
                <Switch defaultChecked onChange={() => setHasChanges(true)} />
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="json" className="mt-4">
          <Card className="bg-card border-border">
            <CardContent className="p-0">
              <textarea
                value={rawConfig}
                onChange={(e) => handleChange(e.target.value)}
                className="w-full min-h-[500px] bg-transparent text-sm font-mono text-foreground p-4 resize-none outline-none"
                spellCheck={false}
              />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
