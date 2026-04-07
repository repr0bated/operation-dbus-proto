# op-introspection - Requirements

## Problem Statement

Agents and external services need a way to discover and understand the available D-Bus services and interfaces on the system. Manually parsing D-Bus XML introspection data is complex and inefficient. The system requires a unified service that can discover, parse, cache, and index D-Bus introspection data into a JSON-serializable format suitable for MCP tools and AI interaction.

## Goals

1.  **Automated Discovery**: Automatically scan and list D-Bus services on both System and Session buses.
2.  **XML to JSON Transformation**: Efficiently parse D-Bus XML introspection data into structured, JSON-serializable Rust objects.
3.  **Performance via Caching**: Implement an asynchronous cache to avoid redundant and slow D-Bus introspection calls.
4.  **Semantic Search**: Provide a full-text search (FTS5) index for D-Bus services, interfaces, methods, and properties to enable semantic queries.
5.  **Schema Projection**: Project D-Bus interface definitions into simplified structures for high-level consumption.

## Functional Requirements

### FR1: Service Scanning
- Discover all active services on the D-Bus system bus and session bus.
- Retrieve basic service metadata (names, owners).
- Support filtering and targeted scanning of specific service namespaces.

### FR2: Introspection and Parsing
- Recursively introspect D-Bus object paths starting from a root path.
- Parse D-Bus XML introspection data into a comprehensive `ObjectInfo` structure.
- Extract interfaces, methods (with arguments and return types), signals, and properties.
- Map D-Bus types to structured Rust equivalents that are `Serialize`/`Deserialize` compatible.

### FR3: Caching
- Store introspection results in an in-memory or persistent cache (SQLite via `rusqlite`).
- Implement cache invalidation and expiration policies.
- Support asynchronous cache lookups and updates using `tokio`.

### FR4: Full-Text Search Indexing
- Maintain an SQLite FTS5 index of introspected D-Bus data.
- Index service names, interface names, method names, and property descriptions.
- Provide a `Search` API for semantic and keyword-based D-Bus discovery.
- Support real-time index updates as new services are discovered or introspected.

### FR5: Hierarchical Mapping
- Represent the D-Bus object hierarchy as a tree structure.
- Support "projections" of specific interfaces for simplified consumption.

## Non-Functional Requirements

### NFR1: Performance
- Use `simd-json` for high-performance JSON operations.
- Leverage `quick-xml` for fast XML parsing.
- Minimize latency for introspection calls through efficient caching.

### NFR2: Concurrency
- Ensure all introspection and indexing operations are non-blocking and thread-safe.
- Support concurrent introspection of multiple services.

### NFR3: Reliability
- Gracefully handle malformed XML or inaccessible D-Bus paths.
- Provide clear error reporting using `anyhow` and `thiserror`.
- Ensure database integrity for the FTS5 index and cache.

### NFR4: Scalability
- Efficiently handle systems with hundreds of D-Bus services and thousands of object paths.

## Success Criteria

1.  Successful discovery and listing of D-Bus services on System and Session buses.
2.  Accurate parsing of complex D-Bus XML into valid JSON-serializable structures.
3.  Introspection results correctly cached and retrieved, reducing bus traffic.
4.  Full-text search queries return relevant D-Bus components with low latency.
5.  Seamless integration with `op-mcp` for dynamic tool generation.
