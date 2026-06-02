/**
 * TypeScript types for operation.v1 RuntimeMirror service.
 * Generated from: crates/op-grpc-bridge/proto/operation.proto
 */

// ── Messages ────────────────────────────────────────────────────────────────

export interface RuntimeSystemInfo {
  hostname: string;
  kernelVersion: string;
  uptimeSeconds: number;
  bootTimestamp: number;
  cpuCount: number;
  memoryTotalBytes: number;
  memoryAvailableBytes: number;
  memoryUsedBytes: number;
  initSystem: string;
  arch: string;
  queriedAt: string;
}

export interface RuntimeServiceInfo {
  name: string;
  state: string;
  pid: number;
  enabled: boolean;
  description: string;
  dependencies: string[];
  startedAt: string;
}

export interface RuntimeListServicesResponse {
  services: RuntimeServiceInfo[];
}

export interface RuntimeStreamMetricsRequest {
  intervalSeconds: number;
  categories: string[];
}

export interface RuntimeMetricUpdate {
  category: string;
  name: string;
  value: number;
  unit: string;
  labels: Record<string, string>;
  timestamp: string;
}

export interface RuntimeNetworkInterface {
  name: string;
  index: number;
  macAddress: string;
  state: string;
  mtu: number;
  ipv4Addresses: string[];
  ipv6Addresses: string[];
  rxBytes: number;
  txBytes: number;
  rxPackets: number;
  txPackets: number;
  driver: string;
  speedMbps: number;
}

export interface RuntimeListInterfacesResponse {
  interfaces: RuntimeNetworkInterface[];
}

export interface NumaNode {
  nodeId: number;
  cpus: number[];
  memoryTotalBytes: number;
  memoryFreeBytes: number;
  memoryUsedBytes: number;
}

export interface RuntimeNumaTopologyResponse {
  nodes: NumaNode[];
}
