# op-introspection - Tasks

## Phase 1: Core Discovery and Parsing

- [ ] Implement the `ServiceScanner` with support for System and Session buses (`scanner.rs`).
- [ ] Develop the `IntrospectionParser` using `quick-xml` and `zbus_xml` (`parser.rs`).
- [ ] Define JSON-serializable structures for `ObjectInfo`, `InterfaceInfo`, and `MethodInfo`.
- [ ] Add unit tests for XML-to-JSON parsing of complex D-Bus introspection data.
- [ ] Verify basic service listing and targeted object introspection.

## Phase 2: Caching and Service Layer

- [ ] Implement the asynchronous `IntrospectionCache` using `parking_lot` and `tokio` (`cache.rs`).
- [ ] Develop the `IntrospectionService` as a unified high-level API (`lib.rs`).
- [ ] Add support for cache-backed introspection calls to reduce D-Bus traffic.
- [ ] Implement service-level JSON serialization methods (`list_services_json`, `introspect_json`).
- [ ] Add integration tests for the full discovery-to-cache workflow.

## Phase 3: Semantic Indexing (FTS5)

- [ ] Implement the `DbusIndexer` using SQLite's FTS5 full-text search engine (`indexer.rs`).
- [ ] Develop the `IndexerManager` for background discovery and indexing tasks (`indexer_manager.rs`).
- [ ] Add methods for indexing service names, interfaces, methods, and properties.
- [ ] Implement a `Search` API for semantic and keyword-based D-Bus discovery.
- [ ] Verify indexing performance and query accuracy.

## Phase 4: Projections and Hierarchy

- [ ] Implement `DbusProjection` for simplifying and flattening D-Bus interfaces (`projection.rs`).
- [ ] Develop the `Hierarchical` tree view for D-Bus object paths (`hierarchical.rs`).
- [ ] Integrate projection capabilities into the `IntrospectionService`.
- [ ] Add hardware-specific optimizations for parsing/indexing if applicable (`cpu_features.rs`).
- [ ] Conduct final security review of input validation and D-Bus client usage.

## Success Metrics

- [ ] Successful parsing of standard system D-Bus services (systemd, networkmanager, etc.).
- [ ] FTS5 queries return correct results for common D-Bus method names and keywords.
- [ ] Introspection latency reduced by > 50% for cached objects.
- [ ] All structures are fully JSON-serializable and compatible with MCP.
