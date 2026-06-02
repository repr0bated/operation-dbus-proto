/**
 * TypeScript types for operation.privacy.v1 PrivacyNetworkService.
 * Generated from: crates/op-grpc-bridge/proto/privacy_network.proto
 */

import type { ProtobufStruct } from "../google/protobuf/struct";

export interface EnsurePrivacyNetworkRequest {
  domain: string;
  forceReprovision: boolean;
  configOverrides?: ProtobufStruct;
}

export interface EnsurePrivacyNetworkResponse {
  success: boolean;
  message: string;
  bridgeName: string;
  wgcfStatus: string;
  xrayStatus: string;
  activePorts: string[];
  provisionedAt: string;
  topologySummary: string;
}

export interface GetNetworkStatusRequest {
  component: string;
}

export interface GetNetworkStatusResponse {
  healthy: boolean;
  overallStatus: string;
  components: NetworkComponent[];
  message: string;
  lastUpdated: string;
}

export interface NetworkComponent {
  name: string;
  status: string;
  type: string;
  ipAddress: string;
  details: string;
  critical: boolean;
}

export interface ProvisionUserRequest {
  email: string;
  wireguardPublicKey: string;
  isAdmin: boolean;
  domain: string;
  containerType: string;
  metadata?: ProtobufStruct;
}

export interface ProvisionUserResponse {
  success: boolean;
  userId: string;
  assignedIp: string;
  privacyConfig: string;
  message: string;
  provisionedAt: string;
  xrayEndpoint: string;
}

export interface GetPrivacyWireGuardConfigRequest {
  email: string;
  userId: string;
  includeXray: boolean;
}

export interface GetPrivacyWireGuardConfigResponse {
  success: boolean;
  wireguardConfig: string;
  publicKey: string;
  endpoint: string;
  assignedIp: string;
  dnsServers: string;
  message: string;
  generatedAt: string;
}

export interface ManageComponentRequest {
  action: string;
  component: string;
  parameters?: ProtobufStruct;
}

export interface ManageComponentResponse {
  success: boolean;
  message: string;
  component: string;
  status: string;
  output: string;
  completedAt: string;
}

export interface GetNetworkTopologyRequest {
  includeDetails: boolean;
}

export interface GetNetworkTopologyResponse {
  bridgeName: string;
  wgcfStatus: string;
  ports: string[];
  managementIp: string;
  xrayConfig: string;
  routes: NetworkRoute[];
  topologyData: ProtobufStruct;
  summary: string;
  proxyConfigs: ProxyConfig[];
}

export interface ProxyConfig {
  containerName: string;
  containerType: string;
  httpProxyEnabled: boolean;
  grpcProxyEnabled: boolean;
  httpPort: number;
  grpcPort: number;
  proxyMode: string;
}

export interface NetworkRoute {
  destination: string;
  gateway: string;
  device: string;
  metric: string;
}

export interface HealthCheckRequest {
  checkWgcf: boolean;
  checkOvs: boolean;
  checkXray: boolean;
  checkPorts: boolean;
}

export interface HealthCheckResponse {
  allHealthy: boolean;
  healthyComponents: number;
  totalComponents: number;
  issues: HealthIssue[];
  overallStatus: string;
  checkedAt: string;
}

export interface HealthIssue {
  component: string;
  severity: string;
  message: string;
  suggestedFix: string;
}

export interface ConfigurePacketRoutingRequest {
  containerName: string;
  containerType: string;
  enableHttpProxy: boolean;
  enableGrpcProxy: boolean;
  proxyType: string;
  socksPort: number;
  httpPort: number;
  enableTproxy: boolean;
}

export interface ConfigurePacketRoutingResponse {
  success: boolean;
  message: string;
  containerName: string;
  proxyConfigSummary: string;
  configuredAt: string;
  appliedRules: string[];
}

export interface GenerateWireGuardKeyPairRequest {
  userToken: string;
  userEmail: string;
  isAdmin: boolean;
  containerType: string;
}

export interface GenerateWireGuardKeyPairResponse {
  success: boolean;
  clientPublicKey: string;
  wireguardConfig: string;
  assignedIp: string;
  keyId: string;
  message: string;
  generatedAt: string;
}
