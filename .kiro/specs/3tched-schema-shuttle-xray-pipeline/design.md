# Design Document

## Overview

The 3tched Schema Shuttle and Xray Injection Pipeline subsystem implements a state-aware network transport layer that cryptographically binds ephemeral WireGuard user sessions to the authoritative JSON-RPC mutation pipeline. This design eliminates legacy SQL polling and D-Bus watchers in favor of zero-copy shared memory access, ensuring minimal overhead and maximum accountability.

## Architectural Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         JSON-RPC Mutation Pipeline                          │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ THE AUTHORITY: Sole path for all state changes                        │  │
│  │ - Mutation proposals                                                  │  │
│  │ - Validation against PluginSchema                                     │  │
│  │ - Approval/Rejection                                                  │  │
│  │ - Mutation Index Update                                               │  │
│  │ - Audit Trail                                                         │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
                                    │
                                    │ Mutation Events
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              The Sled (Shared Memory)                       │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ /dev/shm/plugin_schema.dat (#[repr(C)])                              │  │
│  │ - WireGuard Public Key                                                │  │
│  │ - Mutation Index                                                      │  │
│  │ - Blake3 Hashed Footprint ("Thought")                                │  │
│  │ - Trace ID                                                            │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Zero-Copy Read (Raw Pointer Cast)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            The Shuttle (Rust Courier)                       │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ - Extracts footprint and trace ID                                     │  │
│  │ - Passes to Xray via environment variables (GB_FOOTPRINT, GB_TRACE_ID)│  │
│  │ - Detects and aborts any disk I/O                                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
                                    │
                                    │ Environment Variables
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Xray (Payload Carrier)                         │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ - Injects X-Ghostbridge-Footprint into gRPC metadata                 │  │
│  │ - Injects X-Ghostbridge-Trace-ID into gRPC metadata                  │  │
│  │ - Targets OpenClaw gRPC bridge on 127.0.0.1:18789                    │  │
│  │ - Maintains Accountability Loop via OpenClaw Trusted Proxy auth      │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
                                    │
                                    │ gRPC Metadata
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OpenClaw gRPC Bridge                                │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ - Validates Ghostbridge headers                                       │  │
│  │ - Enforces Accountability Loop                                        │  │
│  │ - Routes to appropriate services                                      │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
```

## JSON-RPC Mutation Pipeline Architecture

### The JSON-RPC Mutation Pipeline as the Sole Authoritative Path

The JSON-RPC mutation pipeline is designed as the ONLY authorized path for all state changes:

1. **Mutation Proposal**: All mutation events are submitted to the JSON-RPC mutation pipeline
2. **Validation**: The JSON-RPC mutation pipeline validates the mutation against the `PluginSchema`
3. **Approval**: The JSON-RPC mutation pipeline generates a mutation event record with timestamp, proposer, and validation result
4. **Commit**: Where a mutation is approved, the JSON-RPC mutation pipeline updates the `mutation_index` in the Sled
5. **Audit**: The JSON-RPC mutation pipeline maintains an immutable audit trail of all mutation proposals, validations, approvals, and commits

### JSON-RPC Mutation Pipeline Stages

```
┌────────────────────────────────���────────────────────────────────────────────┐
│                    JSON-RPC Mutation Pipeline Stages                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Stage 1: Mutation Proposal                                            │  │
│  │   - Submit mutation event with metadata                               │  │
│  │   - Include proposer, timestamp, and mutation details                 │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Stage 2: Validation against PluginSchema                            │  │
│  │   - Validate mutation against current schema state                    │  │
│  │   - Check for conflicts and consistency                               │  │
│  │   - Generate validation result                                        │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Stage 3: Approval/Rejection                                         │  │
│  │   - Approve mutation if validation passes                             │  │
│  │   - Reject mutation if validation fails                               │  │
│  │   - Record approval/rejection with trace ID                           │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Stage 4: Mutation Index Update                                      │  │
│  │   - Update mutation_index in Sled via zero-copy                       │  │
│  │   - Recalculate hashed footprint                                      │  │
│  │   - Update trace ID if needed                                         │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Stage 5: Audit Trail Update                                         │  │
│  │   - Log mutation event with trace ID                                  │  │
│  │   - Log approval/rejection with trace ID                              │  │
│  │   - Maintain immutable audit trail                                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Zero-Copy Memory Mapping

### The Sled Implementation

The Sled is implemented as a 1:1 zero-copy shared memory layout using `#[repr(C)]` to ensure deterministic memory layout:

```rust
#[repr(C)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],      // 32-byte WireGuard public key
    pub mutation_index: u64,             // Current mutation index
    pub hashed_footprint: [u8; 32],      // Blake3 hashed footprint
    pub trace_id: [u8; 16],              // UUID v4 trace ID
    pub schema_version: u32,             // Schema version for compatibility
    pub reserved: [u8; 60],              // Reserved for future use
}
```

### Memory Mapping Process

1. **Initialization**: The Sled opens `/dev/shm/plugin_schema.dat` with `O_RDWR` flags
2. **Memory Mapping**: Uses `mmap` to map the file into memory with `MAP_SHARED`
3. **Pointer Casting**: Casts the mapped pointer to `*const IdentitySled`
4. **Access**: Provides safe access via `&IdentitySled` reference
5. **Synchronization**: Uses memory barriers to ensure visibility of changes

### Safety Guarantees

- **No Copy**: The Sled provides direct access to the underlying memory
- **No Mutation**: The Shuttle performs read-only operations on the Sled
- **No Disk I/O**: All operations occur in `/dev/shm` (tmpfs), avoiding any disk writes

## gRPC Header Injection

### Xray Implementation

Xray is implemented as a gRPC middleware that intercepts outbound calls and injects Ghostbridge headers:

```rust
pub struct GhostbridgeInterceptor {
    footprint: String,
    trace_id: String,
    wireguard_pubkey: Option<String>,
}

impl Interceptor for GhostbridgeInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request.metadata_mut().insert(
            "x-ghostbridge-footprint",
            self.footprint.parse().map_err(|_| Status::invalid_argument("Invalid footprint"))?,
        );
        request.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            self.trace_id.parse().map_err(|_| Status::invalid_argument("Invalid trace ID"))?,
        );
        if let Some(pubkey) = &self.wireguard_pubkey {
            request.metadata_mut().insert(
                "x-wireguard-pubkey",
                pubkey.parse().map_err(|_| Status::invalid_argument("Invalid WireGuard pubkey"))?,
            );
        }
        Ok(request)
    }
}
```

### Outbound Configuration

Xray's outbound configuration targets the OpenClaw gRPC bridge:

```yaml
outbounds:
  - tag: openclaw
    type: grpc
    server: 127.0.0.1
    server_port: 18789
    settings:
      transport:
        httpSettings:
          path: /xray.GhostbridgeService/InjectHeaders
          headers:
            X-Ghostbridge-Footprint: ${GB_FOOTPRINT}
            X-Ghostbridge-Trace-ID: ${GB_TRACE_ID}
            X-WireGuard-Pubkey: ${GB_WIREGUARD_PUBKEY}
```

### Accountability Loop

The Accountability Loop is maintained by:

1. **Header Injection**: Xray injects Ghostbridge headers into all outbound gRPC calls
2. **Header Validation**: OpenClaw validates the headers against the `PluginSchema`
3. **Audit Logging**: All validated calls are logged to the Snowball session ledger
4. **Trace Propagation**: The trace ID is propagated through all service calls

## Zero-Disk I/O Strategy

### Zero-Disk I/O Strategy

The system avoids any disk I/O by:

1. **In-Memory Storage**: All identity data is stored in `/dev/shm` (tmpfs), not on disk
2. **Environment Variables**: The Shuttle passes data to Xray via environment variables, not disk I/O
3. **No JSON-RPC Polling**: The system eliminates polling loops that could trigger disk writes
4. **No D-Bus Watchers**: The system eliminates D-Bus watchers that could trigger disk writes

### NVMe Preservation

NVMe I/O is preserved strictly for the Btrfs vectorized footprint transport (blockchain):

1. **Footprint Transport**: Only the final hashed footprints are written to Btrfs
2. **Vectorized Writes**: Footprints are batched and written in large, sequential blocks
3. **Blockchain Structure**: The Btrfs writes follow a blockchain structure with cryptographic links

## Project Structure

```
crates/
├── op-identity/
│   ├── op-sled/              # The Sled: Zero-copy shared memory
│   ├── op-shuttle/           # The Shuttle: Rust courier
│   └── op-anna-scribe/       # A.N.N.A. Scribe: Identity notary
├── op-grpc-bridge/
│   ├── op-xray/              # Xray: gRPC header injection
│   └── op-openclaw/          # OpenClaw: gRPC bridge
├── op-jsonrpc/
│   └── op-mutation-pipeline/ # JSON-RPC Mutation Pipeline
├── op-compliance/
│   ├── op-olivia-scal/       # Olivia Scal: OSCAL
│   ├── op-eugene-risk/       # E.U.gene Risk: EU AI Act
│   ├── op-penny-privacy/     # Penny Privacy: GDPR
│   └── op-reggie-opa/        # Reggie O.P.A.: Cloud Prosecutor
└── op-web/
    └── ui/
        └── src/
            └── pages/
                └── AccountabilityPage.tsx  # Accountability Loop UI
```

## Implementation Plan

### Phase 1: The Sled (Zero-Copy Shared Memory)

1. Implement `IdentitySled` struct with `#[repr(C)]`
2. Implement memory mapping via `mmap`
3. Implement safe access via `&IdentitySled` reference
4. Implement memory barriers for synchronization

### Phase 2: The Shuttle (Rust Courier)

1. Implement raw pointer cast to read the Sled
2. Implement footprint and trace ID extraction
3. Implement environment variable passing to Xray
4. Implement disk I/O detection and abort

### Phase 3: Xray (gRPC Header Injection)

1. Implement `GhostbridgeInterceptor` for gRPC metadata injection
2. Implement outbound configuration for OpenClaw gRPC bridge
3. Implement environment variable extraction
4. Implement Accountability Loop maintenance

### Phase 4: JSON-RPC Mutation Pipeline

1. Implement mutation proposal endpoint
2. Implement validation against `PluginSchema`
3. Implement approval/rejection logic
4. Implement mutation index update in Sled
5. Implement audit trail

### Phase 5: Accountability Loop UI

1. Implement Chatbot component
2. Implement Qdrant semantic search component
3. Implement UI layout with Chatbot on top and Qdrant on bottom
4. Implement user action research and confrontation features

### Phase 6: Compliance Engine Integration

1. Implement Olivia Scal (OSCAL) attorney
2. Implement E.U.gene Risk (EU AI Act) attorney
3. Implement Penny Privacy (GDPR) attorney
4. Implement Reggie O.P.A. (Cloud Prosecutor) attorney
5. Implement validation against `PluginSchema`
6. Implement audit logging

## AI Accountability Design

### AI Accountability as a First-Class Principle

AI accountability is designed as a first-class principle in the subsystem:

1. **Constrained AI**: AI operations are constrained by the `PluginSchema` and cannot operate outside defined boundaries
2. **Auditable AI**: All AI-assisted decisions are logged with trace IDs for audit purposes
3. **Traceable AI**: Trace IDs link all AI-assisted decisions to the original request
4. **Explainable AI**: AI recommendations include metadata about the recommendation source, confidence level, and supporting evidence
5. **Accountable AI**: AI can recommend, interpret, or assist but never bypass validation and authorization controls

### AI Accountability Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              AI Accountability Loop                         │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ 1. AI generates recommendation with metadata                          │  │
│  │ 2. Recommendation is logged with trace ID                             │  │
│  │ 3. Human operator reviews and approves/rejects                        │  │
│  │ 4. Decision is logged with trace ID and reason                        │  │
│  │ 5. Action is executed if approved                                     │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Trace Propagation Design

### Trace Propagation Across All Identity Mutations

Trace propagation is designed to maintain trace linkage across all identity mutations, state transitions, and Xray injection events:

1. **Trace ID Generation**: When an identity mutation occurs, a new trace ID is generated or the existing trace ID is propagated
2. **Trace ID Inclusion**: The trace ID is included in all mutation event records, Xray injection headers, and audit log entries
3. **Trace ID Propagation**: Where a trace ID is propagated, it maintains the causal chain linking all related events
4. **Trace ID Storage**: The trace ID is stored in the Sled's trace_id field for zero-copy access
5. **Trace ID Logging**: The trace ID is included in the Snowball session ledger for audit purposes
6. **Trace ID Querying**: Where semantic memory linkage is required, the trace ID is used to query the Qdrant index

## Canonicalization and Hashing Design

### Canonicalization Before Hashing

Canonicalization is designed to ensure deterministic and reproducible hashing:

1. **Schema Canonicalization**: When schema state is prepared for hashing, it is canonicalized to a deterministic byte representation
2. **Semantic Preservation**: The canonicalization process preserves all semantic meaning while eliminating non-essential variations
3. **Hash Computation**: The hashed footprint is computed using Blake3 or SHA-256 on the canonicalized schema state
4. **Deterministic Change**: Where the schema state changes, the hashed footprint changes deterministically
5. **Idempotent Process**: The canonicalization and hashing process is idempotent and reproducible
6. **Footprint Storage**: The hashed footprint is stored in the Sled's hashed_footprint field for zero-copy access

## Mutation-Index-Driven Updates Design

### Mutation-Index-Driven State Updates

Mutation-index-driven updates are designed to keep runtime identity state in sync with schema state:

1. **Mutation Index Detection**: When the mutation index in the Sled changes, a state update event is triggered
2. **State Update Event**: The state update event includes the old mutation index, new mutation index, and timestamp
3. **Footprint Recalculation**: Where a state update is triggered, the system recalculates the hashed footprint
4. **Footprint Update**: Where the hashed footprint changes, the system updates the Sled's hashed_footprint field
5. **Trace ID Regeneration**: Where the trace ID needs regeneration, the system generates a new trace ID
6. **Zero-Copy Detection**: The mutation index change detection is performed via zero-copy shared memory access

## Xray Environment Injection and Reload Design

### Dynamic Xray Environment Injection

Xray environment injection is designed to dynamically inject environment variables and trigger Xray reload when state changes:

1. **Footprint Change Detection**: When the Sled's hashed_footprint changes, an Xray environment injection is triggered
2. **Environment Variable Update**: The Xray environment injection updates the `GB_FOOTPRINT` and `GB_TRACE_ID` environment variables
3. **Xray Reload Trigger**: Where environment variables are updated, an Xray reload is triggered
4. **Graceful Reload**: The Xray reload is performed without interrupting active gRPC connections
5. **Error Recovery**: Where the Xray reload fails, the system logs the error and attempts recovery
6. **Zero-Copy Injection**: The Xray environment injection is performed via zero-copy shared memory access

## Error Handling and Recovery Design

### Graceful Error Handling and Recovery

Error handling and recovery are designed to ensure the system remains operational under adverse conditions:

1. **Shared Memory Mapping Errors**: When a shared memory mapping fails, the system returns an error with a descriptive message
2. **JSON-RPC Mutation Pipeline Errors**: Where a JSON-RPC mutation pipeline request fails, the system retries with exponential backoff
3. **Xray Environment Injection Errors**: Where an Xray environment injection fails, the system logs the error and attempts recovery
4. **Mutation Index Change Detection Errors**: Where a mutation index change detection fails, the system logs the error and continues monitoring
5. **Trace ID Generation Errors**: Where a trace ID generation fails, the system uses a fallback trace ID or returns an error
6. **Canonicalization and Hashing Errors**: Where a canonicalization or hashing operation fails, the system logs the error and maintains the current state

## Observability and Monitoring Design

### Health and Performance Monitoring

Observability and monitoring are designed to detect issues proactively:

1. **Initialization Logging**: When the system initializes, it logs the initialization event with timestamp and configuration
2. **Mutation Event Logging**: Where a mutation event occurs, it logs the mutation event with trace ID and mutation index
3. **Xray Injection Logging**: Where an Xray injection occurs, it logs the injection event with footprint and trace ID
4. **Error Logging**: Where an error occurs, it logs the error with stack trace and context
5. **Metrics Exposure**: The system exposes metrics for shared memory access latency, mutation event rate, and Xray injection rate
6. **Monitoring Data Unavailability**: Where monitoring data is unavailable, it logs a warning and continues operation

## Security and Policy Enforcement Design

### Security Policy Enforcement

Security and policy enforcement are designed to prevent unauthorized access:

1. **Shared Memory Permission Validation**: When a shared memory mapping is requested, the system validates the caller's permissions
2. **JSON-RPC Request Signature Validation**: Where a JSON-RPC mutation pipeline request is received, the system validates the request signature
3. **Xray Environment Injection Permission Validation**: Where an Xray environment injection is requested, the system validates the caller's permissions
4. **Least Privilege Enforcement**: The system enforces least privilege for all operations
5. **Security Policy Violation Detection**: Where a security policy violation is detected, the system logs the violation and takes appropriate action
6. **Security Audit Logging**: The system maintains an audit log of all security-related events

## Compliance and Explainability Design

### Compliance and Explainability Enforcement

Compliance and explainability are designed to ensure all operations are compliant with regulatory requirements:

1. **Regulatory Validation**: When an operation is initiated, the system validates it against regulatory requirements
2. **AI Explainability**: Where an AI-assisted decision is made, the system provides an explainability record
3. **Compliance Violation Detection**: Where a compliance violation is detected, the system logs the violation and takes appropriate action
4. **Compliance Audit Logging**: The system maintains an audit log of all compliance-related events
5. **Explainability Record Provision**: Where an explainability record is requested, the system provides the record with trace ID
6. **Regulatory Compliance Enforcement**: The system enforces that all operations are compliant with OSCAL, EU AI Act, GDPR, and Cloud Prosecutor standards