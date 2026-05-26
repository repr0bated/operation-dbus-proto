| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unwrap_expect` | `crates/op-execution-tracker/src/metrics.rs:135` | Implements the infallible `Default` trait by calling `Self::new().expect(...)`, introducing a panic vector if initialization fails. | `Default::default()` should be completely infallible and self-contained; if initialization can fail, fall back to a safe default config or separate the fallible parts from the default constructor. | Infallibility contract violation of `Default` via `.expect()` panic propagation during initialization. | Minor Gap |
| `format_json_manual` | `crates/op-execution-tracker/src/record.rs:354` | Truncates strings using ad-hoc byte slicing `&s[..max_len]` within manual format macros, ignoring UTF-8 character boundaries. | Use robust UTF-8 slicing helpers (`floor_char_boundary` or iterator-based taking) to avoid panics on multi-byte boundaries, and enforce structured schema models (e.g., Protobuf/OSCAL) rather than raw string truncation. | Slicing string bytes directly without verifying char boundaries can panic at runtime. Data contracts are represented via ad-hoc strings instead of a versioned schema. | Major Gap |

---

### Actionable Recommendations

#### For `format_json_manual` (`crates/op-execution-tracker/src/record.rs:354`):

1. **Prevent UTF-8 Boundary Panics during Truncation:**
   Do not slice strings using direct byte indices (`&s[..max_len]`) unless `max_len` is guaranteed to fall on a valid UTF-8 character boundary. Replace the ad-hoc truncation logic with a safe alternative:
   ```rust
   // Safe alternative using char iterator to avoid boundary panics
   let truncated: String = s.chars().take(max_chars).collect();
   if s.chars().count() > max_chars {
       format!("{}... (truncated)", truncated)
   } else {
       s.to_string()
   }
   ```
   Alternatively, if byte-level limits are required, use `floor_char_boundary` (or check `s.is_char_boundary(max_len)`) before slicing:
   ```rust
   let boundary = if s.is_char_boundary(max_len) {
       max_len
   } else {
       // Find nearest lower char boundary
       (0..max_len).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0)
   };
   format!("{}... (truncated)", &s[..boundary])
   ```

2. **Align with Schema-as-Code Discipline:**
   Instead of performing ad-hoc string formatting, truncation, or regex/JSON manipulation on raw execution strings, specify the trace/payload data contract using a versioned schema technology (e.g., Protocol Buffers). 
   - Define execution tracker records in a `.proto` file.
   - Use compiler-generated types to format/serialize structured execution payloads securely.
   - Truncate fields inside specific structured fields within the schema object model rather than modifying raw serialized JSON blobs.