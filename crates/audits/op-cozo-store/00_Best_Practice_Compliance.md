| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-cozo-store/src/lib.rs:536` | Unhandled `DataValue` variants are fallback-serialized to a JSON `Value::String` using standard `Debug` representation formatting (`format!("{other:?}")`). | Explicit, strongly-typed schema mapping or exact variant-to-variant conversion using formal deserialization contracts (e.g., Protocol Buffers, OSCAL, or matching all known variants to their proper JSON equivalents). | Using `Debug` formatting for serialization loses type distinctions (e.g., integers, booleans, and nulls become strings), produces ad-hoc data contracts, and violates the "schema-as-code" discipline. | Major Gap |

### Actionable Recommendations

#### Refactor `dv_to_json` to Use Explicit Mapping and Avoid Debug Fallbacks
At `crates/op-cozo-store/src/lib.rs:536`, instead of using a wildcard pattern with a `Debug` format fallback (`other => Value::String(format!("{other:?}"))`), implement explicit conversions for all known variants of `DataValue`:

1. **Preserve Numeric and Boolean Types:** Map numerical variants (e.g., `DataValue::Int`, `DataValue::Float`) to `Value::Number` and boolean variants (`DataValue::Bool`) to `Value::Bool`.
2. **Handle Nulls Correctly:** Convert `DataValue::Null` directly to `Value::Null` instead of stringifying it as `"Null"`.
3. **Establish Schema-as-Code Contracts:** If a conversion cannot be mapped directly to a standard JSON type, serialize it into a schema-defined representation (such as base64 for bytes or standard RFC 4122 string representation for UUIDs) defined in a versioned schema (e.g., Protocol Buffers). This avoids generating ad-hoc, compiler-dependent `Debug` strings that can break downstream schema consumption.