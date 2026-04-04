import { PageHeader } from "@/components/shell/PageHeader";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shell/StatusBadge";
import { JsonTree } from "@/components/json/JsonTree";
import { Shield } from "lucide-react";

export default function SecurityPage() {
  return (
    <div className="flex flex-col h-full">
      <PageHeader title="Security" description="Authentication, policies, and audit trail" />
      <div className="flex-1 overflow-auto p-4 space-y-4">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <div className="rounded border bg-card p-3 space-y-1">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">TLS</div>
            <StatusBadge status="active" label="Enabled" />
          </div>
          <div className="rounded border bg-card p-3 space-y-1">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Auth Mode</div>
            <div className="font-mono text-sm">OIDC</div>
          </div>
          <div className="rounded border bg-card p-3 space-y-1">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Sessions</div>
            <div className="font-mono text-sm">0</div>
          </div>
          <div className="rounded border bg-card p-3 space-y-1">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">Policies</div>
            <div className="font-mono text-sm">0</div>
          </div>
        </div>

        <div className="space-y-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Recent Audit Events</h2>
          <div className="rounded border bg-card p-4 text-xs text-muted-foreground text-center">
            No audit events. Connect to the backend to view security data.
          </div>
        </div>
      </div>
    </div>
  );
}
