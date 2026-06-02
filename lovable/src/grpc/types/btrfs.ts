/**
 * BTRFS Subvolume & Snapshot types (operation.btrfs.v1)
 * @see docs/architecture-flow.md §5
 */

// ── Subvolume layout ────────────────────────────────────────────────────────

export interface BtrfsSubvolume {
  name: string;
  path: string;
  purpose: "audit_ledger" | "dr_state" | "vector_storage";
  sizeBytes: number;
  usedBytes: number;
  snapshotCount: number;
  lastModified: string;
}

export interface GetSubvolumesResponse {
  subvolumes: BtrfsSubvolume[];
  raidLevel: string;     // e.g. "RAID-1"
  devices: string[];     // e.g. ["/dev/sda", "/dev/sdb"]
  totalBytes: number;
  usedBytes: number;
}

// ── Snapshots ───────────────────────────────────────────────────────────────

export interface BtrfsSnapshot {
  id: string;
  subvolume: string;
  createdAt: string;
  sizeBytes: number;
  parentSnapshotId?: string;
  pinned: boolean;          // pinned until all remotes confirm receipt
  pinnedRemotes: string[];  // remote names still needing this snapshot
}

export interface GetSnapshotsRequest {
  subvolume?: string;
  limit?: number;
}

export interface GetSnapshotsResponse {
  snapshots: BtrfsSnapshot[];
  totalCount: number;
}

// ── Incremental send state ──────────────────────────────────────────────────

export interface SendState {
  remoteName: string;
  remoteHost: string;
  remotePath: string;
  lastSentSnapshotId: string;
  lastSentAt: string;
  status: "idle" | "sending" | "error";
  lastError?: string;
  bytesTransferred: number;
}

export interface GetSendStateResponse {
  remotes: SendState[];
}

// ── DR Recovery ─────────────────────────────────────────────────────────────

export interface DrRecoveryStatus {
  lastCheckpoint: string;
  stateSubvolCurrent: boolean; // current.json exists and valid
  vectorSnapshotAvailable: boolean;
  blocksToReplay: number;
  recoverySteps: string[];    // ordered recovery procedure
}

export interface GetDrStatusResponse {
  status: DrRecoveryStatus;
  recoveryOrder: string[];    // ["boot_baseline", "apply_state", "restore_vectors", "replay_blocks"]
}
