/**
 * Embedding Flow types (operation.embedding.v1)
 * @see docs/architecture-flow.md §9
 */

// ── EmbedRequest lifecycle ──────────────────────────────────────────────────

export interface EmbedRequestInfo {
  blockHash: string;
  embeddingText: string;
  collection: string;
  payload: Record<string, unknown>;
  status: "queued" | "processing" | "completed" | "failed" | "dropped";
  attempt: number;         // current retry attempt (max 5)
  queuedAt: string;
  processedAt?: string;
  error?: string;
}

// ── Queue monitoring ────────────────────────────────────────────────────────

export interface GetEmbeddingQueueRequest {
  limit?: number;
  statusFilter?: string;
}

export interface GetEmbeddingQueueResponse {
  requests: EmbedRequestInfo[];
  queueDepth: number;
  channelCapacity: number;  // mpsc(1024)
  channelUsed: number;
  totalProcessed: number;
  totalDropped: number;
  totalFailed: number;
}

// ── Worker status ───────────────────────────────────────────────────────────

export interface EmbeddingWorkerStatus {
  active: boolean;
  currentRequest?: string;  // block_hash being processed
  provider: string;          // "openclaw:embedder-voyage4lite" or "op-ml"
  model: string;             // e.g. "voyage-4-lite"
  vectorDimension: number;   // 1024
  retryConfig: {
    maxAttempts: number;     // 5
    baseDelayMs: number;     // 500
    backoffMultiplier: number; // exponential
  };
  stats: {
    embedsCompleted: number;
    embedsFailed: number;
    avgLatencyMs: number;
    uptime: string;
  };
}

export interface GetEmbeddingWorkerStatusResponse {
  worker: EmbeddingWorkerStatus;
}

// ── Text construction ───────────────────────────────────────────────────────

export interface EmbeddingTextPreview {
  blockHash: string;
  constructedText: string;  // "plugin={plugin_id} operation={operation} actor=..."
  fields: {
    pluginId: string;
    operation: string;
    actor: string;
    outcome: string;
    summary: string;
  };
}

export interface PreviewEmbeddingTextRequest {
  blockHash: string;
}

export interface PreviewEmbeddingTextResponse {
  preview: EmbeddingTextPreview;
}

// ── Channel semantics ───────────────────────────────────────────────────────

export interface ChannelDiagnostics {
  capacity: number;          // 1024
  currentUsage: number;
  dropPolicy: string;        // "try_send, silent drop on full"
  rationale: string;         // "embedding is runtime cognitive ability, not audit"
  auditSourceOfTruth: string; // "BTRFS timing_subvol (never dropped)"
  totalDropped: number;
  dropRate: number;           // drops per minute
}

export interface GetChannelDiagnosticsResponse {
  channel: ChannelDiagnostics;
}
