import { PageHeader } from "@/components/shell/PageHeader";
import { useQuery } from "@tanstack/react-query";
import { supabase } from "@/integrations/supabase/client";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shell/StatusBadge";
import { Search } from "lucide-react";
import { useState } from "react";

export default function ServicesPage() {
  const [search, setSearch] = useState("");

  const { data: services, isLoading } = useQuery({
    queryKey: ["services-page"],
    queryFn: async () => {
      const { data, error } = await supabase.from("services").select("*, nodes(hostname)").order("updated_at", { ascending: false });
      if (error) throw error;
      return data;
    },
  });

  const filtered = services?.filter((s) =>
    s.name.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="Services" description="D-Bus services across all nodes">
        <div className="relative w-56">
          <Search className="absolute left-2 top-2 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Filter services..." className="pl-7 h-7 text-xs" />
        </div>
      </PageHeader>
      <div className="flex-1 overflow-auto">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-card border-b">
            <tr className="text-left">
              <th className="px-3 py-2 font-medium text-muted-foreground">Name</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">Bus</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">PID</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">Unique</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">Node</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">Activatable</th>
              <th className="px-3 py-2 font-medium text-muted-foreground">Updated</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border/50">
            {isLoading ? (
              <tr><td colSpan={7} className="px-3 py-4 text-center text-muted-foreground">Loading...</td></tr>
            ) : filtered?.length === 0 ? (
              <tr><td colSpan={7} className="px-3 py-4 text-center text-muted-foreground">No services</td></tr>
            ) : (
              filtered?.map((svc) => (
                <tr key={svc.id} className="hover:bg-accent/30">
                  <td className="px-3 py-1.5 font-mono font-medium">{svc.name}</td>
                  <td className="px-3 py-1.5"><Badge variant="outline" className="text-[9px]">{svc.bus}</Badge></td>
                  <td className="px-3 py-1.5 font-mono text-muted-foreground">{svc.pid ?? "—"}</td>
                  <td className="px-3 py-1.5 font-mono text-muted-foreground">{svc.unique_name ?? "—"}</td>
                  <td className="px-3 py-1.5">{(svc as any).nodes?.hostname ?? "—"}</td>
                  <td className="px-3 py-1.5">{svc.is_activatable ? <StatusBadge status="active" label="yes" /> : "—"}</td>
                  <td className="px-3 py-1.5 text-muted-foreground">{new Date(svc.updated_at).toLocaleTimeString()}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
