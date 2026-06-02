/**
 * Blockchain & Vector Pipeline types (operation.blockchain.v1)
 * @see docs/architecture-flow.md §2
 */

// ── Plugin Footprint ────────────────────────────────────────────────────────

export interface PluginFootprint {
  blockHash: string;
  pluginId: string;
  operation: string;
  actor: string;
  outcome: string;
  summary: string;
  sessionId: string;
  timestamp: string;
  previousHash: string;
  blockIndex: number;
}

// ── Embedding Request (mirrors the internal EmbedRequest) ───────────────────

export interface EmbedRequest {
  blockHash: string;         // point ID in Qdrant
  embeddingText: string;     // constructed from footprint_to_embedding_text()
  collection: string;        // target Qdrant collection
  payload: Record<string, unknown>; // plugin_id, op, ts, session_id, ...
}

// ── Blockchain RPCs ─────────────────────────────────────────────────────────

export interface GetFootprintsRequest {
  fromIndex?: number;
  toIndex?: number;
  limit?: number;
  pluginId?: string;
}

export interface GetFootprintsResponse {
  footprints: PluginFootprint[];
  totalBlocks: number;
  chainValid: boolean;
}

export interface VerifyBlockchainRequest {
  fromIndex?: number;
  toIndex?: number;
}

export interface VerifyBlockchainResponse {
  valid: boolean;
  checkedBlocks: number;
  firstInvalidIndex?: number;
  errors: string[];
}

export interface GetEmbeddingQueueStatusResponse {
  channelCapacity: number;
  channelUsed: number;
  droppedCount: number;
  workerActive: boolean;
  retryPending: number;
  lastEmbeddedAt: string;
  lastErrorAt?: string;
  lastError?: string;
}

export interface QdrantCollectionRole {
  name: string;
  role: "ai_analysis" | "disaster_recovery" | "offsite_backup";
  pointCount: number;
  diskSizeBytes: number;
  lastUpdated: string;
}

export interface GetQdrantRolesResponse {
  roles: QdrantCollectionRole[];
  qdrantEndpoint: string;
  qdrantProtocol: string;
}
