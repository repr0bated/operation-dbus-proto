/**
 * ChatbotAccountabilityService types (operation.accountability.v1)
 *
 * Semantic search over control-plane reasoning episodes stored in Qdrant.
 * @see docs/architecture-flow.md §3-4
 */

// ── Search request/response ─────────────────────────────────────────────────

export interface SearchEpisodesRequest {
  query: string;
  outcomeClass?: string;
  pluginId?: string;
  conversationId?: string;
  timeRangeStart?: string; // ISO timestamp
  timeRangeEnd?: string;   // ISO timestamp
  limit?: number;
}

export interface ReasoningEpisode {
  episodeId: string;
  goalText: string;
  trigger: string;
  toolsConsulted: string[];
  reasoningSummary: string;
  decisionOutput: string;
  outcomeClass: string;
  confidence: number;
  pluginId: string;
  conversationId: string;
  startedAt: string;
  endedAt: string;
  durationMs: number;
  exitReason: string;
  piiFlagged: boolean;
  similarity: number; // cosine similarity score from Qdrant
}

export interface SearchEpisodesResponse {
  episodes: ReasoningEpisode[];
  totalFound: number;
  queryVector?: number[]; // for debugging
}

// ── Get single episode ──────────────────────────────────────────────────────

export interface GetEpisodeRequest {
  episodeId: string;
}

export interface GetEpisodeResponse {
  episode: ReasoningEpisode;
  relatedEpisodes: ReasoningEpisode[];
}

// ── Collection stats ────────────────────────────────────────────────────────

export interface CollectionStatsResponse {
  collectionName: string;
  pointCount: number;
  vectorDimension: number;
  segmentCount: number;
  diskSizeBytes: number;
  ramSizeBytes: number;
  lastUpdated: string;
}

// ── Chat with context ───────────────────────────────────────────────────────

export interface ChatWithContextRequest {
  message: string;
  episodeIds: string[];       // episodes to include as context
  conversationHistory: ChatContextMessage[];
}

export interface ChatContextMessage {
  role: "user" | "assistant";
  content: string;
}

export interface ChatWithContextResponse {
  reply: string;
  referencedEpisodes: string[]; // episode IDs referenced in reply
  model: string;
}

// ── Episode lifecycle streaming (§3) ────────────────────────────────────────

export interface SubscribeEpisodesRequest {
  pluginId?: string;
  outcomeClassFilter?: string;
  includeExisting?: boolean;
}

export interface EpisodeEvent {
  eventType: "opened" | "closed" | "vectorized" | "pii_redacted";
  episode: ReasoningEpisode;
  vectorized: boolean;
  queuePosition?: number;   // position in embedding queue
}

// ── PII management ──────────────────────────────────────────────────────────

export interface GetPiiPolicyResponse {
  redactionEnabled: boolean;
  fieldsRedacted: string[];    // fields excluded from vector input
  piiFlaggedCount: number;
  lastFlaggedAt?: string;
}
