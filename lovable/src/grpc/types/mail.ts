/**
 * TypeScript types for operation.mail.v1 MailService.
 * Generated from: crates/op-grpc-bridge/proto/mail.proto
 */

import type { ProtobufStruct } from "../google/protobuf/struct";

export interface SendEmailRequest {
  fromEmail: string;
  toEmail: string;
  subject: string;
  body: string;
  isHtml: boolean;
  domain: string;
  attachments?: ProtobufStruct;
}

export interface SendEmailResponse {
  success: boolean;
  message: string;
  messageId: string;
  sentAt: string;
}

export interface GetInboxRequest {
  email: string;
  domain: string;
  limit: number;
  offset: number;
  folder: string;
}

export interface GetInboxResponse {
  messages: EmailMessage[];
  totalCount: number;
  unreadCount: number;
  folder: string;
}

export interface EmailMessage {
  messageId: string;
  from: string;
  to: string;
  subject: string;
  preview: string;
  isRead: boolean;
  hasAttachments: boolean;
  receivedAt: string;
  sizeBytes: number;
  folder: string;
}

export interface GetMessageRequest {
  messageId: string;
  email: string;
  domain: string;
}

export interface GetMessageResponse {
  success: boolean;
  header: EmailMessage;
  body: string;
  isHtml: boolean;
  attachments: EmailAttachment[];
  rawContent: string;
}

export interface EmailAttachment {
  filename: string;
  contentType: string;
  sizeBytes: number;
  contentId: string;
}

export interface GetMailStatusRequest {
  domain: string;
}

export interface GetMailStatusResponse {
  isConfigured: boolean;
  isRunning: boolean;
  mailServerType: string;
  webmailUrl: string;
  smtpStatus: string;
  imapStatus: string;
  totalAccounts: number;
  totalMessages: number;
  lastChecked: string;
  message: string;
}

export interface ListMailAccountsRequest {
  domain: string;
  includeInactive: boolean;
}

export interface ListMailAccountsResponse {
  accounts: MailAccount[];
  totalCount: number;
}

export interface MailAccount {
  email: string;
  userId: string;
  isAdmin: boolean;
  isActive: boolean;
  createdAt: string;
  messageCount: number;
  unreadCount: number;
  lastLogin: string;
}

export interface AdminMailActionRequest {
  action: string;
  email: string;
  domain: string;
  parameters?: ProtobufStruct;
}

export interface AdminMailActionResponse {
  success: boolean;
  message: string;
  actionId: string;
  timestamp: string;
  result?: ProtobufStruct;
}

export interface CheckMailServerRequest {
  domain: string;
  checkSmtp: boolean;
  checkImap: boolean;
  checkWebmail: boolean;
}

export interface CheckMailServerResponse {
  allHealthy: boolean;
  smtpHealthy: boolean;
  imapHealthy: boolean;
  webmailHealthy: boolean;
  smtpStatus: string;
  imapStatus: string;
  webmailStatus: string;
  message: string;
  issues: string[];
}
