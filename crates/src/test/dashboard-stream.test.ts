import { describe, expect, it } from "vitest";
import {
  createInitialDashboardStreamState,
  parseDashboardStreamEvent,
  reduceDashboardStreamEvent,
  schemaFromFrame,
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

  it("keeps the running contract per plugin from schema frames", () => {
    const schemaFrame = (hash: string, methods: Record<string, unknown>) =>
      parseDashboardStreamEvent(
        "schema_update",
        JSON.stringify({
          change_id: `schema:privacy_router:${hash}`,
          plugin_id: "privacy_router",
          object_path: "/org/opdbus/v1/plugins/privacy_router",
          property_name: null,
          old_value: null,
          new_value: { name: "privacy_router", methods },
          event_id: "0",
          event_hash: "",
          tags_touched: [],
          actor_id: "schema_hydration",
          timestamp: "2026-08-15T00:00:00+00:00",
          change_type: "schema_migration",
          frame_kind: "initial_state",
          schema_hash: hash,
          catalog_hash: `cat-${hash}`,
        }),
      );

    const next = reduceDashboardStreamEvent(
      reduceDashboardStreamEvent(
        createInitialDashboardStreamState(),
        schemaFrame("aaaa1111", { Apply: {} }),
      ),
      schemaFrame("bbbb2222", { Apply: {}, Revoke: {} }),
    );

    // A reseal replaces the contract rather than accumulating versions.
    const contract = next.schemasByPlugin.privacy_router;
    expect(contract?.schema_hash).toBe("bbbb2222");
    expect(schemaFromFrame(contract!)).toEqual({
      name: "privacy_router",
      methods: { Apply: {}, Revoke: {} },
    });
    expect(next.counters.schema_update).toBe(2);
    expect(Object.keys(next.schemasByPlugin)).toHaveLength(1);
  });

  it("flags a plugin whose frames cite a contract we never received", () => {
    const stateFrame = (schemaHash: string) =>
      parseDashboardStreamEvent(
        "state_update",
        JSON.stringify({
          change_id: "c1",
          plugin_id: "network",
          object_path: "/org/opdbus/v1/plugins/network",
          property_name: "bridges",
          old_value: null,
          new_value: { ovsbr0: "up" },
          event_id: "7",
          event_hash: "abc",
          tags_touched: [],
          actor_id: "grpc",
          timestamp: "2026-08-15T00:00:00+00:00",
          change_type: "property_set",
          frame_kind: "update",
          schema_hash: schemaHash,
          catalog_hash: "cat-2",
        }),
      );

    const withContract = reduceDashboardStreamEvent(
      createInitialDashboardStreamState(),
      parseDashboardStreamEvent(
        "schema_update",
        JSON.stringify({
          change_id: "schema:network:held",
          plugin_id: "network",
          object_path: "/org/opdbus/v1/plugins/network",
          property_name: null,
          old_value: null,
          new_value: { name: "network" },
          event_id: "0",
          event_hash: "",
          tags_touched: [],
          actor_id: "schema_hydration",
          timestamp: "2026-08-15T00:00:00+00:00",
          change_type: "schema_migration",
          frame_kind: "initial_state",
          schema_hash: "held",
          catalog_hash: "cat-1",
        }),
      ),
    );

    // A value frame citing a contract we do not hold means a schema frame was
    // dropped — the stream never resends, so this is the only trace of it.
    const drifted = reduceDashboardStreamEvent(withContract, stateFrame("moved"));
    expect(drifted.staleSchemas).toEqual(["network"]);
    expect(drifted.catalogHash).toBe("cat-2");

    // A frame citing the contract we hold is not drift.
    const agreeing = reduceDashboardStreamEvent(withContract, stateFrame("held"));
    expect(agreeing.staleSchemas).toEqual([]);
  });

  it("counts keepalives without inventing a plugin entry", () => {
    const heartbeat = parseDashboardStreamEvent(
      "state_update",
      JSON.stringify({
        change_id: "hb-1",
        plugin_id: "",
        object_path: "",
        property_name: null,
        old_value: null,
        new_value: null,
        event_id: "0",
        event_hash: "",
        tags_touched: [],
        actor_id: "state_sync_keepalive",
        timestamp: "2026-08-15T00:00:00+00:00",
        change_type: "signal",
        frame_kind: "heartbeat",
        schema_hash: null,
        catalog_hash: "cat-idle",
      }),
    );

    const next = reduceDashboardStreamEvent(
      createInitialDashboardStreamState(),
      heartbeat,
    );

    expect(next.counters.state_update).toBe(1);
    expect(Object.keys(next.latestStateByKey)).toHaveLength(0);
    // A keepalive still advances the catalog identity — that is what lets an
    // idle subscriber notice contracts moved without a single value arriving.
    expect(next.catalogHash).toBe("cat-idle");
  });
});
