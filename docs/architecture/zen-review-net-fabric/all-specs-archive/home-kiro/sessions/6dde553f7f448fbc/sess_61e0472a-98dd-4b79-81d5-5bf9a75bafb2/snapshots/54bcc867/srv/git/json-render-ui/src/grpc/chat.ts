// Hand-written gRPC-Web client for ChatService
// Based on /srv/git/odbus/crates/op-chat/proto/chat.proto

import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";

// We'll use a simple fetch-based approach for grpc-web since full codegen is complex

export interface SendRequest {
  conversationId: string;
  uiMessages: Uint8Array; // JSON-encoded UIMessage[]
  personaId?: string;
  provider: string;
  model: string;
  resumeCursor?: bigint;
}

export interface ChatFrame {
  cursor: bigint;
  body:
    | { part: UIMessagePart }
    | { error: StreamError }
    | { heartbeat: Heartbeat }
    | { done: StreamDone }
    | { approval: ApprovalRequired };
}

export interface UIMessagePart {
  messageId: string;
  role: string;
  kind: string;
  payload: Uint8Array;
}

export interface StreamError {
  code: string;
  message: string;
  retryable: boolean;
  retryAfterMs?: number;
}

export interface Heartbeat {
  serverTimeMs: bigint;
}

export interface StreamDone {
  conversationId: string;
  totalParts: bigint;
}

export interface ApprovalRequired {
  toolCallId: string;
  toolName: string;
  toolInput: Uint8Array;
  description: string;
}

// Simple gRPC-Web client using fetch
// grpc-web uses base64 encoding for the wire format

export async function sendChat(
  message: string,
  model: string,
  provider: string,
  onFrame: (frame: { type: string; data: unknown }) => void
): Promise<void> {
  const conversationId = crypto.randomUUID();
  
  const uiMessages = JSON.stringify([
    { role: "user", content: message }
  ]);
  
  // For now, use the A2A endpoint which works
  // TODO: Switch to proper gRPC-Web when auth is sorted
  const response = await fetch("/a2a/dashboard", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: conversationId,
      method: "message/send",
      params: {
        message: {
          role: "user",
          parts: [{ kind: "text", text: message }],
        },
        metadata: { model, provider },
      },
    }),
  });

  const data = await response.json();
  
  if (data.result?.artifacts?.[0]?.parts?.[0]?.text) {
    onFrame({
      type: "text",
      data: {
        content: data.result.artifacts[0].parts[0].text,
        model,
        provider,
      },
    });
  } else if (data.error) {
    onFrame({
      type: "error",
      data: { message: data.error.message || JSON.stringify(data.error) },
    });
  }
  
  onFrame({ type: "done", data: { conversationId } });
}
