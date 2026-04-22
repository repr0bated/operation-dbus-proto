# 3tched Schema Shuttle and Xray Injection Pipeline

This spec defines the requirements, design, and implementation tasks for the 3tched Schema Shuttle and Xray Injection Pipeline subsystem.

## Overview

The 3tched Schema Shuttle and Xray Injection Pipeline subsystem implements a state-aware network transport layer that cryptographically binds ephemeral WireGuard user sessions to the authoritative JSON-RPC mutation pipeline. This feature eliminates legacy SQL polling and D-Bus watchers in favor of zero-copy shared memory access, ensuring minimal overhead and maximum accountability.

## Documents

- **Requirements**: Detailed requirements for the subsystem
- **Design**: Architecture and system design for the subsystem
- **Tasks**: Implementation-ready tasks in logical order

## Key Components

- **The Sled**: A 1:1 zero-copy shared memory layout mapping directly to the active `PluginSchema`
- **The Shuttle**: A pure Rust binary that performs zero-copy reads and passes cryptographic footprints to Xray
- **Xray**: An in-memory payload carrier that injects Ghostbridge headers into gRPC metadata. Sits before the gRPC bridge and WARP tunnel.
- **JSON-RPC Mutation Pipeline**: The authoritative path for all state changes, mutation events, approvals, trace updates, and Xray injection triggers

## Architecture

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
│  │ - Sits before the WARP tunnel and gRPC Bridge                         │  │
│  │ - Injects X-Ghostbridge-Footprint into gRPC metadata                 │  │
│  │ - Injects X-Ghostbridge-Trace-ID into gRPC metadata                  │  │
│  │ - Hands off to WARP Tunnel for transport                             │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
                                    │
                                    │ gRPC + Headers (via WARP Tunnel)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              gRPC Bridge                                    │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ - Receives payload from WARP Tunnel                                   │  │
│  │ - Validates Ghostbridge headers                                       │  │
│  │ - Routes to appropriate internal services                             │  │
│  │ - Ensures end-to-end accountability                                   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
```

## Implementation Phases

1. **The Sled (Zero-Copy Shared Memory)**: Implement the IdentitySled struct and memory mapping
2. **The Shuttle (Rust Courier)**: Implement the Shuttle that reads the Sled and passes data to Xray
3. **Xray (gRPC Header Injection)**: Implement Xray that injects Ghostbridge headers into gRPC metadata
4. **JSON-RPC Mutation Pipeline**: Implement the JSON-RPC mutation pipeline for state changes
5. **AI Accountability**: Implement AI accountability with constrained, auditable, traceable, explainable, and accountable AI
6. **Trace Propagation**: Implement trace propagation across all identity mutations, state transitions, and Xray injection events
7. **Canonicalization and Hashing**: Implement canonicalization before hashing for deterministic and reproducible hashing
8. **Mutation-Index-Driven Updates**: Implement mutation-index-driven state updates
9. **Xray Environment Injection and Reload**: Implement dynamic Xray environment injection and reload
10. **Error Handling and Recovery**: Implement graceful error handling and recovery
11. **Observability and Monitoring**: Implement health and performance monitoring
12. **Security and Policy Enforcement**: Implement security policy enforcement
13. **Compliance and Explainability**: Implement compliance and explainability enforcement
14. **Testing and Simulation**: Implement unit, integration, end-to-end, property-based, performance, and simulation tests
15. **Documentation**: Implement reference, developer, and user documentation
16. **Deployment**: Implement installation, upgrade, and configuration management scripts

## Getting Started

1. Review the requirements document to understand the subsystem requirements
2. Review the design document to understand the architecture and system design
3. Review the tasks document to understand the implementation tasks
4. Start implementing the tasks in logical order

## Related Crates

- **op-identity**: Identity crate with WireGuard pubkey as identity and OAuth token cache
- **op-grpc-bridge**: D-Bus <-> gRPC bidirectional bridge with event chain integration
- **op-jsonrpc**: JSON-RPC server with OVSDB and NonNet database support
- **op-network**: Network configuration using OVSDB
- **op-plugins**: Plugin system using JSON-RPC
- **op-services**: Service management via RPC

## License

MIT License - See LICENSE file for details
