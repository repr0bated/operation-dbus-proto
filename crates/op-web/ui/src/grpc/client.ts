/**
 * gRPC-Web Transport & Service Clients
 *
 * This module provides the gRPC-Web transport layer and typed service
 * client wrappers for all operation.v1 and operation.registry.v1 services.
 *
 * Uses @protobuf-ts/grpcweb-transport for the wire protocol over
 * tonic-web + Nginx on the backend.
 */

import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport";
import type { RpcOptions, ServerStreamingCall, UnaryCall } from "@protobuf-ts/runtime-rpc";

// ── Transport Configuration ─────────────────────────────────────────────────

const GRPC_BASE_URL =
  import.meta.env.VITE_GRPC_BASE_URL || "https://dashboard.3tched.com";

let _transport: GrpcWebFetchTransport | null = null;

export function getTransport(): GrpcWebFetchTransport {
  if (!_transport) {
    _transport = new GrpcWebFetchTransport({
      baseUrl: GRPC_BASE_URL,
      format: "binary",
    });
  }
  return _transport;
}

/**
 * Reconfigure the transport (e.g. for testing or endpoint changes).
 */
export function resetTransport(baseUrl?: string): void {
  _transport = new GrpcWebFetchTransport({
    baseUrl: baseUrl ?? GRPC_BASE_URL,
    format: "binary",
  });
}

// ── Generic gRPC-Web call helpers ───────────────────────────────────────────

/**
 * Make a unary gRPC-Web call using fetch.
 * Since we don't have generated @protobuf-ts service clients,
 * we use raw JSON-encoded gRPC-Web (tonic-web supports JSON encoding).
 */
async function grpcUnary<TReq, TResp>(
  service: string,
  method: string,
  request: TReq,
): Promise<TResp> {
  const url = `${GRPC_BASE_URL}/${service}/${method}`;
  const payload = JSON.stringify(request);
  // Encode JSON payload into a gRPC-web binary frame:
  // 1 byte flags (0x00 = uncompressed) + 4 bytes big-endian length + payload
  const encoder = new TextEncoder();
  const payloadBytes = encoder.encode(payload);
  const frame = new Uint8Array(5 + payloadBytes.length);
  frame[0] = 0x00; // no compression
  const dv = new DataView(frame.buffer);
  dv.setUint32(1, payloadBytes.length, false);
  frame.set(payloadBytes, 5);

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/grpc-web",
      "Accept": "application/grpc-web",
      "x-grpc-web": "1",
    },
    body: frame,
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`gRPC ${service}/${method} failed: ${res.status} ${text}`);
  }

  const buf = new Uint8Array(await res.arrayBuffer());
  // Parse gRPC-web framed response: skip 5-byte header, read JSON body
  // Tonic-web with JSON codec returns JSON inside the binary frame
  const frames = parseGrpcWebFrames(buf);
  if (frames.length === 0) {
    throw new Error(`gRPC ${service}/${method}: empty response`);
  }
  const decoder = new TextDecoder();
  const body = decoder.decode(frames[0]);
  return JSON.parse(body) as TResp;
}

/** Parse gRPC-web binary frames from a response buffer */
function parseGrpcWebFrames(buf: Uint8Array): Uint8Array[] {
  const frames: Uint8Array[] = [];
  let offset = 0;
  while (offset < buf.length) {
    if (offset + 5 > buf.length) break;
    const flags = buf[offset];
    const dv = new DataView(buf.buffer, buf.byteOffset + offset + 1, 4);
    const len = dv.getUint32(0, false);
    offset += 5;
    if (offset + len > buf.length) break;
    // flags & 0x80 = trailers frame, skip those
    if (!(flags & 0x80)) {
      frames.push(buf.slice(offset, offset + len));
    }
    offset += len;
  }
  return frames;
}

/**
 * Open a server-streaming gRPC-Web call.
 * Returns an async iterable of response messages.
 * Uses fetch + ReadableStream for streaming.
 */
function grpcServerStream<TReq, TResp>(
  service: string,
  method: string,
  request: TReq,
): { stream: ReadableStream<TResp>; abort: () => void } {
  const controller = new AbortController();
  const url = `${GRPC_BASE_URL}/${service}/${method}`;

  const payload = JSON.stringify(request);
  const encoder = new TextEncoder();
  const payloadBytes = encoder.encode(payload);
  const frame = new Uint8Array(5 + payloadBytes.length);
  frame[0] = 0x00;
  const frameDv = new DataView(frame.buffer);
  frameDv.setUint32(1, payloadBytes.length, false);
  frame.set(payloadBytes, 5);

  const responsePromise = fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/grpc-web",
      "Accept": "application/grpc-web",
      "x-grpc-web": "1",
    },
    body: frame,
    signal: controller.signal,
  });

  const stream = new ReadableStream<TResp>({
    async start(ctrl) {
      try {
        const res = await responsePromise;
        if (!res.ok || !res.body) {
          ctrl.error(new Error(`gRPC stream ${service}/${method} failed: ${res.status}`));
          return;
        }

        const reader = res.body.getReader();
        let pending = new Uint8Array(0);

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          // Append new data to pending buffer
          const merged = new Uint8Array(pending.length + value.length);
          merged.set(pending);
          merged.set(value, pending.length);
          pending = merged;

          // Parse complete gRPC-web frames from buffer
          while (pending.length >= 5) {
            const flags = pending[0];
            const dv = new DataView(pending.buffer, pending.byteOffset + 1, 4);
            const len = dv.getUint32(0, false);
            if (pending.length < 5 + len) break; // incomplete frame

            if (!(flags & 0x80)) {
              // Data frame — decode JSON
              const decoder = new TextDecoder();
              const body = decoder.decode(pending.slice(5, 5 + len));
              try {
                const msg = JSON.parse(body) as TResp;
                ctrl.enqueue(msg);
              } catch {
                // non-JSON data frame
              }
            }
            pending = pending.slice(5 + len);
          }
        }

        ctrl.close();
      } catch (err) {
        if ((err as Error).name !== "AbortError") {
          ctrl.error(err);
        }
      }
    },
  });

  return { stream, abort: () => controller.abort() };
}

// ── Service Client Exports ──────────────────────────────────────────────────

import type {
  SubscribeRequest,
  StateChange,
  MutateRequest,
  MutateResponse,
  GetStateRequest,
  GetStateResponse,
  BatchMutateRequest,
  BatchMutateResponse,
  PluginInfo,
  ListPluginsResponse,
  GetSchemaRequest,
  GetSchemaResponse,
  CallMethodRequest,
  CallMethodResponse,
  Signal,
  ChainEvent,
  GetEventsRequest,
  GetEventsResponse,
  SubscribeEventsRequest,
  VerifyChainRequest,
  VerifyChainResponse,
  OvsdbListDbsResponse,
  OvsdbGetSchemaResponse,
  OvsdbTransactRequest,
  OvsdbTransactResponse,
  OvsdbMonitorRequest,
  OvsdbUpdate,
  OvsdbGetBridgeStateResponse,
  RuntimeSystemInfo,
  RuntimeServiceInfo,
  RuntimeListServicesResponse,
  RuntimeStreamMetricsRequest,
  RuntimeMetricUpdate,
  RuntimeListInterfacesResponse,
  RuntimeNumaTopologyResponse,
} from "./types/operation";

import type {
  DiscoverRequest,
  DiscoverResponse,
  WatchRequest,
  RegistryEvent,
  ComponentInfo,
} from "./types/registry";

// ── StateSync Service ───────────────────────────────────────────────────────

export const stateSync = {
  subscribe(req: Partial<SubscribeRequest> = {}) {
    const full: SubscribeRequest = {
      pluginIds: req.pluginIds ?? [],
      pathPatterns: req.pathPatterns ?? [],
      tags: req.tags ?? [],
      includeInitialState: req.includeInitialState ?? true,
    };
    return grpcServerStream<SubscribeRequest, StateChange>(
      "operation.v1.StateSync",
      "Subscribe",
      full,
    );
  },

  mutate(req: MutateRequest) {
    return grpcUnary<MutateRequest, MutateResponse>(
      "operation.v1.StateSync",
      "Mutate",
      req,
    );
  },

  getState(req: GetStateRequest) {
    return grpcUnary<GetStateRequest, GetStateResponse>(
      "operation.v1.StateSync",
      "GetState",
      req,
    );
  },

  batchMutate(req: BatchMutateRequest) {
    return grpcUnary<BatchMutateRequest, BatchMutateResponse>(
      "operation.v1.StateSync",
      "BatchMutate",
      req,
    );
  },
};

// ── PluginService ───────────────────────────────────────────────────────────

export const pluginService = {
  listPlugins() {
    return grpcUnary<Record<string, never>, ListPluginsResponse>(
      "operation.v1.PluginService",
      "ListPlugins",
      {},
    );
  },

  getSchema(req: GetSchemaRequest) {
    return grpcUnary<GetSchemaRequest, GetSchemaResponse>(
      "operation.v1.PluginService",
      "GetSchema",
      req,
    );
  },

  callMethod(req: CallMethodRequest) {
    return grpcUnary<CallMethodRequest, CallMethodResponse>(
      "operation.v1.PluginService",
      "CallMethod",
      req,
    );
  },

  subscribeSignals(req: { pluginId: string; signalNames: string[]; objectPath: string }) {
    return grpcServerStream<typeof req, Signal>(
      "operation.v1.PluginService",
      "SubscribeSignals",
      req,
    );
  },
};

// ── EventChainService ───────────────────────────────────────────────────────

export const eventChainService = {
  getEvents(req: Partial<GetEventsRequest> = {}) {
    return grpcUnary<GetEventsRequest, GetEventsResponse>(
      "operation.v1.EventChainService",
      "GetEvents",
      {
        fromEventId: req.fromEventId ?? 0,
        toEventId: req.toEventId ?? 0,
        limit: req.limit ?? 100,
        pluginId: req.pluginId ?? "",
        tags: req.tags ?? [],
        decisionFilter: req.decisionFilter ?? 0,
      },
    );
  },

  subscribeEvents(req: Partial<SubscribeEventsRequest> = {}) {
    return grpcServerStream<SubscribeEventsRequest, ChainEvent>(
      "operation.v1.EventChainService",
      "SubscribeEvents",
      {
        fromEventId: req.fromEventId ?? 0,
        pluginId: req.pluginId ?? "",
        tags: req.tags ?? [],
      },
    );
  },

  verifyChain(req: VerifyChainRequest) {
    return grpcUnary<VerifyChainRequest, VerifyChainResponse>(
      "operation.v1.EventChainService",
      "VerifyChain",
      req,
    );
  },
};

// ── OvsdbMirror Service ─────────────────────────────────────────────────────

export const ovsdbMirror = {
  listDbs() {
    return grpcUnary<Record<string, never>, OvsdbListDbsResponse>(
      "operation.v1.OvsdbMirror",
      "ListDbs",
      {},
    );
  },

  getSchema(db: string) {
    return grpcUnary<{ database: string }, OvsdbGetSchemaResponse>(
      "operation.v1.OvsdbMirror",
      "GetSchema",
      { database: db },
    );
  },

  transact(req: OvsdbTransactRequest) {
    return grpcUnary<OvsdbTransactRequest, OvsdbTransactResponse>(
      "operation.v1.OvsdbMirror",
      "Transact",
      req,
    );
  },

  monitor(req: OvsdbMonitorRequest) {
    return grpcServerStream<OvsdbMonitorRequest, OvsdbUpdate>(
      "operation.v1.OvsdbMirror",
      "Monitor",
      req,
    );
  },

  getBridgeState(bridgeName = "") {
    return grpcUnary<{ bridgeName: string }, OvsdbGetBridgeStateResponse>(
      "operation.v1.OvsdbMirror",
      "GetBridgeState",
      { bridgeName },
    );
  },
};

// ── RuntimeMirror Service ───────────────────────────────────────────────────

export const runtimeMirror = {
  getSystemInfo() {
    return grpcUnary<Record<string, never>, RuntimeSystemInfo>(
      "operation.v1.RuntimeMirror",
      "GetSystemInfo",
      {},
    );
  },

  listServices(stateFilter = "") {
    return grpcUnary<{ stateFilter: string }, RuntimeListServicesResponse>(
      "operation.v1.RuntimeMirror",
      "ListServices",
      { stateFilter },
    );
  },

  getService(serviceName: string) {
    return grpcUnary<{ serviceName: string }, RuntimeServiceInfo>(
      "operation.v1.RuntimeMirror",
      "GetService",
      { serviceName },
    );
  },

  streamMetrics(req: Partial<RuntimeStreamMetricsRequest> = {}) {
    return grpcServerStream<RuntimeStreamMetricsRequest, RuntimeMetricUpdate>(
      "operation.v1.RuntimeMirror",
      "StreamMetrics",
      {
        intervalSeconds: req.intervalSeconds ?? 5,
        categories: req.categories ?? [],
      },
    );
  },

  listInterfaces() {
    return grpcUnary<Record<string, never>, RuntimeListInterfacesResponse>(
      "operation.v1.RuntimeMirror",
      "ListInterfaces",
      {},
    );
  },

  getNumaTopology() {
    return grpcUnary<Record<string, never>, RuntimeNumaTopologyResponse>(
      "operation.v1.RuntimeMirror",
      "GetNumaTopology",
      {},
    );
  },
};

// ── ComponentRegistry Service ───────────────────────────────────────────────

export const componentRegistry = {
  discover(req: Partial<DiscoverRequest> = {}) {
    return grpcUnary<DiscoverRequest, DiscoverResponse>(
      "operation.registry.v1.ComponentRegistry",
      "Discover",
      {
        componentType: req.componentType ?? "",
        capability: req.capability ?? "",
        metadataKey: req.metadataKey ?? "",
        metadataValue: req.metadataValue ?? "",
        includeStale: req.includeStale ?? false,
      },
    );
  },

  watch(req: Partial<WatchRequest> = {}) {
    return grpcServerStream<WatchRequest, RegistryEvent>(
      "operation.registry.v1.ComponentRegistry",
      "Watch",
      {
        componentTypes: req.componentTypes ?? [],
        includeExisting: req.includeExisting ?? true,
      },
    );
  },

  getComponent(componentId: string) {
    return grpcUnary<{ componentId: string }, { component: ComponentInfo; found: boolean }>(
      "operation.registry.v1.ComponentRegistry",
      "GetComponent",
      { componentId },
    );
  },
};
