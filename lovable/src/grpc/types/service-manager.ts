/**
 * TypeScript types for opdbus.services.v1 ServiceManager.
 * Generated from: crates/op-services/proto/services.proto
 */

// ── Enums ───────────────────────────────────────────────────────────────────

export enum ServiceType {
  SIMPLE = 0,
  FORKING = 1,
  ONESHOT = 2,
  NOTIFY = 3,
}

export enum RestartCondition {
  NEVER = 0,
  ALWAYS = 1,
  ON_FAILURE = 2,
}

export enum ServiceState {
  STOPPED = 0,
  STARTING = 1,
  RUNNING = 2,
  STOPPING = 3,
  FAILED = 4,
}

// ── Messages ────────────────────────────────────────────────────────────────

export interface ExecConfig {
  startProgram: string;
  startArgs: string[];
  stopProgram?: string;
  stopArgs: string[];
  workingDir?: string;
  user?: string;
  group?: string;
}

export interface RestartPolicy {
  condition: RestartCondition;
  delay: string; // Duration as string
  maxRetries?: number;
}

export interface ResourceLimits {
  memoryMax?: number;
  cpuQuota?: number;
  tasksMax?: number;
}

export interface HealthCheck {
  program: string;
  args: string[];
  interval: string;
  timeout: string;
  retries: number;
}

export interface ServiceDef {
  name: string;
  type: ServiceType;
  exec: ExecConfig;
  dependsOn: string[];
  restart: RestartPolicy;
  environment: Record<string, string>;
  resources?: ResourceLimits;
  healthCheck?: HealthCheck;
  enabled: boolean;
}

export interface ServiceStatus {
  name: string;
  state: ServiceState;
  pid?: number;
  error?: string;
  startedAt?: string;
}

export interface ServiceEvent {
  name: string;
  oldState: ServiceState;
  newState: ServiceState;
  timestamp: string;
}

// ── Request/Response ────────────────────────────────────────────────────────

export interface ServiceActionResponse {
  status: ServiceStatus;
}

export interface GetServiceResponse {
  service: ServiceDef;
  status: ServiceStatus;
}

export interface ListServicesResponse {
  services: ServiceDef[];
}

export interface WatchServicesRequest {
  names: string[];
}
