/**
 * gRPC-Web Transport & Service Clients
 *
 * This module provides the gRPC-Web transport layer and typed service
 * client wrappers for all services in the operation-dbus architecture:
 *
 * Bridge services (op-grpc-bridge):
 *   - StateSync, PluginService, EventChainService
 *   - OvsdbMirror, RuntimeMirror, ComponentRegistry
 *
 * Domain services:
 *   - MailService (operation.mail.v1)
 *   - RegistrationService (operation.registration.v1)
 *   - PrivacyNetworkService (operation.privacy.v1)
 *   - ServiceManager (opdbus.services.v1)
 *   - McpService (op.mcp.v1)
 *
 * Uses binary gRPC-Web framing over tonic-web + Nginx.
 * @see docs/architecture-flow.md
 */

import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport";

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

export function resetTransport(baseUrl?: string): void {
  _transport = new GrpcWebFetchTransport({
    baseUrl: baseUrl ?? GRPC_BASE_URL,
    format: "binary",
  });
}

// ── Generic gRPC-Web call helpers ───────────────────────────────────────────

async function grpcUnary<TReq, TResp>(
  service: string,
  method: string,
  request: TReq,
): Promise<TResp> {
  const url = `${GRPC_BASE_URL}/${service}/${method}`;
  const payload = JSON.stringify(request);
  const encoder = new TextEncoder();
  const payloadBytes = encoder.encode(payload);
  const frame = new Uint8Array(5 + payloadBytes.length);
  frame[0] = 0x00;
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
  const frames = parseGrpcWebFrames(buf);
  if (frames.length === 0) {
    throw new Error(`gRPC ${service}/${method}: empty response`);
  }
  const decoder = new TextDecoder();
  const body = decoder.decode(frames[0]);
  return JSON.parse(body) as TResp;
}

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
    if (!(flags & 0x80)) {
      frames.push(buf.slice(offset, offset + len));
    }
    offset += len;
  }
  return frames;
}

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

          const merged = new Uint8Array(pending.length + value.length);
          merged.set(pending);
          merged.set(value, pending.length);
          pending = merged;

          while (pending.length >= 5) {
            const flags = pending[0];
            const dv = new DataView(pending.buffer, pending.byteOffset + 1, 4);
            const len = dv.getUint32(0, false);
            if (pending.length < 5 + len) break;

            if (!(flags & 0x80)) {
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

// ── Type Imports ────────────────────────────────────────────────────────────

import type {
  SubscribeRequest, StateChange, MutateRequest, MutateResponse,
  GetStateRequest, GetStateResponse, BatchMutateRequest, BatchMutateResponse,
} from "./types/state-sync";

import type {
  ListPluginsResponse, GetSchemaRequest, GetSchemaResponse,
  CallMethodRequest, CallMethodResponse,
  GetPropertyRequest, GetPropertyResponse,
  SetPropertyRequest, SetPropertyResponse,
  SubscribeSignalsRequest, Signal,
} from "./types/plugin-service";

import type {
  GetEventsRequest, GetEventsResponse, ChainEvent,
  SubscribeEventsRequest, VerifyChainRequest, VerifyChainResponse,
  GetProofRequest, GetProofResponse,
  ProveTagImmutabilityRequest, ProveTagImmutabilityResponse,
  GetSnapshotRequest, GetSnapshotResponse,
  CreateSnapshotRequest, CreateSnapshotResponse,
} from "./types/event-chain";

import type {
  OvsdbListDbsResponse, OvsdbGetSchemaResponse,
  OvsdbTransactRequest, OvsdbTransactResponse,
  OvsdbMonitorRequest, OvsdbUpdate,
  OvsdbEchoRequest, OvsdbEchoResponse,
  OvsdbDumpDbRequest, OvsdbDumpDbResponse,
  OvsdbGetBridgeStateResponse,
} from "./types/ovsdb-mirror";

import type {
  RuntimeSystemInfo, RuntimeServiceInfo, RuntimeListServicesResponse,
  RuntimeStreamMetricsRequest, RuntimeMetricUpdate,
  RuntimeListInterfacesResponse, RuntimeNumaTopologyResponse,
} from "./types/runtime-mirror";

import type {
  DiscoverRequest, DiscoverResponse, WatchRequest, RegistryEvent,
  ComponentInfo,
} from "./types/registry";

import type {
  SendEmailRequest, SendEmailResponse,
  GetInboxRequest, GetInboxResponse,
  GetMessageRequest, GetMessageResponse,
  GetMailStatusRequest, GetMailStatusResponse,
  ListMailAccountsRequest, ListMailAccountsResponse,
  AdminMailActionRequest, AdminMailActionResponse,
  CheckMailServerRequest, CheckMailServerResponse,
} from "./types/mail";

import type {
  EnsurePrivacyNetworkRequest, EnsurePrivacyNetworkResponse,
  GetNetworkStatusRequest, GetNetworkStatusResponse,
  ProvisionUserRequest, ProvisionUserResponse,
  GetPrivacyWireGuardConfigRequest, GetPrivacyWireGuardConfigResponse,
  ManageComponentRequest, ManageComponentResponse,
  GetNetworkTopologyRequest, GetNetworkTopologyResponse,
  HealthCheckRequest, HealthCheckResponse,
  ConfigurePacketRoutingRequest, ConfigurePacketRoutingResponse,
  GenerateWireGuardKeyPairRequest, GenerateWireGuardKeyPairResponse,
} from "./types/privacy";

import type {
  SendMagicLinkRequest, SendMagicLinkResponse,
  VerifyMagicLinkRequest, VerifyMagicLinkResponse,
  RegisterUserRequest, RegisterUserResponse,
  GetUserStatusRequest, GetUserStatusResponse,
  ListUsersRequest, ListUsersResponse,
  GetWireGuardConfigRequest, GetWireGuardConfigResponse,
  AdminUserActionRequest, AdminUserActionResponse,
} from "./types/registration";

import type {
  ServiceDef, ServiceStatus, ServiceEvent,
  GetServiceResponse, ListServicesResponse as SvcMgrListResponse,
  WatchServicesRequest,
} from "./types/service-manager";

import type {
  McpHealthResponse, InitializeRequest, InitializeResponse,
  ListToolsRequest, ListToolsResponse as McpListToolsResponse,
  CallToolRequest, CallToolResponse, ToolOutput,
  McpSubscribeRequest, McpEvent,
} from "./types/mcp";

import type {
  SearchEpisodesRequest, SearchEpisodesResponse,
  GetEpisodeRequest, GetEpisodeResponse,
  CollectionStatsResponse,
  ChatWithContextRequest, ChatWithContextResponse,
  SubscribeEpisodesRequest, EpisodeEvent,
  GetPiiPolicyResponse,
} from "./types/accountability";

import type {
  GetFootprintsRequest, GetFootprintsResponse,
  VerifyBlockchainRequest, VerifyBlockchainResponse,
  GetEmbeddingQueueStatusResponse,
  GetQdrantRolesResponse,
} from "./types/blockchain";

import type {
  GetSubvolumesResponse,
  GetSnapshotsRequest as BtrfsGetSnapshotsRequest, GetSnapshotsResponse as BtrfsGetSnapshotsResponse,
  GetSendStateResponse,
  GetDrStatusResponse,
} from "./types/btrfs";

import type {
  ListPersonasResponse,
  GetPersonaRequest, GetPersonaResponse,
  CreatePersonaRequest, CreatePersonaResponse,
  UpdatePersonaRequest, UpdatePersonaResponse,
  DeletePersonaRequest, DeletePersonaResponse,
  ListAgentRoutesResponse,
} from "./types/persona";

import type {
  GetDataStoresResponse,
  GetStoreDetailRequest, GetStoreDetailResponse,
} from "./types/data-stores";

import type {
  GetEmbeddingQueueRequest, GetEmbeddingQueueResponse,
  GetEmbeddingWorkerStatusResponse,
  PreviewEmbeddingTextRequest, PreviewEmbeddingTextResponse,
  GetChannelDiagnosticsResponse,
} from "./types/embedding";

// ═══════════════════════════════════════════════════════════════════════════
// BRIDGE SERVICES (op-grpc-bridge)
// ═══════════════════════════════════════════════════════════════════════════

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
      "operation.v1.StateSync", "Subscribe", full,
    );
  },

  mutate(req: MutateRequest) {
    return grpcUnary<MutateRequest, MutateResponse>(
      "operation.v1.StateSync", "Mutate", req,
    );
  },

  getState(req: GetStateRequest) {
    return grpcUnary<GetStateRequest, GetStateResponse>(
      "operation.v1.StateSync", "GetState", req,
    );
  },

  batchMutate(req: BatchMutateRequest) {
    return grpcUnary<BatchMutateRequest, BatchMutateResponse>(
      "operation.v1.StateSync", "BatchMutate", req,
    );
  },
};

// ── PluginService ───────────────────────────────────────────────────────────

export const pluginService = {
  listPlugins() {
    return grpcUnary<Record<string, never>, ListPluginsResponse>(
      "operation.v1.PluginService", "ListPlugins", {},
    );
  },

  getSchema(req: GetSchemaRequest) {
    return grpcUnary<GetSchemaRequest, GetSchemaResponse>(
      "operation.v1.PluginService", "GetSchema", req,
    );
  },

  callMethod(req: CallMethodRequest) {
    return grpcUnary<CallMethodRequest, CallMethodResponse>(
      "operation.v1.PluginService", "CallMethod", req,
    );
  },

  getProperty(req: GetPropertyRequest) {
    return grpcUnary<GetPropertyRequest, GetPropertyResponse>(
      "operation.v1.PluginService", "GetProperty", req,
    );
  },

  setProperty(req: SetPropertyRequest) {
    return grpcUnary<SetPropertyRequest, SetPropertyResponse>(
      "operation.v1.PluginService", "SetProperty", req,
    );
  },

  subscribeSignals(req: SubscribeSignalsRequest) {
    return grpcServerStream<SubscribeSignalsRequest, Signal>(
      "operation.v1.PluginService", "SubscribeSignals", req,
    );
  },
};

// ── EventChainService ───────────────────────────────────────────────────────

export const eventChainService = {
  getEvents(req: Partial<GetEventsRequest> = {}) {
    return grpcUnary<GetEventsRequest, GetEventsResponse>(
      "operation.v1.EventChainService", "GetEvents", {
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
      "operation.v1.EventChainService", "SubscribeEvents", {
        fromEventId: req.fromEventId ?? 0,
        pluginId: req.pluginId ?? "",
        tags: req.tags ?? [],
      },
    );
  },

  verifyChain(req: VerifyChainRequest) {
    return grpcUnary<VerifyChainRequest, VerifyChainResponse>(
      "operation.v1.EventChainService", "VerifyChain", req,
    );
  },

  getProof(req: GetProofRequest) {
    return grpcUnary<GetProofRequest, GetProofResponse>(
      "operation.v1.EventChainService", "GetProof", req,
    );
  },

  proveTagImmutability(req: ProveTagImmutabilityRequest) {
    return grpcUnary<ProveTagImmutabilityRequest, ProveTagImmutabilityResponse>(
      "operation.v1.EventChainService", "ProveTagImmutability", req,
    );
  },

  getSnapshot(req: GetSnapshotRequest) {
    return grpcUnary<GetSnapshotRequest, GetSnapshotResponse>(
      "operation.v1.EventChainService", "GetSnapshot", req,
    );
  },

  createSnapshot(req: CreateSnapshotRequest) {
    return grpcUnary<CreateSnapshotRequest, CreateSnapshotResponse>(
      "operation.v1.EventChainService", "CreateSnapshot", req,
    );
  },
};

// ── OvsdbMirror Service ─────────────────────────────────────────────────────

export const ovsdbMirror = {
  listDbs() {
    return grpcUnary<Record<string, never>, OvsdbListDbsResponse>(
      "operation.v1.OvsdbMirror", "ListDbs", {},
    );
  },

  getSchema(db: string) {
    return grpcUnary<{ database: string }, OvsdbGetSchemaResponse>(
      "operation.v1.OvsdbMirror", "GetSchema", { database: db },
    );
  },

  transact(req: OvsdbTransactRequest) {
    return grpcUnary<OvsdbTransactRequest, OvsdbTransactResponse>(
      "operation.v1.OvsdbMirror", "Transact", req,
    );
  },

  monitor(req: OvsdbMonitorRequest) {
    return grpcServerStream<OvsdbMonitorRequest, OvsdbUpdate>(
      "operation.v1.OvsdbMirror", "Monitor", req,
    );
  },

  echo(req: OvsdbEchoRequest) {
    return grpcUnary<OvsdbEchoRequest, OvsdbEchoResponse>(
      "operation.v1.OvsdbMirror", "Echo", req,
    );
  },

  dumpDb(db: string) {
    return grpcUnary<OvsdbDumpDbRequest, OvsdbDumpDbResponse>(
      "operation.v1.OvsdbMirror", "DumpDb", { database: db },
    );
  },

  getBridgeState(bridgeName = "") {
    return grpcUnary<{ bridgeName: string }, OvsdbGetBridgeStateResponse>(
      "operation.v1.OvsdbMirror", "GetBridgeState", { bridgeName },
    );
  },
};

// ── RuntimeMirror Service ───────────────────────────────────────────────────

export const runtimeMirror = {
  getSystemInfo() {
    return grpcUnary<Record<string, never>, RuntimeSystemInfo>(
      "operation.v1.RuntimeMirror", "GetSystemInfo", {},
    );
  },

  listServices(stateFilter = "") {
    return grpcUnary<{ stateFilter: string }, RuntimeListServicesResponse>(
      "operation.v1.RuntimeMirror", "ListServices", { stateFilter },
    );
  },

  getService(serviceName: string) {
    return grpcUnary<{ serviceName: string }, RuntimeServiceInfo>(
      "operation.v1.RuntimeMirror", "GetService", { serviceName },
    );
  },

  streamMetrics(req: Partial<RuntimeStreamMetricsRequest> = {}) {
    return grpcServerStream<RuntimeStreamMetricsRequest, RuntimeMetricUpdate>(
      "operation.v1.RuntimeMirror", "StreamMetrics", {
        intervalSeconds: req.intervalSeconds ?? 5,
        categories: req.categories ?? [],
      },
    );
  },

  listInterfaces() {
    return grpcUnary<Record<string, never>, RuntimeListInterfacesResponse>(
      "operation.v1.RuntimeMirror", "ListInterfaces", {},
    );
  },

  getNumaTopology() {
    return grpcUnary<Record<string, never>, RuntimeNumaTopologyResponse>(
      "operation.v1.RuntimeMirror", "GetNumaTopology", {},
    );
  },
};

// ── ComponentRegistry Service ───────────────────────────────────────────────

export const componentRegistry = {
  discover(req: Partial<DiscoverRequest> = {}) {
    return grpcUnary<DiscoverRequest, DiscoverResponse>(
      "operation.registry.v1.ComponentRegistry", "Discover", {
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
      "operation.registry.v1.ComponentRegistry", "Watch", {
        componentTypes: req.componentTypes ?? [],
        includeExisting: req.includeExisting ?? true,
      },
    );
  },

  getComponent(componentId: string) {
    return grpcUnary<{ componentId: string }, { component: ComponentInfo; found: boolean }>(
      "operation.registry.v1.ComponentRegistry", "GetComponent", { componentId },
    );
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// DOMAIN SERVICES
// ═══════════════════════════════════════════════════════════════════════════

// ── MailService (operation.mail.v1) ─────────────────────────────────────────

export const mailService = {
  sendEmail(req: SendEmailRequest) {
    return grpcUnary<SendEmailRequest, SendEmailResponse>(
      "operation.mail.v1.MailService", "SendEmail", req,
    );
  },

  getInbox(req: GetInboxRequest) {
    return grpcUnary<GetInboxRequest, GetInboxResponse>(
      "operation.mail.v1.MailService", "GetInbox", req,
    );
  },

  getMessage(req: GetMessageRequest) {
    return grpcUnary<GetMessageRequest, GetMessageResponse>(
      "operation.mail.v1.MailService", "GetMessage", req,
    );
  },

  getMailStatus(req: Partial<GetMailStatusRequest> = {}) {
    return grpcUnary<GetMailStatusRequest, GetMailStatusResponse>(
      "operation.mail.v1.MailService", "GetMailStatus", { domain: req.domain ?? "" },
    );
  },

  listMailAccounts(req: Partial<ListMailAccountsRequest> = {}) {
    return grpcUnary<ListMailAccountsRequest, ListMailAccountsResponse>(
      "operation.mail.v1.MailService", "ListMailAccounts", {
        domain: req.domain ?? "",
        includeInactive: req.includeInactive ?? false,
      },
    );
  },

  adminMailAction(req: AdminMailActionRequest) {
    return grpcUnary<AdminMailActionRequest, AdminMailActionResponse>(
      "operation.mail.v1.MailService", "AdminMailAction", req,
    );
  },

  checkMailServer(req: Partial<CheckMailServerRequest> = {}) {
    return grpcUnary<CheckMailServerRequest, CheckMailServerResponse>(
      "operation.mail.v1.MailService", "CheckMailServer", {
        domain: req.domain ?? "",
        checkSmtp: req.checkSmtp ?? true,
        checkImap: req.checkImap ?? true,
        checkWebmail: req.checkWebmail ?? true,
      },
    );
  },
};

// ── PrivacyNetworkService (operation.privacy.v1) ────────────────────────────

export const privacyService = {
  ensurePrivacyNetwork(req: Partial<EnsurePrivacyNetworkRequest> = {}) {
    return grpcUnary<EnsurePrivacyNetworkRequest, EnsurePrivacyNetworkResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "EnsurePrivacyNetwork", {
        domain: req.domain ?? "",
        forceReprovision: req.forceReprovision ?? false,
      },
    );
  },

  getNetworkStatus(req: Partial<GetNetworkStatusRequest> = {}) {
    return grpcUnary<GetNetworkStatusRequest, GetNetworkStatusResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "GetNetworkStatus", {
        component: req.component ?? "all",
      },
    );
  },

  provisionUser(req: ProvisionUserRequest) {
    return grpcUnary<ProvisionUserRequest, ProvisionUserResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "ProvisionUser", req,
    );
  },

  getPrivacyWireGuardConfig(req: GetPrivacyWireGuardConfigRequest) {
    return grpcUnary<GetPrivacyWireGuardConfigRequest, GetPrivacyWireGuardConfigResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "GetPrivacyWireGuardConfig", req,
    );
  },

  manageComponent(req: ManageComponentRequest) {
    return grpcUnary<ManageComponentRequest, ManageComponentResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "ManageComponent", req,
    );
  },

  getNetworkTopology(req: Partial<GetNetworkTopologyRequest> = {}) {
    return grpcUnary<GetNetworkTopologyRequest, GetNetworkTopologyResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "GetNetworkTopology", {
        includeDetails: req.includeDetails ?? true,
      },
    );
  },

  healthCheck(req: Partial<HealthCheckRequest> = {}) {
    return grpcUnary<HealthCheckRequest, HealthCheckResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "HealthCheck", {
        checkWgcf: req.checkWgcf ?? true,
        checkOvs: req.checkOvs ?? true,
        checkXray: req.checkXray ?? true,
        checkPorts: req.checkPorts ?? true,
      },
    );
  },

  configurePacketRouting(req: ConfigurePacketRoutingRequest) {
    return grpcUnary<ConfigurePacketRoutingRequest, ConfigurePacketRoutingResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "ConfigurePacketRouting", req,
    );
  },

  generateWireGuardKeyPair(req: GenerateWireGuardKeyPairRequest) {
    return grpcUnary<GenerateWireGuardKeyPairRequest, GenerateWireGuardKeyPairResponse>(
      "operation.privacy.v1.PrivacyNetworkService", "GenerateWireGuardKeyPair", req,
    );
  },
};

// ── RegistrationService (operation.registration.v1) ─────────────────────────

export const registrationService = {
  sendMagicLink(req: SendMagicLinkRequest) {
    return grpcUnary<SendMagicLinkRequest, SendMagicLinkResponse>(
      "operation.registration.v1.RegistrationService", "SendMagicLink", req,
    );
  },

  verifyMagicLink(req: VerifyMagicLinkRequest) {
    return grpcUnary<VerifyMagicLinkRequest, VerifyMagicLinkResponse>(
      "operation.registration.v1.RegistrationService", "VerifyMagicLink", req,
    );
  },

  registerUser(req: RegisterUserRequest) {
    return grpcUnary<RegisterUserRequest, RegisterUserResponse>(
      "operation.registration.v1.RegistrationService", "RegisterUser", req,
    );
  },

  getUserStatus(req: GetUserStatusRequest) {
    return grpcUnary<GetUserStatusRequest, GetUserStatusResponse>(
      "operation.registration.v1.RegistrationService", "GetUserStatus", req,
    );
  },

  listUsers(req: Partial<ListUsersRequest> = {}) {
    return grpcUnary<ListUsersRequest, ListUsersResponse>(
      "operation.registration.v1.RegistrationService", "ListUsers", {
        limit: req.limit ?? 100,
        offset: req.offset ?? 0,
        includeAdminsOnly: req.includeAdminsOnly ?? false,
        domainFilter: req.domainFilter ?? "",
      },
    );
  },

  getWireGuardConfig(req: GetWireGuardConfigRequest) {
    return grpcUnary<GetWireGuardConfigRequest, GetWireGuardConfigResponse>(
      "operation.registration.v1.RegistrationService", "GetWireGuardConfig", req,
    );
  },

  adminUserAction(req: AdminUserActionRequest) {
    return grpcUnary<AdminUserActionRequest, AdminUserActionResponse>(
      "operation.registration.v1.RegistrationService", "AdminUserAction", req,
    );
  },
};

// ── ServiceManager (opdbus.services.v1) ─────────────────────────────────────

export const serviceManager = {
  start(name: string) {
    return grpcUnary<{ name: string }, { status: ServiceStatus }>(
      "opdbus.services.v1.ServiceManager", "Start", { name },
    );
  },

  stop(name: string) {
    return grpcUnary<{ name: string }, { status: ServiceStatus }>(
      "opdbus.services.v1.ServiceManager", "Stop", { name },
    );
  },

  restart(name: string) {
    return grpcUnary<{ name: string }, { status: ServiceStatus }>(
      "opdbus.services.v1.ServiceManager", "Restart", { name },
    );
  },

  reload(name: string) {
    return grpcUnary<{ name: string }, { status: ServiceStatus }>(
      "opdbus.services.v1.ServiceManager", "Reload", { name },
    );
  },

  create(service: ServiceDef) {
    return grpcUnary<{ service: ServiceDef }, { service: ServiceDef }>(
      "opdbus.services.v1.ServiceManager", "Create", { service },
    );
  },

  delete(name: string) {
    return grpcUnary<{ name: string }, Record<string, never>>(
      "opdbus.services.v1.ServiceManager", "Delete", { name },
    );
  },

  get(name: string) {
    return grpcUnary<{ name: string }, GetServiceResponse>(
      "opdbus.services.v1.ServiceManager", "Get", { name },
    );
  },

  list(filter = "") {
    return grpcUnary<{ filter: string }, SvcMgrListResponse>(
      "opdbus.services.v1.ServiceManager", "List", { filter },
    );
  },

  enable(name: string) {
    return grpcUnary<{ name: string }, Record<string, never>>(
      "opdbus.services.v1.ServiceManager", "Enable", { name },
    );
  },

  disable(name: string) {
    return grpcUnary<{ name: string }, Record<string, never>>(
      "opdbus.services.v1.ServiceManager", "Disable", { name },
    );
  },

  watchStatus(req: Partial<WatchServicesRequest> = {}) {
    return grpcServerStream<WatchServicesRequest, ServiceEvent>(
      "opdbus.services.v1.ServiceManager", "WatchStatus", {
        names: req.names ?? [],
      },
    );
  },
};

// ── McpService (op.mcp.v1) ──────────────────────────────────────────────────

export const mcpService = {
  health() {
    return grpcUnary<Record<string, never>, McpHealthResponse>(
      "op.mcp.v1.McpService", "Health", {},
    );
  },

  initialize(req: Partial<InitializeRequest> = {}) {
    return grpcUnary<InitializeRequest, InitializeResponse>(
      "op.mcp.v1.McpService", "Initialize", {
        clientName: req.clientName ?? "web-ui",
        capabilities: req.capabilities ?? [],
      },
    );
  },

  listTools(req: Partial<ListToolsRequest> = {}) {
    return grpcUnary<ListToolsRequest, McpListToolsResponse>(
      "op.mcp.v1.McpService", "ListTools", {
        limit: req.limit ?? 100,
        offset: req.offset ?? 0,
      },
    );
  },

  callTool(req: CallToolRequest) {
    return grpcUnary<CallToolRequest, CallToolResponse>(
      "op.mcp.v1.McpService", "CallTool", req,
    );
  },

  callToolStreaming(req: CallToolRequest) {
    return grpcServerStream<CallToolRequest, ToolOutput>(
      "op.mcp.v1.McpService", "CallToolStreaming", req,
    );
  },

  subscribe(req: Partial<McpSubscribeRequest> = {}) {
    return grpcServerStream<McpSubscribeRequest, McpEvent>(
      "op.mcp.v1.McpService", "Subscribe", {
        eventTypes: req.eventTypes ?? [],
      },
    );
  },
};

// ── ChatbotAccountabilityService (operation.accountability.v1) ──────────────

export const accountabilityService = {
  searchEpisodes(req: SearchEpisodesRequest) {
    return grpcUnary<SearchEpisodesRequest, SearchEpisodesResponse>(
      "operation.accountability.v1.ChatbotAccountabilityService", "SearchEpisodes", {
        query: req.query,
        outcomeClass: req.outcomeClass ?? "",
        pluginId: req.pluginId ?? "",
        conversationId: req.conversationId ?? "",
        timeRangeStart: req.timeRangeStart ?? "",
        timeRangeEnd: req.timeRangeEnd ?? "",
        limit: req.limit ?? 20,
      },
    );
  },

  getEpisode(req: GetEpisodeRequest) {
    return grpcUnary<GetEpisodeRequest, GetEpisodeResponse>(
      "operation.accountability.v1.ChatbotAccountabilityService", "GetEpisode", req,
    );
  },

  getCollectionStats() {
    return grpcUnary<Record<string, never>, CollectionStatsResponse>(
      "operation.accountability.v1.ChatbotAccountabilityService", "GetCollectionStats", {},
    );
  },

  chatWithContext(req: ChatWithContextRequest) {
    return grpcUnary<ChatWithContextRequest, ChatWithContextResponse>(
      "operation.accountability.v1.ChatbotAccountabilityService", "ChatWithContext", req,
    );
  },

  subscribeEpisodes(req: Partial<SubscribeEpisodesRequest> = {}) {
    return grpcServerStream<SubscribeEpisodesRequest, EpisodeEvent>(
      "operation.accountability.v1.ChatbotAccountabilityService", "SubscribeEpisodes", {
        pluginId: req.pluginId ?? "",
        outcomeClassFilter: req.outcomeClassFilter ?? "",
        includeExisting: req.includeExisting ?? false,
      },
    );
  },

  getPiiPolicy() {
    return grpcUnary<Record<string, never>, GetPiiPolicyResponse>(
      "operation.accountability.v1.ChatbotAccountabilityService", "GetPiiPolicy", {},
    );
  },
};

// ── BlockchainService (operation.blockchain.v1) ─────────────────────────────

export const blockchainService = {
  getFootprints(req: Partial<GetFootprintsRequest> = {}) {
    return grpcUnary<GetFootprintsRequest, GetFootprintsResponse>(
      "operation.blockchain.v1.BlockchainService", "GetFootprints", {
        fromIndex: req.fromIndex ?? 0,
        toIndex: req.toIndex ?? 0,
        limit: req.limit ?? 100,
        pluginId: req.pluginId ?? "",
      },
    );
  },

  verifyChain(req: Partial<VerifyBlockchainRequest> = {}) {
    return grpcUnary<VerifyBlockchainRequest, VerifyBlockchainResponse>(
      "operation.blockchain.v1.BlockchainService", "VerifyChain", {
        fromIndex: req.fromIndex ?? 0,
        toIndex: req.toIndex ?? 0,
      },
    );
  },

  getEmbeddingQueueStatus() {
    return grpcUnary<Record<string, never>, GetEmbeddingQueueStatusResponse>(
      "operation.blockchain.v1.BlockchainService", "GetEmbeddingQueueStatus", {},
    );
  },

  getQdrantRoles() {
    return grpcUnary<Record<string, never>, GetQdrantRolesResponse>(
      "operation.blockchain.v1.BlockchainService", "GetQdrantRoles", {},
    );
  },
};

// ── BtrfsService (operation.btrfs.v1) ───────────────────────────────────────

export const btrfsService = {
  getSubvolumes() {
    return grpcUnary<Record<string, never>, GetSubvolumesResponse>(
      "operation.btrfs.v1.BtrfsService", "GetSubvolumes", {},
    );
  },

  getSnapshots(req: Partial<BtrfsGetSnapshotsRequest> = {}) {
    return grpcUnary<BtrfsGetSnapshotsRequest, BtrfsGetSnapshotsResponse>(
      "operation.btrfs.v1.BtrfsService", "GetSnapshots", {
        subvolume: req.subvolume ?? "",
        limit: req.limit ?? 50,
      },
    );
  },

  getSendState() {
    return grpcUnary<Record<string, never>, GetSendStateResponse>(
      "operation.btrfs.v1.BtrfsService", "GetSendState", {},
    );
  },

  getDrStatus() {
    return grpcUnary<Record<string, never>, GetDrStatusResponse>(
      "operation.btrfs.v1.BtrfsService", "GetDrStatus", {},
    );
  },
};

// ── PersonaService (operation.agents.v1) ────────────────────────────────────

export const personaService = {
  listPersonas() {
    return grpcUnary<Record<string, never>, ListPersonasResponse>(
      "operation.agents.v1.PersonaService", "ListPersonas", {},
    );
  },

  getPersona(req: GetPersonaRequest) {
    return grpcUnary<GetPersonaRequest, GetPersonaResponse>(
      "operation.agents.v1.PersonaService", "GetPersona", req,
    );
  },

  createPersona(req: CreatePersonaRequest) {
    return grpcUnary<CreatePersonaRequest, CreatePersonaResponse>(
      "operation.agents.v1.PersonaService", "CreatePersona", req,
    );
  },

  updatePersona(req: UpdatePersonaRequest) {
    return grpcUnary<UpdatePersonaRequest, UpdatePersonaResponse>(
      "operation.agents.v1.PersonaService", "UpdatePersona", req,
    );
  },

  deletePersona(req: DeletePersonaRequest) {
    return grpcUnary<DeletePersonaRequest, DeletePersonaResponse>(
      "operation.agents.v1.PersonaService", "DeletePersona", req,
    );
  },

  listAgentRoutes() {
    return grpcUnary<Record<string, never>, ListAgentRoutesResponse>(
      "operation.agents.v1.PersonaService", "ListAgentRoutes", {},
    );
  },
};

// ── DataStoreService (operation.stores.v1) ──────────────────────────────────

export const dataStoreService = {
  getDataStores() {
    return grpcUnary<Record<string, never>, GetDataStoresResponse>(
      "operation.stores.v1.DataStoreService", "GetDataStores", {},
    );
  },

  getStoreDetail(req: GetStoreDetailRequest) {
    return grpcUnary<GetStoreDetailRequest, GetStoreDetailResponse>(
      "operation.stores.v1.DataStoreService", "GetStoreDetail", req,
    );
  },
};

// ── EmbeddingService (operation.embedding.v1) ───────────────────────────────

export const embeddingService = {
  getQueue(req: Partial<GetEmbeddingQueueRequest> = {}) {
    return grpcUnary<GetEmbeddingQueueRequest, GetEmbeddingQueueResponse>(
      "operation.embedding.v1.EmbeddingService", "GetQueue", {
        limit: req.limit ?? 50,
        statusFilter: req.statusFilter ?? "",
      },
    );
  },

  getWorkerStatus() {
    return grpcUnary<Record<string, never>, GetEmbeddingWorkerStatusResponse>(
      "operation.embedding.v1.EmbeddingService", "GetWorkerStatus", {},
    );
  },

  previewEmbeddingText(req: PreviewEmbeddingTextRequest) {
    return grpcUnary<PreviewEmbeddingTextRequest, PreviewEmbeddingTextResponse>(
      "operation.embedding.v1.EmbeddingService", "PreviewEmbeddingText", req,
    );
  },

  getChannelDiagnostics() {
    return grpcUnary<Record<string, never>, GetChannelDiagnosticsResponse>(
      "operation.embedding.v1.EmbeddingService", "GetChannelDiagnostics", {},
    );
  },
};
