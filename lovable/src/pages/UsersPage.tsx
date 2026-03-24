import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getUsers, createUser, deleteUser, revokeUser, getUserDetail, getUserActivity } from "@/lib/api";
import { Trash2, Ban, Eye, Search, Plus, Download, UserPlus } from "lucide-react";
import { useToast } from "@/hooks/use-toast";

function formatBytes(bytes: number) {
  if (!bytes || bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

function UserDetailDialog({ userId, open, onClose }: { userId: string | null; open: boolean; onClose: () => void }) {
  const detail = useQuery({ queryKey: ["userDetail", userId], queryFn: () => getUserDetail(userId!), enabled: !!userId });
  const activity = useQuery({ queryKey: ["userActivity", userId], queryFn: () => getUserActivity(userId!), enabled: !!userId });
  const user = detail.data ?? {};
  const actData = Array.isArray(activity.data) ? activity.data : activity.data?.activity ?? [];

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader><DialogTitle>{user.email ?? "User Detail"}</DialogTitle></DialogHeader>
        <Tabs defaultValue="overview">
          <TabsList className="bg-secondary mb-3">
            <TabsTrigger value="overview" className="text-xs">Overview</TabsTrigger>
            <TabsTrigger value="vpn" className="text-xs">VPN Config</TabsTrigger>
            <TabsTrigger value="activity" className="text-xs">Activity</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Email</p><p className="text-sm font-mono">{user.email ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">VPN IP</p><p className="text-sm font-mono">{user.vpn_ip ?? user.ip ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Status</p><Badge variant="outline" className={`text-xs ${user.status === "active" ? "text-success" : ""}`}>{user.status ?? "—"}</Badge></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Registered</p><p className="text-sm">{user.registered_at ?? user.created_at ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Last Connection</p><p className="text-sm">{user.last_connection ?? "—"}</p></div>
              <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Data Transfer</p><p className="text-sm">↑ {formatBytes(user.data_up ?? 0)} / ↓ {formatBytes(user.data_down ?? 0)}</p></div>
            </div>
          </TabsContent>
          <TabsContent value="vpn" className="space-y-3">
            <div><p className="text-xs font-medium text-muted-foreground mb-1">WireGuard Configuration</p>
              <pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto whitespace-pre-wrap">{user.wireguard_config ?? "Configuration not available"}</pre>
            </div>
            {user.qr_code && <div><p className="text-xs font-medium text-muted-foreground mb-1">QR Code</p><img src={user.qr_code} alt="QR" className="w-48 h-48 rounded-lg" /></div>}
            <div className="flex gap-2">
              <Button size="sm" variant="outline" className="text-xs gap-1"><Download className="h-3 w-3" />Download Config</Button>
              <Button size="sm" variant="outline" className="text-xs gap-1 text-warning">Regenerate Keys</Button>
            </div>
          </TabsContent>
          <TabsContent value="activity">
            <Table>
              <TableHeader><TableRow className="border-border/50"><TableHead className="text-xs">Time</TableHead><TableHead className="text-xs">Event</TableHead><TableHead className="text-xs">Duration</TableHead><TableHead className="text-xs">Data</TableHead></TableRow></TableHeader>
              <TableBody>
                {actData.length > 0 ? actData.map((a: any, i: number) => (
                  <TableRow key={i} className="border-border/30"><TableCell className="text-xs font-mono">{a.timestamp}</TableCell><TableCell className="text-sm">{a.event}</TableCell><TableCell className="text-xs">{a.duration}</TableCell><TableCell className="text-xs font-mono">{a.data}</TableCell></TableRow>
                )) : <TableRow><TableCell colSpan={4} className="text-center py-6 text-sm text-muted-foreground">{activity.isLoading ? "Loading..." : "No activity"}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

function AddUserDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [email, setEmail] = useState("");
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const createMut = useMutation({ mutationFn: () => createUser({ email }), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["users"] }); toast({ title: "User created" }); onClose(); setEmail(""); } });

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader><DialogTitle>Add User</DialogTitle></DialogHeader>
        <div className="space-y-3">
          <div><Label className="text-xs">Email</Label><Input value={email} onChange={(e) => setEmail(e.target.value)} placeholder="user@example.com" className="bg-secondary border-none mt-1" /></div>
          <Button onClick={() => createMut.mutate()} disabled={!email || createMut.isPending}>{createMut.isPending ? "Creating..." : "Add User"}</Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default function UsersPage() {
  const [search, setSearch] = useState("");
  const [selectedUserId, setSelectedUserId] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const users = useQuery({ queryKey: ["users"], queryFn: getUsers });
  const deleteMut = useMutation({ mutationFn: deleteUser, onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["users"] }); toast({ title: "User deleted" }); } });
  const revokeMut = useMutation({ mutationFn: revokeUser, onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["users"] }); toast({ title: "Access revoked" }); } });

  const data = Array.isArray(users.data) ? users.data : users.data?.users ?? [];
  const filtered = data.filter((u: any) => !search || u.email?.toLowerCase().includes(search.toLowerCase()));

  const exportCsv = () => {
    const csv = ["Email,VPN IP,Status,Registered,Last Connection", ...filtered.map((u: any) => `${u.email},${u.vpn_ip ?? ""},${u.status},${u.registered_at ?? u.created_at ?? ""},${u.last_connection ?? ""}`)].join("\n");
    const blob = new Blob([csv], { type: "text/csv" });
    const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = "users.csv"; a.click();
  };

  return (
    <div className="space-y-6 animate-slide-in">
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div><h1 className="text-2xl font-bold text-foreground">VPN Users</h1><p className="text-sm text-muted-foreground mt-1">Manage VPN users and access</p></div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
            <Input placeholder="Search users..." value={search} onChange={(e) => setSearch(e.target.value)} className="h-8 w-52 pl-8 bg-secondary border-none text-sm" />
          </div>
          <Button size="sm" variant="outline" className="text-xs gap-1 h-8" onClick={exportCsv}><Download className="h-3 w-3" />CSV</Button>
          <Button size="sm" className="text-xs gap-1 h-8" onClick={() => setAddOpen(true)}><UserPlus className="h-3 w-3" />Add User</Button>
        </div>
      </div>
      <Card className="border-border/50">
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow className="border-border/50 hover:bg-transparent">
                <TableHead className="text-xs">Email</TableHead>
                <TableHead className="text-xs">VPN IP</TableHead>
                <TableHead className="text-xs">Status</TableHead>
                <TableHead className="text-xs">Registered</TableHead>
                <TableHead className="text-xs">Last Connection</TableHead>
                <TableHead className="text-xs">Data Transfer</TableHead>
                <TableHead className="text-xs text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.length > 0 ? filtered.map((u: any) => (
                <TableRow key={u.id ?? u.email} className="border-border/30">
                  <TableCell className="text-sm">{u.email}</TableCell>
                  <TableCell className="text-sm font-mono text-muted-foreground">{u.vpn_ip ?? u.ip ?? "—"}</TableCell>
                  <TableCell>
                    <Badge variant="outline" className={`text-xs ${u.status === "active" ? "bg-success/10 text-success border-success/20" : ""}`}>
                      {u.status ?? "unknown"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-sm text-muted-foreground">{u.registered_at ?? u.created_at ?? "—"}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{u.last_connection ?? u.last_seen ?? "—"}</TableCell>
                  <TableCell className="text-xs text-muted-foreground font-mono">
                    {u.data_up || u.data_down ? `↑${formatBytes(u.data_up ?? 0)} ↓${formatBytes(u.data_down ?? 0)}` : "—"}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground" onClick={() => setSelectedUserId(u.id)}><Eye className="h-3.5 w-3.5" /></Button>
                      <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-muted-foreground hover:text-warning" onClick={() => revokeMut.mutate(u.id)}><Ban className="h-3.5 w-3.5" /></Button>
                      <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive" onClick={() => deleteMut.mutate(u.id)}><Trash2 className="h-3.5 w-3.5" /></Button>
                    </div>
                  </TableCell>
                </TableRow>
              )) : (
                <TableRow><TableCell colSpan={7} className="text-center py-8 text-sm text-muted-foreground">{users.isLoading ? "Loading users..." : "No users found"}</TableCell></TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <UserDetailDialog userId={selectedUserId} open={!!selectedUserId} onClose={() => setSelectedUserId(null)} />
      <AddUserDialog open={addOpen} onClose={() => setAddOpen(false)} />
    </div>
  );
}
