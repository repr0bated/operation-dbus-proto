import { useEffect, useRef } from "react";
import { grpcServerStream, resolveIdentity } from "./grpc-transport";
import { useStateStore } from "@/store/state-store";
import { decodeStateChange } from "./decode";

/**
 * Connects to the StateSync.Subscribe gRPC server stream and pushes
 * frames into the zustand state store. Reconnects on disconnect.
 */
export function useEventStream() {
  const setConnected = useStateStore((s) => s.setConnected);
  const applyUpdate = useStateStore((s) => s.applyUpdate);
  const abortRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function connect() {
      // Resolve identity before opening the stream
      await resolveIdentity();

      if (cancelled) return;

      // Subscribe request: empty = all plugins, include initial state
      // Protobuf encoding of SubscribeRequest { include_initial_state: true }
      // field 4, varint 1 → [0x20, 0x01]
      const subscribePayload = new Uint8Array([0x20, 0x01]);

      const { stream, abort } = grpcServerStream(
        "operation.v1.StateSync",
        "Subscribe",
        subscribePayload,
      );
      abortRef.current = abort;
      setConnected(true);

      try {
        const reader = stream.getReader();
        while (true) {
          const { done, value } = await reader.read();
          if (done || cancelled) break;

          const change = decodeStateChange(value);
          if (change && change.pluginId && change.newValue !== undefined) {
            applyUpdate(change.pluginId, change.memberName, change.newValue);
          }
        }
      } catch (err) {
        if (!cancelled) {
          console.warn("[event-stream] disconnected:", err);
        }
      }

      setConnected(false);
      abortRef.current = null;

      // Reconnect after 3s
      if (!cancelled) {
        setTimeout(connect, 3000);
      }
    }

    connect();

    return () => {
      cancelled = true;
      abortRef.current?.();
    };
  }, [setConnected, applyUpdate]);
}
