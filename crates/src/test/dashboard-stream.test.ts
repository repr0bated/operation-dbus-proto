import { describe, expect, it } from "vitest";
import {
  createInitialDashboardStreamState,
  parseDashboardStreamEvent,
  reduceDashboardStreamEvent,
} from "@/lib/dashboard-stream";

describe("dashboard stream parsing", () => {
  it("parses named state update events", () => {
    const event = parseDashboardStreamEvent(
      "state_update",
      JSON.stringify({
        plugin_id: "network",
        object_path: "/network/bridge0",
        property_name: "state",
        new_value: "up",
      }),
    );

    expect(event.type).toBe("state_update");
    expect("plugin_id" in event.payload && event.payload.plugin_id).toBe("network");
  });

  it("falls back to unknown payload for invalid json", () => {
    const event = parseDashboardStreamEvent("audit_event", "{bad json");

    expect(event.type).toBe("unknown");
    expect("raw" in event.payload && event.payload.raw).toBe("{bad json");
  });
});

describe("dashboard stream reduction", () => {
  it("reduces events into counters and keyed state", () => {
    const initial = createInitialDashboardStreamState();
    const stateUpdate = parseDashboardStreamEvent(
      "state_update",
      JSON.stringify({
        plugin_id: "privacy_router",
        object_path: "/plugins/privacy_router",
        property_name: "status",
        new_value: { mode: "active" },
      }),
    );
    const auditEvent = parseDashboardStreamEvent(
      "audit_event",
      JSON.stringify({
        event_id: "evt-1",
        plugin_id: "privacy_router",
        operation: "apply",
        target: "/plugins/privacy_router",
        decision: "allowed",
      }),
    );

    const next = reduceDashboardStreamEvent(
      reduceDashboardStreamEvent(initial, stateUpdate),
      auditEvent,
    );

    expect(next.counters.state_update).toBe(1);
    expect(next.counters.audit_event).toBe(1);
    expect(
      next.latestStateByKey[
        "privacy_router:/plugins/privacy_router:status"
      ]?.new_value,
    ).toEqual({ mode: "active" });
    expect(next.lastAuditEvent?.decision).toBe("allowed");
    expect(next.events).toHaveLength(2);
  });
});
