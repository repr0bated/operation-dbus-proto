# Test Audit

### 1. Test Analysis
The codebase contains extensive unit testing suites embedded within the source files using the `#[cfg(test)]` attribute and the `#[test]` / `#[tokio::test]` annotations. No separate integration tests were found in the provided files.

### 2. Test Function Count
There are **36** total test functions implemented across the provided files:
*   `crates/op-cognitive-mcp/src/activity_filter.rs`: **9** tests
*   `crates/op-cognitive-mcp/src/notebooklm.rs`: **1** test
*   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`: **5** tests
*   `crates/op-cognitive-mcp/src/session.rs`: **5** tests
*   `crates/op-cognitive-mcp/src/quota.rs`: **3** tests
*   `crates/op-cognitive-mcp/src/grpc_service.rs`: **3** tests
*   `crates/op-cognitive-mcp/src/gemini_fallback.rs`: **4** tests
*   `crates/op-cognitive-mcp/src/tool_profiles.rs`: **4** tests
*   `crates/op-cognitive-mcp/src/doctor.rs`: **2** tests

### 3. Representative Tests
*   **`test_noise_tag_suppresses`**: `crates/op-cognitive-mcp/src/activity_filter.rs:330`
    Verifies that schemas marked with the `"noise"` tag successfully derive a significance level of `Significance::Noise`.
*   **`should_create_session_with_generated_id`**: `crates/op-cognitive-mcp/src/session.rs:173`
    Asserts that requesting a session with an empty ID correctly generates a new, active UUID session.
*   **`should_allow_queries_within_limit`**: `crates/op-cognitive-mcp/src/quota.rs:103`
    Validates that client requests within the daily limit are correctly tracked and permitted by the quota manager.

### 4. Property-Based Testing and Fuzzing
No property-based tests (e.g., using `proptest` or `quickcheck`) or fuzzing targets (e.g., `cargo-fuzz`) are present in the provided files.

---

# Critical Security Vulnerabilities

### Finding 1: Memory Safety & Undefined Behavior via Unpadded Slice Ingestion in `simd-json`
*   **Severity**: Critical
*   **Path**: `crates/op-cognitive-mcp/src/dbus_interface.rs:59` and `crates/op-cognitive-mcp/src/cognitive_tools.rs:271`

#### Description
The functions `parse_simd` and `serde_to_simd_json` parse JSON strings using `simd_json::from_slice`. `simd-json` is designed for high-performance parsing utilizing SIMD registers (AVX2/SSE4.2/NEON), which read memory in aligned blocks. Because of this, **the `simd-json` parser strictly requires that the input slice be padded with `simd_json::SIMDJSON_PADDING` bytes** of extra buffer allocation. 

In `dbus_interface.rs:59`:
```rust
fn parse_simd(s: &str) -> Result<simd_json::OwnedValue, String> {
    let mut buf = s.as_bytes().to_vec();
    simd_json::from_slice(&mut buf).map_err(|e| e.to_string())
}
```
And in `cognitive_tools.rs:271`:
```rust
fn serde_to_simd_json(v: serde_json::Value) -> Value {
    let s = serde_json::to_string(&v).unwrap_or_default();
    let mut buf = s.into_bytes();
    simd_json::from_slice(&mut buf).unwrap_or(Value::Static(simd_json::StaticNode::Null))
}
```

Neither allocation allocates the required padding bytes. Passing an unpadded slice to `simd_json::from_slice` allows SIMD instructions to perform out-of-bounds reads past the allocated buffer bounds. 

#### Exploitation/Impact
This is directly exploitable via the D-Bus interface `CallTool(s name, s args_json)` which feeds `args_json` directly into `parse_simd`. An attacker on the system D-Bus can send crafted JSON payloads that trigger segmentation faults (Denial of Service) or potentially read adjacent uninitialized heap memory through out-of-bounds register reads.

#### Remediation
Ensure all slices passed to `simd_json::from_slice` are appropriately padded. Use `simd_json::to_padded_bin` or manually extend the capacity of the `Vec` with `SIMDJSON_PADDING` zeros before parsing.

---

### Finding 2: Shared Memory ABI Mismatch & Memory Corruption between `interceptor.rs` and `qdrant_shuttle.rs`
*   **Severity**: Critical
*   **Path**: `crates/op-cognitive-mcp/src/interceptor.rs:5` vs `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:31`

#### Description
There are two completely incompatible `#[repr(C)]` definitions of `IdentitySled` mapping to the same shared memory file (`/dev/shm/plugin_schema.dat`).

**Definition in `qdrant_shuttle.rs:31`:**
```rust
#[repr(C)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32],
}
```
*   **Total Expected Size**: `32 + 8 + 1 + 32 = 73 bytes` (no alignment padding is generated for `hashed_footprint` since arrays of `u8` have an alignment of 1).

**Definition in `interceptor.rs:5`:**
```rust
#[repr(C)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub _pad: [u8; 7],
    pub hashed_footprint: [u8; 32],
    pub schema_uuid: [u8; 16],
    pub subid: [u8; 64],
    pub control_source: [u8; 32],
    pub nextdns_profile: [u8; 16],
}
```
*   **Total Expected Size**: `32 + 8 + 1 + 7 + 32 + 16 + 64 + 32 + 16 = 208 bytes`.

#### Exploitation/Impact
This mismatch causes two distinct critical errors:
1.  **Memory Corruption & Field Displacement**: The offset of `hashed_footprint` differs. In `interceptor.rs`, it is located at offset `48` (due to `_pad`). In `qdrant_shuttle.rs`, it is located at offset `41`. When `qdrant_shuttle` reads `current_trace_context()`, it reads 32 bytes from offset `41` (which actually overlaps with `is_valid` and the padding bytes in the real structure), resulting in a totally corrupted `trace_id`.
2.  **Offset Miscalculation**: `qdrant_shuttle.rs` extracts appended `PluginSchema` JSON bytes by offsetting `size_of::<IdentitySled>()` (which is ~73 or 80 bytes). However, the actual memory layout was structured using `interceptor.rs`'s size of `208` bytes. This forces the JSON parser in `parse_plugin_schema` to parse raw binary footprint, subid, and control source bytes as JSON, crashing the parser and breaking the security tracing loop.

#### Remediation
Consolidate `IdentitySled` into a single, unified definition in a shared internal library dependency. Do not duplicate physical binary layouts using local ad-hoc struct definitions.

---

# Quality and Schema-as-Code Violations

### Finding 3: Data Contract Bypass — Ad-hoc JSON Strings inside Versioned Protobuf Messages
*   **Severity**: Medium
*   **Path**: `crates/op-cognitive-mcp/src/grpc_service.rs:280`, `625`, `700`, `821`

#### Description
Although this crate uses Protocol Buffers for defining the `CognitiveToolService` gRPC interface, several responses bypass the schema-as-code discipline entirely by wrapping raw, ad-hoc JSON payloads in generic string fields rather than specifying typed versioned protobuf structures.

*   `GetNotebookResponse::metadata_json` (line 280): Raw JSON metadata.
*   `GenerateDataTableResponse::data_json` (line 625): Bypasses tabular schemas by sending the entire matrix as a serialized JSON string.
*   `GetHealthResponse::components_json` (line 700): Component health status sent as a dynamically structured JSON string.
*   `GeminiQueryResponse::sections_json` (line 821): Complex hierarchical sections structured using unstructured JSON strings.

#### Impact
This undermines the schema-as-code paradigm. Changes to these dynamic JSON maps cannot be tracked by protobuf code generation tools, exposing clients to runtime parsing failures when internal structures drift.

#### Remediation
Refactor the `.proto` schemas to fully specify these responses. Use protobuf features such as `map<string, string>`, `google.protobuf.Struct`, or dedicated versioned sub-messages instead of sending raw, unvalidated JSON string blocks.

---

### Finding 4: Ad-hoc JSON Strings over D-Bus API Contracts
*   **Severity**: Low
*   **Path**: `crates/op-cognitive-mcp/src/dbus_interface.rs:25`

#### Description
The D-Bus methods `ListTools`, `GetToolSchema`, and `CallTool` pass and return parameters using raw, unvalidated serialized JSON strings (`String`) instead of using native D-Bus marshaling types (such as dictionaries, arrays, or structured structs). 

#### Impact
The lack of formal schemas at the D-Bus serialization layer requires both endpoints to run ad-hoc serialization and deserialization steps (`simd_json::from_slice` and `serde_json::to_string`), complicating validation and expanding the attack surface.

#### Remediation
Declare clean, structured parameter types on the D-Bus interface using `zbus` attributes (e.g., passing native arrays and dict structures instead of a flat string containing JSON).

---

### Finding 5: Uncontrolled Recursion in Custom Glob Matcher
*   **Severity**: Low / Denial of Service
*   **Path**: `crates/op-cognitive-mcp/src/grpc_service.rs:1001`

#### Description
The implementation of `glob_match_inner` uses basic structural recursion to evaluate wildcards:
```rust
fn glob_match_inner(pattern: &[char], name: &[char]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // '*' matches zero or more characters
            glob_match_inner(&pattern[1..], name)
                || (!name.is_empty() && glob_match_inner(pattern, &name[1..]))
        }
        ...
```
This naive backtrack-heavy matching does not use memoization or dynamic programming. If an attacker passes complex path matching rules with numerous consecutive wildcards (e.g., `*a*b*c*d*`), processing moderately sized inputs can consume massive amounts of CPU resources and exhaust the execution stack.

#### Impact
An attacker can trigger thread hanging or stack overflows (Denial of Service) via standard folder discovery actions containing malicious search queries.

#### Remediation
Replace the custom recursive glob matcher with a standard, production-grade crate (e.g. `globset` or the workspace-wide `glob` dependency defined in `Cargo.toml`).