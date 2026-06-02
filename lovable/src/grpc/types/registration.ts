/**
 * TypeScript types for operation.registration.v1 RegistrationService.
 * Generated from: crates/op-grpc-bridge/proto/registration.proto
 */

import type { ProtobufStruct } from "../google/protobuf/struct";

export interface SendMagicLinkRequest {
  email: string;
  domain: string;
  isAdmin: boolean;
  customMessage?: string;
}

export interface SendMagicLinkResponse {
  success: boolean;
  message: string;
  token?: string;
  expiresAt: string;
}

export interface VerifyMagicLinkRequest {
  token: string;
  domain: string;
}

export interface VerifyMagicLinkResponse {
  success: boolean;
  userId: string;
  email: string;
  wireguardPublicKey: string;
  assignedIp: string;
  wireguardConfig: string;
  message: string;
  isAdmin: boolean;
  verifiedAt: string;
}

export interface RegisterUserRequest {
  email: string;
  wireguardPublicKey: string;
  domain: string;
  isAdmin: boolean;
  metadata?: ProtobufStruct;
}

export interface RegisterUserResponse {
  success: boolean;
  userId: string;
  message: string;
  assignedIp: string;
  wireguardConfig: string;
  registeredAt: string;
}

export interface GetUserStatusRequest {
  email: string;
  userId: string;
  domain: string;
}

export interface GetUserStatusResponse {
  registered: boolean;
  userId: string;
  email: string;
  emailVerified: boolean;
  wireguardPublicKey: string;
  assignedIp: string;
  isAdmin: boolean;
  registeredAt: string;
  lastActive: string;
}

export interface ListUsersRequest {
  limit: number;
  offset: number;
  includeAdminsOnly: boolean;
  domainFilter: string;
}

export interface ListUsersResponse {
  users: UserInfo[];
  totalCount: number;
  filteredCount: number;
}

export interface UserInfo {
  userId: string;
  email: string;
  emailVerified: boolean;
  wireguardPublicKey: string;
  assignedIp: string;
  isAdmin: boolean;
  registeredAt: string;
  lastActive: string;
  metadata?: ProtobufStruct;
}

export interface GetWireGuardConfigRequest {
  email: string;
  userId: string;
  domain: string;
}

export interface GetWireGuardConfigResponse {
  success: boolean;
  wireguardConfig: string;
  publicKey: string;
  assignedIp: string;
  message: string;
  generatedAt: string;
}

export interface AdminUserActionRequest {
  action: string;
  userId: string;
  email: string;
  parameters?: ProtobufStruct;
}

export interface AdminUserActionResponse {
  success: boolean;
  message: string;
  userId: string;
  actionTimestamp: string;
}
