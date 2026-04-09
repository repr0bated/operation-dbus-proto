// ── Bridge services ──────────────────────────────────────────────────────────
export {
  stateSync, pluginService, eventChainService,
  ovsdbMirror, runtimeMirror, componentRegistry,
  // Domain services
  mailService, privacyService, registrationService,
  serviceManager, mcpService, accountabilityService,
  // Architecture services (§2,5,6,8,9)
  blockchainService, btrfsService, personaService,
  dataStoreService, embeddingService,
  // Transport
  getTransport, resetTransport,
} from "./client";

// ── Types: operation.v1 (re-exported from split files) ──────────────────────
export * from "./types/operation";

// ── Types: registry ─────────────────────────────────────────────────────────
export * from "./types/registry";

// ── Types: domain services ──────────────────────────────────────────────────
export * from "./types/mail";
export * from "./types/privacy";
export * from "./types/registration";
export * from "./types/service-manager";
export * from "./types/mcp";
export * from "./types/accountability";
export * from "./types/blockchain";
export * from "./types/btrfs";
export * from "./types/persona";
export * from "./types/data-stores";
export * from "./types/embedding";

// ── Protobuf helpers ────────────────────────────────────────────────────────
export { structToObject, valueToJs, objectToStruct } from "./google/protobuf/struct";
