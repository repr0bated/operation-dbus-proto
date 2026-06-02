/**
 * Data Store health types (operation.stores.v1)
 * @see docs/architecture-flow.md §8
 */

export interface DataStoreStatus {
  name: string;
  location: string;
  description: string;
  durability: string;       // e.g. "Persistent + replicated"
  status: "healthy" | "degraded" | "unreachable";
  sizeBytes?: number;
  usedBytes?: number;
  lastChecked: string;
  latencyMs?: number;
  details: Record<string, unknown>;
}

export interface GetDataStoresResponse {
  stores: DataStoreStatus[];
}

// Individual store checks

export interface SqliteStoreStatus {
  path: string;
  sizeBytes: number;
  tables: string[];
  walMode: boolean;
  integrityCheck: "ok" | "corrupt";
}

export interface QdrantStoreStatus {
  endpoint: string;
  protocol: string;
  collections: {
    name: string;
    pointCount: number;
    vectorDimension: number;
    diskSizeBytes: number;
    ramSizeBytes: number;
    status: string;
  }[];
  healthy: boolean;
}

export interface BtrfsStoreStatus {
  subvolumes: string[];
  raidLevel: string;
  totalBytes: number;
  usedBytes: number;
  scrubStatus: string;
  lastScrubAt: string;
}

export interface EmbeddingChannelStatus {
  capacity: number;
  used: number;
  droppedTotal: number;
  workerAlive: boolean;
}

export interface GetStoreDetailRequest {
  storeName: string;
}

export interface GetStoreDetailResponse {
  sqlite?: SqliteStoreStatus;
  qdrant?: QdrantStoreStatus;
  btrfs?: BtrfsStoreStatus;
  embeddingChannel?: EmbeddingChannelStatus;
}
