### 1. Memory Mapping and Sled Usage Audit

An audit of the provided source files was performed to identify usages of `memmap2`, `mmap`, `MmapMut`, `MmapOptions`, and `sled`. 

*   **Direct Memory Maps (`memmap2`)**: No direct usages of `memmap2`, `mmap`, `MmapMut`, or `MmapOptions` were found within the provided `op-agents` source files.
*   **Sled Storage**: While `cozo` (configured with `storage-sled`) is declared in the workspace dependencies (`Cargo.toml`), no direct initialization or usage of `sled` occurs in the provided `op-agents` code.

#### Memory Map Table

| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| None | N/A | N/A | No memory mapping or direct `sled` instantiation is performed in the provided files. |

---

### 2. Large Heap Allocations (> 1MB)

*   **`crates/op-agents/src/security/sandbox.rs:206`**
    *   **Allocation**: `let mut stdout_buf = Vec::with_capacity(max_output.min(1024 * 1024));`
    *   **Detail**: Allocates a heap buffer of up to $1\text{MB}$ ($1,048,576$ bytes) upfront for standard output collection. Under concurrent agent execution, multiple concurrent allocations of this size can cause rapid memory spikes.
*   **`crates/op-agents/src/security/sandbox.rs:207`**
    *   **Allocation**: `let mut stderr_buf = Vec::with_capacity(max_output.min(1024 * 1024));`
    *   **Detail**: Allocates a heap buffer of up to $1\text{MB}$ ($1,048,576$ bytes) upfront for standard error collection.

---

### 3. Unsafe `simd_json` Usage on Non-Padded Buffers

`simd_json` relies on SIMD hardware instructions that read in 32-byte or 64-byte chunks. To prevent out-of-bounds memory access (and resulting segmentation faults), `simd_json` strictly requires input buffers to be padded with `simd_json::PADDING_SIZE` bytes. Using `unsafe { simd_json::from_str(...) }` on a standard, unpadded Rust `String` or slice is highly unsafe.

*   **`crates/op-agents/src/dbus_service.rs:124`** [**CRITICAL**]
    *   **Code**: `let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }...`
    *   **Detail**: `task_json_mut` is a standard unpadded `String` cloned from the `task_json` parameter. Since this parameter is received directly from the D-Bus interface (external user input), any unprivileged local user with access to the session/system bus can pass a specially crafted unpadded JSON payload to cause an out-of-bounds read, triggering a Segmentation Fault (DoS) or memory exposure.
*   **`crates/op-agents/src/generator/template.rs:434`** [**CRITICAL**]
    *   **Code**: `let task: {struct_name}Task = match unsafe {{ simd_json::from_str(&mut task_json) }}...`
    *   **Detail**: The agent code generator produces templates that parse incoming D-Bus payloads (`task_json`) using `unsafe simd_json::from_str` on unpadded strings. This propagates the critical D-Bus exploitable vulnerability to every single auto-generated agent binary.
*   **`crates/op-agents/src/agent_registry.rs:224`** [**HIGH**]
    *   **Code**: `let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }...`
    *   **Detail**: Parses `content` read via `tokio::fs::read_to_string` directly. If an attacker gains write permissions to the agent specifications configuration file, they can craft an unpadded payload to crash the agent manager daemon.
*   **`crates/op-agents/src/agents/orchestration/memory.rs:128`** [**HIGH**]
    *   **Code**: `let value: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut content_mut) }...`
    *   **Detail**: Parses persistent cognitive memory entries loaded from a standard file (`/var/lib/op-dbus/memory_cognitive.json`) using an unpadded string.
*   **`crates/op-agents/src/agents/orchestration/memory.rs:199`** [**HIGH**]
    *   **Code**: `let old_cache: HashMap<String, String> = unsafe { simd_json::from_str(&mut content_mut) }...`
    *   **Detail**: Parses migrated old key-value memory data using an unpadded string.
*   **`crates/op-agents/src/security/validation.rs:188`** [**HIGH**]
    *   **Code**: `unsafe { simd_json::from_str(&mut json_mut) }`
    *   **Detail**: The validation helper itself performs unsafe, unpadded parsing on user input, rendering the security sanitization phase vulnerable to process-crashing exploits.

---

### 4. `OwnedValue.clone()` on Large JSON Payloads

*   **`crates/op-agents/src/router.rs:144`** [**MEDIUM**]
    *   **Code**: `let config = request.get("config").cloned();`
    *   **Detail**: This occurs within the Axum HTTP POST request handler `spawn_agent_handler`. `request` is a `Json<Value>` where `Value` is `simd_json::OwnedValue`. Calling `.cloned()` on an `OwnedValue` performs a deep clone of the entire parsed JSON abstract syntax tree (AST). If an attacker submits a deeply nested or extremely large JSON configuration, this deep clone causes high CPU load, excessive memory allocation, and potential thread starvation.

---

### 5. `Vec::new` / `String::new` Inside Loops Without Pre-allocation

*   **`crates/op-agents/src/agents/orchestration/memory.rs:173`**
    *   **Code**: `let mut entries = Vec::new();`
    *   **Detail**: Instantiated without capacity and subsequently filled within `for (key, entry) in cache.iter()`. As cognitive memory grows, this vector experiences repeated re-allocations and elements copying. It should be initialized using `Vec::with_capacity(cache.len())`.
*   **`crates/op-agents/src/agents/orchestration/memory.rs:182`**
    *   **Code**: `entry.tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(",")`
    *   **Detail**: Allocates a temporary `Vec` inside the loop for every memory entry being serialized, leading to excessive short-lived heap allocations.
*   **`crates/op-agents/src/agents/orchestration/memory.rs:197`**
    *   **Code**: `let entry = MemoryEntry::new(..., vec![]);`
    *   **Detail**: Passes an empty `Vec` allocated via the `vec![]` macro for every single migrated memory record inside the `old_cache` loop.

---

### 6. `format!` in Hot Paths

The following `format!` macro invocations occur inside frequently executed paths such as D-Bus IPC methods or serialization/deserialization loops:

#### D-Bus IPC Path (High-Frequency Method Dispatches)
*   **`crates/op-agents/src/dbus_service.rs:78`**: `format!("org.dbusmcp.Agent.{}", to_pascal_case(agent_type))`
*   **`crates/op-agents/src/dbus_service.rs:84`**: `format!("/org/dbusmcp/Agent/{}", to_pascal_case(agent_type))`
*   **`crates/op-agents/src/dbus_service.rs:126`**: `format!("Invalid task JSON: {}", e)`
*   **`crates/op-agents/src/dbus_service.rs:131`**: `format!("Unsupported operation '{}'. Supported: {:?}", ...)`
*   **`crates/op-agents/src/dbus_service.rs:138`**: `format!("Execution failed: {}", e)`
*   **`crates/op-agents/src/dbus_service.rs:143`**: `format!("Serialization failed: {}", e)`
*   **`crates/op-agents/src/dbus_service.rs:164`**: `format!("Failed to serialize task: {}", e)`

#### Serialization & Memory Queries (Iterative Loops over Memory Collections)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:181`**: `format!(",\"expires_at\":{}", e)` (executed inside a loop over the entire cache)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:184`**: `format!("\"{}\":{{\"value\":\"{}\", ...}}", ...)` (executed inside a loop over the entire cache)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:192`**: `format!("{{{}}}", entries.join(","))`
*   **`crates/op-agents/src/agents/orchestration/memory.rs:251`**: `format!("Recalled (exact): {} = {} (accessed: {} times)", ...)`
*   **`crates/op-agents/src/agents/orchestration/memory.rs:265`**: `format!("{} = {} (accessed: {} times)", ...)` (executed inside a collection loop)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:312`**: `format!("[score: {:.2}] {} = {}", score, k, v)` (executed inside a collection loop)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:333`**: `format!("{} = {}", k, v)` (executed inside a collection loop)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:365`**: `format!(" [tags: {}]", entry.tags.join(", "))` (executed inside a collection loop)
*   **`crates/op-agents/src/agents/orchestration/memory.rs:375`**: `format!("{} = {}{}{}", ...)` (executed inside a collection loop)