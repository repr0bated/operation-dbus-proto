### Integration Audit Report: `op-services`

---

### Workspace & Dependency Summary

#### 1. Crates Depending on `op-services`
Based on the workspace `Cargo.toml` and `Cargo.lock` sections provided:
*   **None**: There are no workspace crates listing `op-services` as a dependency. `op-services` acts as an independent system daemon and leaf crate containing three binary targets (`op-services`, `systemctl`, and `systemctl-native`).

#### 2. Registered D-Bus Services and Object Paths
The following D-Bus parameters are registered in `crates/op-services/src/dbus/interface.rs`:
*   **Service Name**: `org.opdbus.services` (Requested at `interface.rs:111`)
*   **Object Path**: `/org/opdbus/services` (Exposed at `interface.rs:109`)
*   **Interface Name**: `org.opdbus.services.v1.Manager` (Registered via zbus macro at `interface.rs:22`)

#### 3. Exposed HTTP/gRPC Endpoints
The gRPC interface binds and listens on the following endpoint (configured in `crates/op-services/src/bin/op-services.rs`):
*   **Address**: Configured via the `OP_SERVICES_GRPC_ADDR` environment variable, defaulting to `[::]:50053` (defined at `op-services.rs:40-42`).
*   **Service**: `opdbus.services.v1.ServiceManager` (Tonic server definition generated at `crates/op-services/src/grpc/mod.rs:7`).
*   **Exposed gRPC Methods** (defined in `crates/op-services/src/grpc/server.rs:41-218`):
    *   `Start(StartRequest) -> StartResponse`
    *   `Stop(StopRequest) -> StopResponse`
    *   `Restart(RestartRequest) -> RestartResponse`
    *   `Reload(ReloadRequest) -> ReloadResponse`
    *   `Create(CreateRequest) -> CreateResponse`
    *   `Delete(DeleteRequest) -> DeleteResponse`
    *   `Get(GetRequest) -> GetResponse`
    *   `List(ListRequest) -> ListResponse`
    *   `Enable(EnableRequest) -> EnableResponse`
    *   `Disable(DisableRequest) -> DisableResponse`
    *   `WatchStatus(WatchRequest) -> Stream<ServiceEvent>`

#### 4. Cross-Crate Circular Dependency Risks
*   **No Risk Found**: `op-services` depends on `op-plugins` to resolve internal system schemas (specified in `crates/op-services/Cargo.toml:13`). `op-plugins` does not reference `op-services` in its dependency graph. Communication back from plugins or other system layers to the service manager is performed strictly out-of-process via D-Bus IPC or gRPC network calls, ensuring no compile-time cyclic dependencies can occur.

---

### Critical Findings

#### Finding 1: Unauthenticated Remote Code Execution as Root via Public gRPC Port
*   **File/Line**: `crates/op-services/src/bin/op-services.rs:36-53`, `crates/op-services/src/grpc/server.rs:136-159`
*   **Severity**: Critical
*   **Description**: The gRPC server binds to all network interfaces (`[::]:50053`) by default and uses no TLS configuration, token interceptors, or peer verification mechanisms. Because the `Create` and `Start` methods accept arbitrary command paths and argument vectors, any remote attacker with network access to port 50053 can register a malicious service and execute arbitrary shell commands or binary payloads with elevated privileges (as the `root` user running the service manager).

---

### Security & Quality Findings

#### Finding 2: Fallback Process Manager Silently Ignores Configured User/Group Privileges
*   **File/Line**: `crates/op-services/src/manager/process.rs:35-57`
*   **Severity**: High
*   **Description**: When the service manager falls back to direct process execution (due to the absence of the `dinit-dbus` interface), it spawns child processes using `ProcessManager::start`. However, this function completely ignores the `user` and `group` configurations defined in the `ServiceDef`. Consequently, any unprivileged service that is supposed to run under restricted credentials will silently execute with the high-privilege context of the service manager daemon (typically `root`).

#### Finding 3: Self-Inflicted SIGTERM to Service Manager Process Group on Spawn Failure
*   **File/Line**: `crates/op-services/src/manager/process.rs:49`, `crates/op-services/src/manager/process.rs:65`
*   **Severity**: High
*   **Description**: In `ProcessManager::start`, if a child process fails to return a valid PID, `child.id().unwrap_or(0)` sets the tracking ID to `0`. When `ProcessManager::stop` is subsequently called for this service, it invokes `kill(Pid::from_raw(0), Signal::SIGTERM)`. In POSIX systems, calling `kill` with PID `0` sends the signal to all processes in the caller's process group. This will immediately terminate the service manager daemon itself and all sibling services in its group.

#### Finding 4: Missing Identity Verification on D-Bus System Bus Interface
*   **File/Line**: `crates/op-services/src/dbus/interface.rs:25-96`
*   **Severity**: Medium
*   **Description**: The registered D-Bus methods handle highly privileged system actions (starting, stopping, and restarting services). However, the implementation does not perform any credential checks (e.g., retrieving the caller's UID via `zbus::Connection` or standard Polkit/Policy checks). This forces absolute reliance on external D-Bus configuration policies; any system misconfiguration will allow unprivileged local users to perform administrative service modifications.

#### Finding 5: Silent Dropping of Service Installation Failures
*   **File/Line**: `crates/op-services/src/manager/service_manager.rs:142-152`
*   **Severity**: Low
*   **Description**: In `ServiceManager::create`, if writing the physical service configuration to the `/etc/dinit.d/` directory fails during `service.install()`, the manager logs a warning but still returns `Ok(())`. This tells the caller that creation succeeded, even though the service file was not written to disk and is completely unmanageable by the `dinit` backend.

---

### Schema-as-Code Discipline Violations

#### Finding 6: Ad-hoc JSON-in-String Serialization Over D-Bus IPC
*   **File/Line**: `crates/op-services/src/dbus/interface.rs:34`, `crates/op-services/src/dbus/interface.rs:47`, `crates/op-services/src/dbus/interface.rs:60`, `crates/op-services/src/dbus/interface.rs:73`
*   **Severity**: Style & Quality
*   **Description**: The D-Bus methods (`start`, `stop`, `restart`, `get_status`) return state contracts as raw serialized JSON strings (`serde_json::to_string(&status)`) inside native D-Bus primitive string fields. This circumvents the Schema-as-Code architecture by avoiding both versioned Protobuf contracts and strongly typed zbus/G-Variant structures, introducing parser vulnerability risks and integration friction.