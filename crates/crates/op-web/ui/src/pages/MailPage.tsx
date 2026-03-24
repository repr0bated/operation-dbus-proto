import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Mail, CheckCircle, XCircle, AlertTriangle, Server, RefreshCw, Eye } from "lucide-react";
import { getMailStats, getMailQueue, getMailDnsStatus, getRecentEmails, getMailServerStatus, resendEmail, getEmailDetail } from "@/lib/api";
import { useToast } from "@/hooks/use-toast";

function EmailDetailDialog({ emailId, open, onClose }: { emailId: string | null; open: boolean; onClose: () => void }) {
  const detail = useQuery({ queryKey: ["emailDetail", emailId], queryFn: () => getEmailDetail(emailId!), enabled: !!emailId });
  const email = detail.data ?? {};

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-auto">
        <DialogHeader><DialogTitle>Email Detail</DialogTitle></DialogHeader>
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">From</p><p className="text-sm font-mono">{email.from ?? "—"}</p></div>
            <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">To</p><p className="text-sm font-mono">{email.to ?? "—"}</p></div>
            <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Subject</p><p className="text-sm">{email.subject ?? "—"}</p></div>
            <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">Status</p><Badge variant="outline" className={`text-xs ${email.status === "delivered" ? "text-success" : "text-destructive"}`}>{email.status ?? "—"}</Badge></div>
          </div>
          {email.headers && <div><p className="text-xs font-medium text-muted-foreground mb-1">Headers</p><pre className="text-xs font-mono bg-secondary p-3 rounded-lg overflow-auto max-h-32">{typeof email.headers === "string" ? email.headers : JSON.stringify(email.headers, null, 2)}</pre></div>}
          {email.body && <div><p className="text-xs font-medium text-muted-foreground mb-1">Body Preview</p><div className="text-sm bg-secondary p-3 rounded-lg max-h-40 overflow-auto">{email.body}</div></div>}
          {email.attachments && <div><p className="text-xs font-medium text-muted-foreground mb-1">Attachments</p>{Array.isArray(email.attachments) ? email.attachments.map((a: any, i: number) => <Badge key={i} variant="outline" className="text-xs mr-1">{a.name ?? a}</Badge>) : <span className="text-xs text-muted-foreground">None</span>}</div>}
          <div className="grid grid-cols-2 gap-3">
            {email.spf_result && <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">SPF</p><p className="text-sm">{email.spf_result}</p></div>}
            {email.dkim_result && <div className="p-3 rounded-lg bg-secondary"><p className="text-xs text-muted-foreground">DKIM</p><p className="text-sm">{email.dkim_result}</p></div>}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default function MailPage() {
  const { toast } = useToast();
  const [selectedEmailId, setSelectedEmailId] = useState<string | null>(null);
  const stats = useQuery({ queryKey: ["mailStats"], queryFn: getMailStats, refetchInterval: 10000 });
  const queue = useQuery({ queryKey: ["mailQueue"], queryFn: getMailQueue });
  const dns = useQuery({ queryKey: ["mailDns"], queryFn: getMailDnsStatus });
  const emails = useQuery({ queryKey: ["recentEmails"], queryFn: getRecentEmails });
  const serverStatus = useQuery({ queryKey: ["mailServerStatus"], queryFn: getMailServerStatus, refetchInterval: 10000 });
  const resendMut = useMutation({ mutationFn: resendEmail, onSuccess: () => toast({ title: "Email resent" }) });

  const dnsData = dns.data ?? {};
  const emailList = Array.isArray(emails.data) ? emails.data : emails.data?.emails ?? [];
  const srvStatus = serverStatus.data ?? {};

  return (
    <div className="space-y-6 animate-slide-in">
      <div>
        <h1 className="text-2xl font-bold text-foreground">Mail Server</h1>
        <p className="text-sm text-muted-foreground mt-1">Maddy mail server status & management</p>
      </div>

      {/* 2x2 Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card className="border-border/50">
          <CardContent className="p-5">
            <p className="text-xs text-muted-foreground uppercase tracking-wider">Queue Status</p>
            <p className="text-3xl font-bold text-foreground mt-1">{queue.data?.pending ?? stats.data?.queue_count ?? "—"}</p>
            <div className="flex gap-3 mt-1 text-xs text-muted-foreground">
              <span>Retry: {queue.data?.retry ?? 0}</span>
              <span>Dead: {queue.data?.dead_letter ?? 0}</span>
            </div>
          </CardContent>
        </Card>
        <Card className="border-border/50">
          <CardContent className="p-5">
            <p className="text-xs text-muted-foreground uppercase tracking-wider">Today's Stats</p>
            <p className="text-3xl font-bold text-foreground mt-1">{stats.data?.sent_today ?? "—"}</p>
            <div className="flex gap-3 mt-1 text-xs text-muted-foreground">
              <span>{stats.data?.received_today ?? 0} received</span>
              {stats.data?.delivery_rate != null && <span>{stats.data.delivery_rate}% delivery</span>}
              {stats.data?.bounce_rate != null && <span>{stats.data.bounce_rate}% bounce</span>}
            </div>
          </CardContent>
        </Card>

        {/* DNS Health */}
        <Card className="border-border/50">
          <CardContent className="p-5">
            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-2">DNS Health</p>
            <div className="space-y-1.5">
              {["spf", "dkim", "dmarc", "mx", "reverse_dns"].map((rec) => (
                <div key={rec} className="flex items-center gap-2">
                  {dnsData[rec] === true ? <CheckCircle className="h-3.5 w-3.5 text-success" /> : dnsData[rec] === false ? <XCircle className="h-3.5 w-3.5 text-destructive" /> : <AlertTriangle className="h-3.5 w-3.5 text-warning" />}
                  <span className="text-xs uppercase text-foreground">{rec.replace("_", " ")}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Server Status */}
        <Card className="border-border/50">
          <CardContent className="p-5">
            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-2">Server Status</p>
            <div className="space-y-1.5">
              {[
                { label: "Maddy Service", key: "maddy", port: null },
                { label: "SMTP Port 25", key: "smtp_25", port: 25 },
                { label: "Submission 587", key: "submission_587", port: 587 },
                { label: "IMAP 143", key: "imap_143", port: 143 },
              ].map((item) => (
                <div key={item.label} className="flex items-center gap-2">
                  <div className={`h-2 w-2 rounded-full ${srvStatus[item.key] !== false ? "bg-success" : "bg-destructive"}`} />
                  <span className="text-xs text-foreground">{item.label}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Recent Emails */}
      <Card className="border-border/50">
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">Recent Emails</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow className="border-border/50 hover:bg-transparent">
                <TableHead className="text-xs">From</TableHead>
                <TableHead className="text-xs">To</TableHead>
                <TableHead className="text-xs">Subject</TableHead>
                <TableHead className="text-xs">Status</TableHead>
                <TableHead className="text-xs">Time</TableHead>
                <TableHead className="text-xs text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {emailList.length > 0 ? emailList.map((e: any, i: number) => (
                <TableRow key={e.id ?? i} className="border-border/30">
                  <TableCell className="text-sm font-mono">{e.from}</TableCell>
                  <TableCell className="text-sm font-mono">{e.to}</TableCell>
                  <TableCell className="text-sm max-w-[200px] truncate">{e.subject}</TableCell>
                  <TableCell>
                    <Badge variant="outline" className={`text-xs ${e.status === "delivered" || e.status === "sent" ? "text-success border-success/20" : e.status === "bounced" || e.status === "failed" ? "text-destructive border-destructive/20" : "text-warning border-warning/20"}`}>
                      {e.status}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">{e.time}</TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => setSelectedEmailId(e.id)}><Eye className="h-3 w-3" /></Button>
                      {(e.status === "bounced" || e.status === "failed") && (
                        <Button size="sm" variant="ghost" className="h-6 w-6 p-0 text-warning" onClick={() => resendMut.mutate(e.id)} disabled={resendMut.isPending}><RefreshCw className="h-3 w-3" /></Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              )) : (
                <TableRow><TableCell colSpan={6} className="text-center py-8 text-sm text-muted-foreground">{emails.isLoading ? "Loading..." : "No recent emails"}</TableCell></TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <EmailDetailDialog emailId={selectedEmailId} open={!!selectedEmailId} onClose={() => setSelectedEmailId(null)} />
    </div>
  );
}
