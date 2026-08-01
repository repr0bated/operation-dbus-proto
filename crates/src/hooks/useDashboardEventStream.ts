import { useEffect, useMemo, useState } from "react";
import { API_BASE } from "@/api/types";
import {
  createInitialDashboardStreamState,
  parseDashboardStreamEvent,
  reduceDashboardStreamEvent,
  type DashboardStreamState,
} from "@/lib/dashboard-stream";

const STREAM_EVENT_TYPES = ["state_update", "audit_event", "system_stats"] as const;

export function useDashboardEventStream() {
  const [state, setState] = useState<DashboardStreamState>(() =>
    createInitialDashboardStreamState(),
  );

  useEffect(() => {
    const source = new EventSource(`${API_BASE}/events`);

    const applyConnectionState = (connected: boolean) => {
      setState((current) => ({ ...current, connected }));
    };

    const handleTypedEvent = (eventType: string, event: MessageEvent<string>) => {
      setState((current) =>
        reduceDashboardStreamEvent(
          current,
          parseDashboardStreamEvent(eventType, event.data),
        ),
      );
    };

    STREAM_EVENT_TYPES.forEach((eventType) => {
      source.addEventListener(eventType, (event) =>
        handleTypedEvent(eventType, event as MessageEvent<string>),
      );
    });

    source.onopen = () => applyConnectionState(true);
    source.onmessage = (event) => handleTypedEvent("message", event);
    source.onerror = () => applyConnectionState(false);

    return () => {
      source.close();
    };
  }, []);

  return useMemo(() => state, [state]);
}
