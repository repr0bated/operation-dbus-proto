import { PageHeader } from "@/components/shell/PageHeader";
import { EventTape } from "@/components/json/EventTape";
import { StateProjectionPanel } from "@/components/json/StateProjectionPanel";
import { useEventStream } from "@/api/eventStream";
import { Input } from "@/components/ui/input";
import { Search } from "lucide-react";
import { useState } from "react";

export default function StatePage() {
  const events = useEventStream((s) => s.events);
  const [search, setSearch] = useState("");

  const stateEvents = events.filter((e) => e.type === "state_update");
  const filtered = search
    ? stateEvents.filter((e) => JSON.stringify(e.data).toLowerCase().includes(search.toLowerCase()))
    : stateEvents;

  return (
    <div className="flex flex-col h-full">
      <PageHeader title="State" description="Live state exploration and projections">
        <div className="relative w-56">
          <Search className="absolute left-2 top-2 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Filter state..." className="pl-7 h-7 text-xs" />
        </div>
      </PageHeader>
      <div className="flex-1 overflow-auto p-4">
        <div className="grid lg:grid-cols-2 gap-4">
          <div className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Current State</h2>
            <StateProjectionPanel />
          </div>
          <div className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">State Changes ({filtered.length})</h2>
            <EventTape events={filtered} maxHeight="calc(100vh - 180px)" />
          </div>
        </div>
      </div>
    </div>
  );
}
