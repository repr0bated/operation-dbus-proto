### 1. Unsafe JSON Parsing (`simd_json` / `serde_json` in `unsafe`)

*   **crates/op-agents/src/agent_registry.rs:245**
    ```rust
    let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
    ```
    *Audit:* Using `simd_json::from_str` inside an `unsafe` block mutates the input string buffer `content` to perform in-place parsing. If the parsed structure (`AgentSpec`) contains borrowed references (it doesn't currently, but changing the struct could introduce them), this leads to lifetime invalidation. Moreover, if the string buffer has shared or invalid UTF-8 state, it can trigger undefined behavior.

*   **crates/op-agents/src/dbus_service.rs:135**
    ```rust
    let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }
    ```
    *Audit:* Mutates `task_json_mut` in-place inside `unsafe`. The D-Bus method handler is highly concurrent; if any reference to `task_json_mut` escapes or is aliased, it results in a data race.

*   **crates/op-agents/src/agents/orchestration/memory.rs:141**
    ```rust
    let value: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };
    ```
    *Audit:* Bypasses safety checks during persistent memory recovery. If `content_mut` is corrupted on disk, this safe wrapper can crash or corrupt the allocator state because `simd_json` assumes the input is perfectly formed when running in unsafe mode.

*   **crates/op-agents/src/agents/orchestration/memory.rs:219**
    ```rust
    let old_cache: HashMap<String, String> = unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };
    ```
    *Audit:* Same in-place parsing issue on migration content.

*   **crates/op-agents/src/security/validation.rs:188**
    ```rust
    unsafe { simd_json::from_str(&mut json_mut) }
    ```
    *Audit:* Performing unsafe JSON deserialization within a security validation routine is an anti-pattern. If malicious inputs can exploit `simd_json` parser bugs via unsafe in-place mutation, the security boundary is completely bypassed.

*   **crates/op-agents/src/generator/template.rs:360**
    ```rust
    let task: {struct_name}Task = match unsafe {{ simd_json::from_str(&mut task_json) }}
    ```
    *Audit:* Auto-generated code contains raw `unsafe` blocks. This delegates the unsafe burden to downstream binaries compiled from this template generator.

---

### 2. Memory Allocations & Hot-Path Performance Issues

*   **crates/op-agents/src/agent_catalog.rs:81-164**
    ```rust
    Box::new(BashProAgent::new(agent_id.clone())),
    Box::new(CProAgent::new(agent_id.clone())),
    ...
    ```
    *Audit:* To build the built-in catalog, the code instantiates over 70 dynamic agents on the heap using `Box::new`, cloning `agent_id` (a heap-allocated `String`) for every single agent. This causes ~70 redundant allocations every time `builtin_agent_descriptors()` is invoked.

*   **crates/op-agents/src/agents/base.rs:336**
    ```rust
    $ops.iter().map(|s| s.to_string()).collect()
    ```
    *Audit:* Implemented inside the macro `impl_agent_common!`. Every time `agent.operations()` is called, it allocates a brand new `Vec<String>` and converts every slice to an owned `String`. These operation queries are on the hot path for task routing.

*   **crates/op-agents/src/agents/orchestration/memory.rs:183**
    ```rust
    fn serialize_memory_entries(cache: &HashMap<String, MemoryEntry>) -> Result<String, String> {
        let mut entries = Vec::new();
        for (key, entry) in cache.iter() {
            ...
            let entry_json = format!(
                "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
                key, entry.value, ...
            );
            entries.push(entry_json);
        }
        Ok(format!("{{{}}}", entries.join(",")))
    }
    ```
    *Audit:* Extreme allocation bottleneck. This manually serializes database entries to JSON by calling `format!` for *every* entry inside a loop, collecting them into a `Vec<String>`, and then calling `join(",")` followed by another `format!`. This creates $O(N)$ intermediate string allocations and should use `serde_json` with a buffered writer.

*   **crates/op-agents/src/agents/base.rs:137**
    ```rust
    let data = format!("stdout:\n{}\n\nstderr:\n{}", result.stdout, result.stderr);
    ```
    *Audit:* Allocates a new formatted string containing the entire stdout/stderr buffer on every single process execution result.

*   **crates/op-agents/src/dbus_service.rs:103**
    ```rust
    pub fn service_name(agent_type: &str) -> String {
        format!("org.dbusmcp.Agent.{}", to_pascal_case(agent_type))
    }
    ```
    *Audit:* Allocates a new string on every property and method call resolution by formatting the bus name dynamically.

*   **crates/op-agents/src/dbus_service.rs:136**
    ```rust
    let mut task_json_mut = task_json.to_string();
    ```
    *Audit:* Clones the entire incoming JSON task string on every single invocation of `execute`, producing a completely redundant heap allocation.

*   **crates/op-agents/src/security/validation.rs:69**
    ```rust
    for c in input.chars() {
        if FORBIDDEN_CHARS.contains(&c) {
            return Err(ValidationError::ForbiddenCharacter(c));
        }
    }
    ```
    *Audit:* A linear scan `FORBIDDEN_CHARS.contains(&c)` inside a loop over every character of a large input string has $O(N \cdot M)$ complexity. It should use a pre-compiled bitset lookup table or `u128` bit mask to perform $O(1)$ validation.

---

### 3. Clone Abuse & Structural Inefficiencies

*   **crates/op-agents/src/agent_registry.rs:296**
    ```rust
    let spec = specs
        .get(agent_type)
        .ok_or_else(|| anyhow::anyhow!("Unknown agent type: {}", agent_type))?
        .clone();
    ```
    *Audit:* Clones the entire `AgentSpec` struct (which contains deeply nested `Vec<String>`, `HashMap<String, String>`, and `Option` blocks) just to verify execution limits and hand it to the factory.

*   **crates/op-agents/src/agent_registry.rs:408**
    ```rust
    instances.values().cloned().collect()
    ```
    *Audit:* Clones every single active `AgentInstance` when querying the list of registry instances. This gets slower linearly as active agents scale.

*   **crates/op-agents/src/agent_registry.rs:420**
    ```rust
    specs.get(agent_type).cloned()
    ```
    *Audit:* Unnecessary clone of the entire `AgentSpec` on every metadata query.

*   **crates/op-agents/src/agents/base.rs:188**
    ```rust
    let executor = SandboxExecutor::new(profile.clone());
    ```
    *Audit:* Clones `SecurityProfile` (including its nested fields, set configurations, and paths) every time a new `AgentContext` is instantiated.

*   **crates/op-agents/src/unified/registry.rs:92**
    ```rust
    self.factories.keys()
        .filter_map(|id| {
            self.get(id).map(|agent| agent.metadata())
        })
        .collect()
    ```
    *Audit:* Deep clone chain inside `all_metadata()`: calling `get(id)` clones the lazy-loaded atomic reference, and `.metadata()` constructs and serializes fresh objects on every call.