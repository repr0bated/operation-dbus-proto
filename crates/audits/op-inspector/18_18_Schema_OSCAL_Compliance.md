## Schema-as-Code Audit

The following table lists data contracts identified in the `op-inspector` crate that are expressed as ad-hoc Rust structs or raw dynamic JSON values instead of versioned schemas (such as Protocol Buffers) or documented OSCAL definitions:

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `CliSchema` | Struct | `crates/op-inspector/src/cli.rs:34` | No | Defined as an ad-hoc Rust struct with `serde` serialization instead of a versioned schema. |
| `CliCommand` | Struct | `crates/op-inspector/src/cli.rs:46` | No | Recursive command hierarchy contract defined directly in Rust code. |
| `CliFlag` | Struct | `crates/op-inspector/src/cli.rs:76` | No | Flag configuration contract defined directly in Rust code. |
| `CliArg` | Struct | `crates/op-inspector/src/cli.rs:95` | No | Argument metadata contract defined directly in Rust code. |
| `ImportedObject` | Struct | `crates/op-inspector/src/datadump.rs:52` | No | Ad-hoc serialization. Uses `simd_json::OwnedValue` (`Value`) for the raw `data` payload (line 62), bypassing type-safety. |
| `GCloudSchema` | Struct | `crates/op-inspector/src/gcloud.rs:37` | No | Specialized CLI introspection schema defined as an ad-hoc Rust struct. |
| `SchemaDefinition` | Struct | `crates/op-inspector/src/introspective_gadget.rs:45` | No | Defines knowledge base schema storage using a dynamic, untyped `Value` (line 51). |
| `ObjectSchema` | Struct | `crates/op-inspector/src/introspective_gadget.rs:470` | No | Hand-rolled schema generation representation instead of using standard schemas like JSON Schema drafts or protobuf descriptors. |
| `ContainerInspection` | Struct | `crates/op-inspector/src/introspective_gadget.rs:531` | No | Docker inspection contract mapping. Uses untyped `Value` for config and network settings (lines 540-541). |

---

## OSCAL Coverage Audit

The following table maps critical security control areas implemented within the `op-inspector` crate against NIST SP 800-53 security controls, identifying where machine-readable OSCAL mappings or system security plans (SSPs) are absent:

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System and Information Integrity (SI-7)** | `crates/op-inspector/src/cli.rs:188` | None | Introspection parses arbitrary local binary help outputs without establishing file/binary integrity controls or signing verification. |
| **Least Privilege / Boundary Protection (AC-3 / SC-7)** | `crates/op-inspector/src/datadump.rs:154` | None | Invokes external system commands dynamically parsed from untrusted outputs without role-based access checks or privilege limitations documented in an OSCAL Component Definition. |
| **Configuration Management (CM-7 / CM-8)** | `crates/op-inspector/src/introspective_gadget.rs:263` | None | Executes dynamic system queries (`docker inspect`) and captures full environmental settings without configuration auditing or mapping to OSCAL assessment logs. |

---

## Detailed Findings & Quality Gaps

### 1. Undefined Behavior & Memory Corruption via Unpadded `simd_json::from_str`
- **Severity**: Critical
- **Citations**: 
  - `crates/op-inspector/src/introspective_gadget.rs:271`
  - `crates/op-inspector/src/introspective_gadget.rs:658`
  - `crates/op-inspector/src/introspective_gadget.rs:719`
- **Description**: The codebase invokes `simd_json::from_str` within unsafe blocks on standard, unpadded Rust `String` instances (e.g., `&mut inspect_json`, `&mut data_mut`, and `&mut json_str`). 
- **Exploitability**: Directly exploitable. `simd-json` utilizes advanced vector instructions (AVX2/SSE) and strictly requires the input buffer to be padded with `simd_json::SIMDJSON_PADDING` bytes (typically 32 or 64 bytes) to prevent out-of-bounds memory reads. When parsing an unpadded string whose length does not align with vector register widths, the parser reads past the end of the allocated heap memory, causing a segmentation fault or memory corruption.
- **Remediation**:
  1. Replace the unsafe `simd_json::from_str` with `simd_json::from_slice` after converting the string to a padded buffer using `simd_json::to_padded_bin`.
  2. Alternatively, utilize `serde_json::from_str` which does not require special memory padding.

```rust
// Remediation Example
let padded_bytes = simd_json::to_padded_bin(&inspect_json);
let container_data: Value = simd_json::from_slice(&mut padded_bytes)
    .context("Failed to safely parse padded JSON")?;
```

---

### 2. Algorithmic Complexity CPU Denial of Service ($O(N^2)$) in Binary Analysis
- **Severity**: High
- **Citations**:
  - `crates/op-inspector/src/introspective_gadget.rs:372`
- **Description**: The function `analyze_binary_patterns` uses a nested loop to identify repeating patterns in raw binary arrays:
  ```rust
  for i in 0..data.len().saturating_sub(8) {
      ...
      while let Some(found) = data[pos..].windows(8).position(|w| w == pattern) { ... }
  }
  ```
- **Exploitability**: Exploitable via Denial of Service. An attacker submitting a modest binary payload (e.g., a 5MB legacy file) will trigger up to $5 \times 10^{12}$ comparisons. Because this synchronous function executes directly on the Tokio thread pool without being offloaded, it blocks the runtime thread indefinitely, starving other asynchronous tasks and freezing the system.
- **Remediation**: 
  1. Replace the brute-force nested lookup with a linear-time Suffix Tree, Lempel-Ziv windowing, or a hash-map-based frequency analysis of 8-byte chunks.
  2. If raw compute is required, execute the synchronous parsing routine inside a `tokio::task::spawn_blocking` block to prevent blocking the async runtime.

---

### 3. Data Integrity Corruption of Legacy Binary Files
- **Severity**: Medium
- **Citations**:
  - `crates/op-inspector/src/introspective_gadget.rs:311`
- **Description**: In `inspect_legacy_data`, the raw legacy binary slice (`&[u8]`) is converted into a string using `String::from_utf8_lossy(data).to_string()` before passing it to `InspectionInput`. 
- **Exploitability**: High impact on functionality. `from_utf8_lossy` replaces any byte sequence that is not valid UTF-8 with the Unicode replacement character `` (U+FFFD). When the binary parser later calls `.as_bytes()`, the original non-ASCII or corrupt bytes are gone and replaced with `EF BF BD`. This makes it impossible to correctly analyze, decode, or reverse-engineer legacy formats such as Apple Lisa disk images.
- **Remediation**: 
  Modify the `InspectionInput` struct and the `InspectionSource::RawData` enum to hold a binary vector (`Vec<u8>`) or a Base64-encoded string instead of raw Unicode strings.

---

### 4. Arbitrary Binary Execution via Untrusted Tool Introspection
- **Severity**: High
- **Citations**:
  - `crates/op-inspector/src/cli.rs:141`
  - `crates/op-inspector/src/cli.rs:538`
- **Description**: `CliParser` accepts an arbitrary `program` string and directly executes it via `tokio::process::Command::new(&self.program)`.
- **Exploitability**: If this parser is exposed via a dynamic plugin manager or user-controlled input parameter, an attacker can specify arbitrary system binaries (e.g., `/bin/sh` or a downloaded backdoor binary) instead of standard utilities like `incus` or `gcloud`.
- **Remediation**: 
  Implement an allowlist of authorized system executables. Reject any binary paths that contain path separators (`/`, `\`) or do not match the expected set of system administrative tools.

---

### 5. Excessive Regular Expression Compilation Bottlenecks
- **Severity**: Low (Performance Anti-pattern)
- **Citations**:
  - `crates/op-inspector/src/cli.rs:252`
  - `crates/op-inspector/src/cli.rs:331`
  - `crates/op-inspector/src/introspective_gadget.rs:327`
- **Description**: Regular expressions (e.g., `cmd_name_re`, `long_flag_re`, `default_re`, `xmlns_re`) are re-compiled on every call to `parse_commands_section`, `parse_flags_section`, and XML inspection methods. Regex compilation in Rust is highly compute-intensive.
- **Remediation**: 
  Utilize `once_cell::sync::Lazy` or `std::sync::OnceLock` to compile the regular expressions exactly once globally.

```rust
// Remediation Example
use std::sync::OnceLock;
static CMD_NAME_RE: OnceLock<Regex> = OnceLock::new();
let re = CMD_NAME_RE.get_or_init(|| Regex::new(r"^\s{2,8}(\w[\w-]*)\s{2,}(.*)$").unwrap());
```