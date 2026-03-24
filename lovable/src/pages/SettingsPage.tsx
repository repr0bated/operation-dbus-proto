import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { getSettings, updateSettings, getApiKeys, createApiKey, revokeApiKey, sendTestEmail, exportConfig, backupDatabase } from "@/lib/api";
import { useToast } from "@/hooks/use-toast";
import { Copy, Trash2, Plus, Download, Upload, Database, Send } from "lucide-react";

function GeneralTab() {
  const { toast } = useToast();
  const settings = useQuery({ queryKey: ["settings", "general"], queryFn: () => getSettings("general") });
  const [values, setValues] = useState<Record<string, any>>({});
  const save = useMutation({ mutationFn: () => updateSettings("general", { ...settings.data, ...values }), onSuccess: () => toast({ title: "Settings saved" }) });
  const val = (k: string) => values[k] ?? settings.data?.[k] ?? "";

  return (
    <Card className="border-border/50"><CardContent className="p-6 space-y-4">
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Site Name</Label><Input value={val("site_name")} onChange={(e) => setValues({ ...values, site_name: e.target.value })} className="bg-secondary border-none" /></div>
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Base URL</Label><Input value={val("base_url")} onChange={(e) => setValues({ ...values, base_url: e.target.value })} className="bg-secondary border-none" placeholder="https://3tched.com" /></div>
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Time Zone</Label>
        <Select value={val("timezone") || "UTC"} onValueChange={(v) => setValues({ ...values, timezone: v })}><SelectTrigger className="bg-secondary border-none"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="UTC">UTC</SelectItem><SelectItem value="US/Eastern">US/Eastern</SelectItem><SelectItem value="US/Pacific">US/Pacific</SelectItem><SelectItem value="Europe/London">Europe/London</SelectItem></SelectContent></Select>
      </div>
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Language</Label>
        <Select value={val("language") || "en"} onValueChange={(v) => setValues({ ...values, language: v })}><SelectTrigger className="bg-secondary border-none"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="en">English</SelectItem><SelectItem value="es">Spanish</SelectItem><SelectItem value="fr">French</SelectItem></SelectContent></Select>
      </div>
      <Button onClick={() => save.mutate()} disabled={save.isPending}>Save Changes</Button>
    </CardContent></Card>
  );
}

function SmtpTab() {
  const { toast } = useToast();
  const settings = useQuery({ queryKey: ["settings", "smtp"], queryFn: () => getSettings("smtp") });
  const [values, setValues] = useState<Record<string, string>>({});
  const save = useMutation({ mutationFn: () => updateSettings("smtp", { ...settings.data, ...values }), onSuccess: () => toast({ title: "SMTP settings saved" }) });
  const testMut = useMutation({ mutationFn: sendTestEmail, onSuccess: () => toast({ title: "Test email sent" }), onError: () => toast({ title: "Test email failed", variant: "destructive" }) });
  const val = (k: string) => values[k] ?? settings.data?.[k] ?? "";

  return (
    <Card className="border-border/50"><CardContent className="p-6 space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Host</Label><Input value={val("smtp_host")} onChange={(e) => setValues({ ...values, smtp_host: e.target.value })} className="bg-secondary border-none" placeholder="10.149.181.121" /></div>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Port</Label><Input value={val("smtp_port")} onChange={(e) => setValues({ ...values, smtp_port: e.target.value })} className="bg-secondary border-none" placeholder="587" /></div>
      </div>
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Username</Label><Input value={val("smtp_user")} onChange={(e) => setValues({ ...values, smtp_user: e.target.value })} className="bg-secondary border-none" placeholder="jeremy@3tched.com" /></div>
      <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Password</Label><Input type="password" value={val("smtp_pass")} onChange={(e) => setValues({ ...values, smtp_pass: e.target.value })} className="bg-secondary border-none" /></div>
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">From Email</Label><Input value={val("from_email")} onChange={(e) => setValues({ ...values, from_email: e.target.value })} className="bg-secondary border-none" placeholder="noreply@3tched.com" /></div>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">From Name</Label><Input value={val("from_name")} onChange={(e) => setValues({ ...values, from_name: e.target.value })} className="bg-secondary border-none" placeholder="Operation DBUS" /></div>
      </div>
      <div className="flex gap-2">
        <Button onClick={() => save.mutate()} disabled={save.isPending}>Save</Button>
        <Button variant="outline" onClick={() => testMut.mutate()} disabled={testMut.isPending} className="gap-1"><Send className="h-3.5 w-3.5" />{testMut.isPending ? "Sending..." : "Test Email"}</Button>
      </div>
    </CardContent></Card>
  );
}

function SecurityTab() {
  const { toast } = useToast();
  const settings = useQuery({ queryKey: ["settings", "security"], queryFn: () => getSettings("security") });
  const apiKeys = useQuery({ queryKey: ["apiKeys"], queryFn: getApiKeys });
  const [keyName, setKeyName] = useState("");
  const [sessionTimeout, setSessionTimeout] = useState("");
  const [require2fa, setRequire2fa] = useState(false);
  const createKeyMut = useMutation({ mutationFn: () => createApiKey(keyName), onSuccess: () => { apiKeys.refetch(); setKeyName(""); toast({ title: "API key created" }); } });
  const revokeKeyMut = useMutation({ mutationFn: revokeApiKey, onSuccess: () => { apiKeys.refetch(); toast({ title: "Key revoked" }); } });
  const saveMut = useMutation({ mutationFn: () => updateSettings("security", { session_timeout: sessionTimeout || settings.data?.session_timeout, require_2fa: require2fa }), onSuccess: () => toast({ title: "Security settings saved" }) });

  const keys = Array.isArray(apiKeys.data) ? apiKeys.data : apiKeys.data?.keys ?? [];

  return (
    <div className="space-y-4">
      <Card className="border-border/50"><CardContent className="p-6 space-y-4">
        <h3 className="text-sm font-medium text-foreground">API Keys</h3>
        <div className="flex gap-2">
          <Input value={keyName} onChange={(e) => setKeyName(e.target.value)} placeholder="Key name..." className="bg-secondary border-none flex-1 h-8 text-sm" />
          <Button size="sm" onClick={() => createKeyMut.mutate()} disabled={!keyName || createKeyMut.isPending} className="gap-1 h-8"><Plus className="h-3 w-3" />Generate</Button>
        </div>
        <Table>
          <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Name</TableHead><TableHead className="text-xs">Created</TableHead><TableHead className="text-xs">Last Used</TableHead><TableHead className="text-xs text-right">Actions</TableHead></TableRow></TableHeader>
          <TableBody>
            {keys.length > 0 ? keys.map((k: any) => (
              <TableRow key={k.id ?? k.name} className="border-border/30">
                <TableCell className="text-sm">{k.name}</TableCell><TableCell className="text-xs text-muted-foreground">{k.created ?? "—"}</TableCell><TableCell className="text-xs text-muted-foreground">{k.last_used ?? "Never"}</TableCell>
                <TableCell className="text-right"><div className="flex items-center justify-end gap-1">
                  <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => { navigator.clipboard.writeText(k.key ?? k.id); toast({ title: "Copied" }); }}><Copy className="h-3 w-3" /></Button>
                  <Button size="sm" variant="ghost" className="h-6 w-6 p-0 text-destructive" onClick={() => revokeKeyMut.mutate(k.id)}><Trash2 className="h-3 w-3" /></Button>
                </div></TableCell>
              </TableRow>
            )) : <TableRow><TableCell colSpan={4} className="text-center py-4 text-sm text-muted-foreground">No API keys</TableCell></TableRow>}
          </TableBody>
        </Table>
      </CardContent></Card>

      <Card className="border-border/50"><CardContent className="p-6 space-y-4">
        <h3 className="text-sm font-medium text-foreground">Session Settings</h3>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Session Timeout (minutes)</Label><Input type="number" value={sessionTimeout || settings.data?.session_timeout || ""} onChange={(e) => setSessionTimeout(e.target.value)} className="bg-secondary border-none w-32" /></div>
        <div className="flex items-center gap-3"><Switch checked={require2fa} onCheckedChange={setRequire2fa} /><Label className="text-sm">Require 2FA</Label></div>
        <Button onClick={() => saveMut.mutate()} disabled={saveMut.isPending}>Save Security Settings</Button>
      </CardContent></Card>
    </div>
  );
}

function IntegrationsTab() {
  const { toast } = useToast();
  const settings = useQuery({ queryKey: ["settings", "integrations"], queryFn: () => getSettings("integrations") });
  const [values, setValues] = useState<Record<string, any>>({});
  const save = useMutation({ mutationFn: () => updateSettings("integrations", { ...settings.data, ...values }), onSuccess: () => toast({ title: "Integrations saved" }) });
  const val = (k: string) => values[k] ?? settings.data?.[k] ?? "";

  return (
    <div className="space-y-4">
      <Card className="border-border/50"><CardContent className="p-6 space-y-4">
        <h3 className="text-sm font-medium text-foreground">Google OAuth</h3>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Client ID</Label><Input value={val("google_client_id")} onChange={(e) => setValues({ ...values, google_client_id: e.target.value })} className="bg-secondary border-none" /></div>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Client Secret</Label><Input type="password" value={val("google_client_secret")} onChange={(e) => setValues({ ...values, google_client_secret: e.target.value })} className="bg-secondary border-none" /></div>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Redirect URL</Label><Input value={val("google_redirect_url") || "https://3tched.com/auth/callback"} readOnly className="bg-secondary border-none opacity-60" /></div>
        <div className="flex items-center gap-3"><Switch checked={values.google_enabled ?? settings.data?.google_enabled ?? false} onCheckedChange={(v) => setValues({ ...values, google_enabled: v })} /><Label className="text-sm">Enable Google OAuth</Label></div>
      </CardContent></Card>

      <Card className="border-border/50"><CardContent className="p-6 space-y-4">
        <h3 className="text-sm font-medium text-foreground">Webhooks</h3>
        <div className="space-y-1.5"><Label className="text-xs text-muted-foreground">Webhook URL</Label><Input value={val("webhook_url")} onChange={(e) => setValues({ ...values, webhook_url: e.target.value })} className="bg-secondary border-none" placeholder="https://..." /></div>
        <Button variant="outline" size="sm" className="text-xs">Test Webhook</Button>
      </CardContent></Card>

      <Button onClick={() => save.mutate()} disabled={save.isPending}>Save Integrations</Button>
    </div>
  );
}

function BackupTab() {
  const { toast } = useToast();
  const exportMut = useMutation({ mutationFn: exportConfig, onSuccess: (data) => {
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = "config-export.json"; a.click();
    toast({ title: "Configuration exported" });
  }});
  const backupMut = useMutation({ mutationFn: backupDatabase, onSuccess: () => toast({ title: "Database backup started" }) });

  return (
    <Card className="border-border/50"><CardContent className="p-6 space-y-4">
      <h3 className="text-sm font-medium text-foreground">Backup & Restore</h3>
      <div className="flex gap-3 flex-wrap">
        <Button variant="outline" className="gap-1.5" onClick={() => exportMut.mutate()} disabled={exportMut.isPending}><Download className="h-3.5 w-3.5" />{exportMut.isPending ? "Exporting..." : "Export Configuration"}</Button>
        <Button variant="outline" className="gap-1.5" onClick={() => document.getElementById("import-config")?.click()}><Upload className="h-3.5 w-3.5" />Import Configuration</Button>
        <input id="import-config" type="file" accept=".json" className="hidden" onChange={() => toast({ title: "Import not yet implemented" })} />
        <Button variant="outline" className="gap-1.5" onClick={() => backupMut.mutate()} disabled={backupMut.isPending}><Database className="h-3.5 w-3.5" />{backupMut.isPending ? "Backing up..." : "Backup Database"}</Button>
      </div>
    </CardContent></Card>
  );
}

export default function SettingsPage() {
  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">Settings</h1>
        <p className="text-sm text-muted-foreground mt-1">System configuration</p>
      </div>
      <Tabs defaultValue="general">
        <TabsList className="bg-secondary mb-4">
          <TabsTrigger value="general" className="text-xs">General</TabsTrigger>
          <TabsTrigger value="smtp" className="text-xs">SMTP</TabsTrigger>
          <TabsTrigger value="security" className="text-xs">Security</TabsTrigger>
          <TabsTrigger value="integrations" className="text-xs">Integrations</TabsTrigger>
          <TabsTrigger value="backup" className="text-xs">Backup</TabsTrigger>
        </TabsList>
        <TabsContent value="general"><GeneralTab /></TabsContent>
        <TabsContent value="smtp"><SmtpTab /></TabsContent>
        <TabsContent value="security"><SecurityTab /></TabsContent>
        <TabsContent value="integrations"><IntegrationsTab /></TabsContent>
        <TabsContent value="backup"><BackupTab /></TabsContent>
      </Tabs>
    </div>
  );
}
