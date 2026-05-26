# D-Bus ↔ gRPC Bidirectional Bridge: Quality & Integration Audit

## 1. Workspace Dependency Mapping

The workspace `Cargo.toml` lists the `op-grpc-bridge` crate as a workspace dependency:
- **Workspace Declaration**: `op-grpc-bridge = { path = "crates/op-grpc-bridge" }` (workspace `Cargo.toml`)

Based on the provided workspace configuration and dependencies, the following packages depend on `op-grpc-bridge`:
1. **`op-dbus`** (defined in root `Cargo.toml`): Declares `op-grpc-bridge.workspace = true` as a dependency.
2. **`op-chat`** (declared in `Cargo.lock`): Declares a dependency on `op-grpc-bridge`.
3. **`op-dbus-mirror`** (declared in `Cargo.lock`): Declares a dependency on `op-grpc-bridge`.
4. **`op-projection`** (declared in `Cargo.lock`): Declares a dependency on `op-grpc-bridge`.
5. **`op-web`** (declared in `Cargo.lock`): Declares a dependency on `op-grpc-bridge`.

---

## 2. Registered D-Bus Destinations, Object Paths, and Interfaces

The gRPC server acts as a bridge by proxying requests to and from the system D-Bus bus. The following services, object paths, and interfaces are interacted with or registered:

### Dynamic Destinations
- **Destination**: `org.opdbus.{plugin_id}.v1` (where `{plugin_id}` is dynamically supplied by the gRPC client)
  - *Citations*: `crates/op-grpc-bridge/src/grpc_server.rs:562`, `crates/op-grpc-bridge/src/grpc_server.rs:595`, and `crates/op-grpc-bridge/src/schema_engine.rs:275`
  - *Object Paths*: Dynamic path supplied by `req.object_path.as_str()` (`crates/op-grpc-bridge/src/grpc_server.rs:565`, `598`)

### Core Services
- **Destination**: `org.opdbus.v1`
  - *Interface*: `org.opdbus.OvsdbV1`
    - *Object Path*: `/org/opdbus/v1/ovsdb`
    - *Citations*: `crates/op-grpc-bridge/src/grpc_server.rs:755-757`
  - *Interface*: `org.opdbus.MailV1`
    - *Object Path*: `/org/opdbus/v1/mail`
    - *Citations*: `crates/op-grpc-bridge/src/grpc_server.rs:1109-1111`
  - *Interface*: `org.opdbus.PrivacyV1`
    - *Object Path*: `/org/opdbus/v1/privacy`
    - *Citations*: `crates/op-grpc-bridge/src/grpc_server.rs:1391-1393`
  - *Interface*: `org.opdbus.RegistrationV1`
    - *Object Path*: `/org/opdbus/v1/registration`
    - *Citations*: `crates/op-grpc-bridge/src/grpc_server.rs:1968-1970`

### Authoritative State Paths (Schema Engine Watchers)
- *Object Paths*:
  - `/org/opdbus/v1/nonnet/{db_name}/{table_name}` (`crates/op-grpc-bridge/src/schema_engine.rs:143`)
  - `/org/opdbus/v1/ovsdb/{table_name}` (`crates/op-grpc-bridge/src/schema_engine.rs:180`)
  - `/org/opdbus/v1/ovsdb/Bridge/bridge_name` (`crates/op-grpc-bridge/src/schema_engine.rs:291`)
  - `/org/opdbus/v1/mail/outbox/{message_id}` (`crates/op-grpc-bridge/src/grpc_server.rs:1162`)
  - `/org/opdbus/v1/mail/admin_actions/{action_id}` (`crates/op-grpc-bridge/src/grpc_server.rs:1294`)
  - `/org/opdbus/v1/privacy/network` (`crates/op-grpc-bridge/src/grpc_server.rs:1461`)
  - `/org/opdbus/v1/privacy/users/{user_id}` (`crates/op-grpc-bridge/src/grpc_server.rs:1599`)
  - `/org/opdbus/v1/privacy/components/{component}` (`crates/op-grpc-bridge/src/grpc_server.rs:1693`)
  - `/org/opdbus/v1/privacy/routing/{container_name}` (`crates/op-grpc-bridge/src/grpc_server.rs:1867`)
  - `/org/opdbus/v1/registration/magic_links/{token}` (`crates/op-grpc-bridge/src/grpc_server.rs:2026`)
  - `/org/opdbus/v1/registration/users/{user_id}` (`crates/op-grpc-bridge/src/grpc_server.rs:2132`)
  - `/org/opdbus/v1/registration/admin_actions/{uuid}` (`crates/op-grpc-bridge/src/grpc_server.rs:2337`)

---

## 3. Exposed HTTP and gRPC Endpoints

The gRPC server binds to the runtime `SocketAddr` configuration (`crates/op-grpc-bridge/src/grpc_server.rs:235`). It registers and exposes the following gRPC services supporting raw HTTP/2 and gRPC-Web (`tonic_web::enable`):

1. **`StateSync`** (`crates/op-grpc-bridge/src/grpc_server.rs:307`):
   - `Subscribe` (Streams state updates)
   - `Mutate` (Mutates system state)
   - `GetState` (Retrieves full plugin state cache)
   - `BatchMutate` (Executes atomic groups of state mutations)
2. **`PluginService`** (`crates/op-grpc-bridge/src/grpc_server.rs:308`):
   - `ListPlugins` (Lists active plugins)
   - `GetSchema` (Exposes schema definitions)
   - `CallMethod` (Routes D-Bus calls via SchemaEngine)
   - `GetProperty` / `SetProperty` (Accesses properties on the system bus)
   - `SubscribeSignals` (Streams D-Bus signal changes)
3. **`EventChainService`** (`crates/op-grpc-bridge/src/grpc_server.rs:309`):
   - `GetEvents` / `SubscribeEvents` (Real-time immutable ledger query and stream)
   - `VerifyChain` (Validates cryptographic hash continuity)
   - `GetProof` (Generates Merkle proofs of event integrity)
   - `ProveTagImmutability` (Audit tool for proving compliance of tagged namespaces)
   - `GetSnapshot` / `CreateSnapshot` (State serialization backup points)
   - `SearchSemanticTrace` (Semantic trace queries against Qdrant vector spaces)
4. **`OvsdbMirror`** (`crates/op-grpc-bridge/src/grpc_server.rs:312`):
   - RFC 7047-compatible methods (`ListDbs`, `GetSchema`, `Transact`, `Monitor`, `Echo`, `DumpDb`, `GetBridgeState`)
5. **`RuntimeMirror`** (`crates/op-grpc-bridge/src/grpc_server.rs:313`):
   - System telemetry endpoints (`GetSystemInfo`, `ListServices`, `GetService`, `StreamMetrics`, `ListInterfaces`, `GetNumaTopology`)
6. **`ComponentRegistry`** (`crates/op-grpc-bridge/src/grpc_server.rs:314`):
   - MCP Discovery backend (`Register`, `Deregister`, `Discover`, `GetComponent`, `Watch`, `Heartbeat`)
7. **`MailService`** (`crates/op-grpc-bridge/src/grpc_server.rs:317`):
   - Maddy-bridge operations (`SendEmail`, `GetInbox`, `GetMessage`, `GetMailStatus`, `ListMailAccounts`, `AdminMailAction`, `CheckMailServer`)
8. **`PrivacyNetworkService`** (`crates/op-grpc-bridge/src/grpc_server.rs:318`):
   - Privacy router operations (`EnsurePrivacyNetwork`, `GetNetworkStatus`, `ProvisionUser`, `GetPrivacyWireGuardConfig`, `ManageComponent`, `GetNetworkTopology`, `HealthCheck`, `ConfigurePacketRouting`, `GenerateWireGuardKeyPair`)
9. **`RegistrationService`** (`crates/op-grpc-bridge/src/grpc_server.rs:321`):
   - Dynamic identity provisioning (`SendMagicLink`, `VerifyMagicLink`, `RegisterUser`, `GetUserStatus`, `ListUsers`, `GetWireGuardConfig`, `AdminUserAction`)
10. **`McpService`** (`crates/op-grpc-bridge/src/grpc_server.rs:324`):
    - Multi-Agent cache routing protocol
11. **`reflection`** (`crates/op-grpc-bridge/src/grpc_server.rs:325`):
    - Standard gRPC Server Reflection v1
12. **`health_service`** (`crates/op-grpc-bridge/src/grpc_server.rs:326`):
    - Standard gRPC Liveness Probe and Health protocol

---

## 4. Cross-Crate Circular Dependency Risks

An analysis of `crates/op-grpc-bridge/Cargo.toml` and workspace configurations reveals structural circular dependency risks:

1. **Bridge-to-MCP Coupling**: `op-grpc-bridge` depends directly on `op-cognitive-mcp` (path `../op-cognitive-mcp`) to instantiate the `QdrantSemanticShuttle` for the `SearchSemanticTrace` endpoint. If `op-cognitive-mcp` ever requires access to compiled protobuf types, helper functions, or the gRPC client interface defined in `op-grpc-bridge`, a circular dependency compiles error will occur.
2. **Bridge-to-Cache Coupling**: `op-grpc-bridge` imports `op-cache` (path `../op-cache`) to bootstrap the MCP gRPC services (`AgentServiceImpl`, `OrchestratorServiceImpl`, `CacheServiceImpl`) inside the primary server runloop. If a leaf crate like `op-cache` needs to publish events using types generated or exported by `op-grpc-bridge`, it cannot do so without causing a cyclic import.
3. **Ledger-to-Bridge Coupling**: `op-grpc-bridge` depends on `op-state-store`. If `op-state-store` requires any gRPC transmission definitions or bridge-specific models, it faces compilation blocks.

*Recommendation*: Extract protobuf generation outputs and raw domain-agnostic types to a clean, lightweight, separate `op-api-schemas` crate that sits at the bottom of the dependency tree.

---

## 5. Security and Quality Audit Findings

### [CRITICAL] Memory-Mapped Out-of-Bounds Read in Interceptor
- **File & Line**: `crates/op-grpc-bridge/src/interceptor.rs:43-52`
- **Description**: The tonic interceptor maps `/dev/shm/plugin_schema.dat` to memory using `mmap` without performing length verification. It directly casts the pointer to `*const IdentitySled` and dereferences its fields. If the file on disk is truncated to a size smaller than `std::mem::size_of::<IdentitySled>()` (due to corruption, concurrent write, or local attack), dereferencing the pointer leads to an immediate out-of-bounds memory read, resulting in a `SIGBUS` signal or segmentation fault.
- **Impact**: Any local process or malformed state change that truncates `/dev/shm/plugin_schema.dat` can instantly crash the main gRPC service on port 50051 when a new request arrives, resulting in a Denial of Service (DoS).
- **Remediation**:
  ```rust
  let metadata = file.metadata().map_err(|_| Status::internal("Metadata inaccessible"))?;
  if metadata.len() < std::mem::size_of::<IdentitySled>() as u64 {
      return Err(Status::internal("Identity sled file truncated"));
  }
  ```

### [HIGH] Unsafe Concurrent Shared Memory Mutations
- **File & Line**: `crates/op-grpc-bridge/src/interceptor.rs:48-55`
- **Description**: Using memory-mapped files via `mmap` to read structures that are concurrently written to by the `SchemaEngine` without synchronization primitives is undefined behavior in Rust. The compiler assumes data accessed via non-volatile pointers remains immutable. Additionally, concurrent truncation or replacing of the file by the writer while mapped by the reader triggers `SIGBUS` panics.
- **Remediation**: Use atomic types inside the shared memory structure, or enforce flock / POSIX robust read-write locks during file reads and writes.

### [MEDIUM] Arbitrary D-Bus Object Path Traversal and Escape
- **File & Line**: `crates/op-grpc-bridge/src/grpc_server.rs:562-565`, `595-598`
- **Description**: In both the `GetProperty` and `SetProperty` gRPC methods, the client-provided `plugin_id` and `object_path` are passed directly to `PropertiesProxy::builder` without validation. A malicious gRPC client can use injection characters in the `plugin_id` to alter the targeted bus name or supply an unauthorized `object_path` to read or mutate internal system properties.
- **Remediation**: Enforce strict alphanumeric validation on `plugin_id` and restrict `object_path` matching to an allowed prefix whitelist matching only registered services.

### [MEDIUM] Option Injection in Service Management Command
- **File & Line**: `crates/op-grpc-bridge/src/grpc_server.rs:906-909`
- **Description**: In `get_service`, the `service_name` string from the gRPC request is passed directly as an argument to the `dinitctl status` command. If the service name begins with a hyphen (e.g., `-h` or `--help`), it will be processed as a command-line flag by `dinitctl`, resulting in command option injection.
- **Remediation**: Use a double dash separator (`--`) to terminate option parsing:
  ```rust
  let output = tokio::process::Command::new("dinitctl")
      .args(["status", "--", name])
      .output()
      .await?
  ```

### [LOW] Misleading Address Binding Log Message
- **File & Line**: `crates/op-grpc-bridge/src/grpc_server.rs:301`
- **Description**: The gRPC server prints `info!("FORCE BINDING gRPC server to 0.0.0.0:50051")` unconditionally, but the server actually binds to the address provided in `addr` parameter. If the server is configured to bind to a local interface or custom port, this log statement prints incorrect information.
- **Remediation**: Update log output to dynamically capture the true binding socket: `info!(addr = %addr, "FORCE BINDING gRPC server");`

---

## 6. Schema-As-Code Violations

The system is designed to adhere to a schema-as-code discipline using Protocol Buffers and OSCAL. The following locations violate this rule by expressing data contracts as ad-hoc structs, strings, or untyped JSON:

1. **Shared Memory Sled Layout**:
   - `crates/op-grpc-bridge/src/interceptor.rs:18-24`: `IdentitySled` is represented as an ad-hoc C struct, defining cryptographic session mappings without versioning or reflection schemas.
2. **Unstructured Dynamic JSON Serialization**:
   - `crates/op-grpc-bridge/src/grpc_server.rs:1121` (Email operations)
   - `crates/op-grpc-bridge/src/grpc_server.rs:1404` (Privacy network configs)
   - `crates/op-grpc-bridge/src/grpc_server.rs:1980` (Magic link generation parameters)
   - *Violation*: Contracts are expressed as arbitrary, unversioned JSON structures created dynamically using `simd_json::json!({...}).to_string()`, bypassing formal versioned Protobuf messages.
3. **Ad-Hoc Parsing of OVSDB Tables**:
   - `crates/op-grpc-bridge/src/grpc_server.rs:761`: Structure attributes (Bridge, Port, Interface) are extracted dynamically using ad-hoc string indexing (`row.get("ports")`, `row.get("name")`) rather than statically typed schemas.
4. **Ad-Hoc Ledger Mapping Context**:
   - `crates/op-grpc-bridge/src/schema_engine.rs:310`: Writes context directly to the shared memory sled using raw positional environment variables rather than using a compiled OSCAL mapping schema model.