import { useEffect, useRef } from "react";
import { grpcServerStream, resolveIdentity } from "./grpc-transport";
import {
  CHANGE_TYPE_SCHEMA_MIGRATION,
  decodeStateChange,
} from "./decode";
import { applyPluginSchema, applyPluginValue, noteSchemaHash, uiStore } from "@/store/ui-store";

/**
 * Connects to StateSync.Subscribe and mirrors every plugin object into the
 * json-render state model (/plugins, /pluginIndex, /schemas).
 */
export function useEventStream() {
  const abortRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function connect() {
      await resolveIdentity();
      if (cancelled) return;

      // SubscribeRequest { include_initial_state: true, include_schema: true }
      // field 4 varint 1 → 0x20 0x01
      // field 5 varint 1 → 0x28 0x01  (SCHEMA_MIGRATION contract frames)
      const subscribePayload = new Uint8Array([0x20, 0x01, 0x28, 0x01]);

      const { stream, abort } = grpcServerStream(
        "operation.v1.StateSync",
        "Subscribe",
        subscribePayload,
      );
      abortRef.current = abort;
      uiStore.set("/connected", true);

      try {
        const reader = stream.getReader();
        while (true) {
          const { done, value } = await reader.read();
          if (done || cancelled) break;

          const change = decodeStateChange(value);
          if (!change) continue;

          if (change.catalogHash) {
            uiStore.set("/catalogHash", change.catalogHash);
          }

          if (change.changeType === CHANGE_TYPE_SCHEMA_MIGRATION) {
            if (change.newValue !== undefined) {
              applyPluginSchema(
                change.pluginId,
                change.schemaHash || change.memberName || "",
                change.newValue,
              );
            }
            continue;
          }

          if (change.schemaHash) {
            noteSchemaHash(change.pluginId, change.schemaHash);
          }

          if (change.newValue !== undefined) {
            const key = `${change.pluginId}${change.memberName ? '.' + change.memberName : ''}`;
            console.log(`[StateSync-value] ${key}`, change.newValue);
            applyPluginValue(change.pluginId, change.memberName, change.newValue);
          }

          if (change.schemaHash) {
            console.log(`[StateSync-schema] ${change.pluginId} hash=${change.schemaHash}`);
          }
        }
      } catch (err) {
        if (!cancelled) {
          console.warn("[event-stream] disconnected:", err);
        }
      }

      uiStore.set("/connected", false);
      abortRef.current = null;

      if (!cancelled) {
        setTimeout(connect, 3000);
      }
    }

    connect();

    return () => {
      cancelled = true;
      abortRef.current?.();
    };
  }, []);
}
