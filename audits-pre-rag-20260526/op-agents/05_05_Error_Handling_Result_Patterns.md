### Error Handling Metrics

| Metric | Count |
| :--- | :--- |
| `.unwrap()` | 6 |
| `.expect()` | 0 |
| `.unwrap_or()` | 117 |
| `?` operator | 278 |
| `todo!()` | 0 |
| `unimplemented!()` | 0 |
| `panic!()` | 0 |

---

### First 5 `.unwrap()` Sites

#### 1. `crates/op-agents/src/lib.rs:314`
```rust
let agent = agent.unwrap();
```
* **Context**: Used in the unit test module `tests` to verify that `create_agent` successfully initializes the "memory" agent.
* **Recommendation**: **Panic is acceptable**. In Rust test suites, calling `.unwrap()` is idiomatic and preferred because tests must fail immediately upon unexpected initialization failures.

#### 2. `crates/op-agents/src/agents/orchestration/sequential_thinking.rs:35`
```rust
Ok(simd_json::to_string_pretty(&steps).unwrap())
```
* **Context**: Used to serialize a hardcoded, statically validated JSON payload inside the `SequentialThinkingAgent::analyze` method.
* **Recommendation**: **Result**. Even though serialization of a hardcoded structure is highly unlikely to fail, it is safer to replace this with `.unwrap_or_else(|_| "{}".to_string())` or propagate the error up as a `Result<String, String>` using `map_err` to guarantee that the production service thread never crashes under unexpected JSON library states.

#### 3. `crates/op-agents/src/generator/md_parser.rs:98`
```rust
let yaml_content = captures.get(1).unwrap().as_str();
```
* **Context**: Extracting the captured YAML frontmatter section from an agent's markdown file after regex match confirmation.
* **Recommendation**: **Result**. If the regex capture group structure is ever modified, this `.unwrap()` will panic the parser generator. Replace with `.ok_or_else(|| anyhow::anyhow!("Missing YAML frontmatter capture"))?` to propagate a clean `Result` error.

#### 4. `crates/op-agents/src/generator/md_parser.rs:99`
```rust
let markdown_content = captures.get(2).unwrap().as_str();
```
* **Context**: Extracting the markdown body capture group from the agent's definition file.
* **Recommendation**: **Result**. Similar to Site 3, replace with `.ok_or_else(|| anyhow::anyhow!("Missing markdown body capture"))?` to gracefully fail with an error message instead of crashing the compiler binary.

#### 5. `crates/op-agents/src/unified/registry.rs:31`
```rust
let agents = self.agents.read().unwrap();
```
* **Context**: Acquiring a read lock on the global `UnifiedAgent` registry.
* **Recommendation**: **Result (using Lock Poisoning mitigation)**. See the detailed analysis below.

---

### Lock Poisoning Risk Analysis

The following sites in `crates/op-agents/src/unified/registry.rs` are flagged as critical lock-poisoning risks:

* **`crates/op-agents/src/unified/registry.rs:31`**: `let agents = self.agents.read().unwrap();`
* **`crates/op-agents/src/unified/registry.rs:41`**: `let mut agents = self.agents.write().unwrap();`

#### Risk
The registry utilizes `std::sync::RwLock` for managing loaded agents. If any thread panics while holding a write lock (for example, during lazy-initialization of an agent), the `RwLock` becomes permanently poisoned. Subsequent attempts by other threads to read or write to the registry will receive a `PoisonError`. Because these sites call `.unwrap()`, they will immediately panic. This leads to a total denial-of-service (DoS) of the agent manager process, preventing any agent from being queried.

#### Recommendation
Replace `std::sync::RwLock` with `parking_lot::RwLock`. `parking_lot` is the industry standard for high-performance systems in Rust. It does not implement lock poisoning; instead, if a thread panics while holding a lock, other threads can still access the protected data normally. This avoids the need for `.unwrap()` entirely as `read()` and `write()` return the guard directly instead of a `Result`.

---

### Production Quality & Security Findings

#### 1. Arbitrary Write/Command Execution via SQL Multi-Statement Bypass (Critical)
* **File:Line**: `crates/op-agents/src/agents/database/sql_pro.rs:32`
* **File:Line**: `crates/op-agents/src/agents/database/database_optimizer.rs:32`
* **Vulnerability**:
Both `SqlProAgent` and `DatabaseOptimizerAgent` attempt to enforce a read-only safety boundary by checking if the query starts with the string `SELECT`, `.SCHEMA`, or `.TABLES`:
```rust
let q_upper = q.to_uppercase();
if !q_upper.trim().starts_with("SELECT") { ... }
```
This validation is completely bypassed by multi-statement queries. For example, passing the query below satisfies the `starts_with("SELECT")` check, yet executes the subsequent write command:
```sql
SELECT 1; ATTACH DATABASE '/tmp/malicious.db' AS malicious; ...
```
In `database_optimizer.rs`, the command executed is:
```rust
cmd.arg(format!("EXPLAIN QUERY PLAN {}", q));
```
Because the entire query string `q` is formatted directly into the `EXPLAIN QUERY PLAN` command, a multi-statement query like:
```sql
SELECT 1; DROP TABLE users;
```
will cause `sqlite3` to explain `SELECT 1` and then physically execute `DROP TABLE users;` on the target database, bypassing the read-only audit contract.
* **Remediation**: Never validate query safety using naive string-prefix checks. Use a real SQL parser (such as the `sqlparser` crate) to parse the AST and explicitly verify that only `Query` statements are present, or connect to the SQLite database with read-only connection flags (`SQLITE_OPEN_READONLY`).

#### 2. Arbitrary File Read/Write via Lexical-Only Path Traversal (Critical)
* **File:Line**: `crates/op-agents/src/security/validation.rs:88`
* **Vulnerability**:
The `validate_path` function attempts to restrict file reads/writes to `ALLOWED_DIRECTORIES` (e.g., `/home`, `/tmp`) using a prefix check:
```rust
let path_buf = PathBuf::from(path);
...
let is_allowed = allowed_dirs.iter().any(|allowed| path_buf.starts_with(allowed));
```
Because `PathBuf::from` performs a purely lexical path check, it does not resolve symbolic links on the filesystem. An attacker can create a symbolic link inside an allowed directory (such as `/tmp/leak`) that points to a sensitive system file (such as `/etc/shadow` or `/root/.ssh/id_rsa`). When the path `/tmp/leak` is validated, `starts_with("/tmp")` evaluates to `true`. The command executor (e.g., `cat`) is then spawned with `/tmp/leak` as an argument, resolving the symlink and leaking the host's sensitive data.
* **Remediation**: Use `std::fs::canonicalize` on the path to fully resolve all symbolic links and redundant parent segments before verifying if the path is contained within the allowed directories.

#### 3. Persistent Memory Corruption via JSON Injection (High)
* **File:Line**: `crates/op-agents/src/agents/orchestration/memory.rs:160-176`
* **Vulnerability**:
The `serialize_memory_entries` method manually serializes key-value pairs into a JSON string using raw string formatting:
```rust
let entry_json = format!(
    "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",...}}",
    key, entry.value, ...
);
```
No escaping is performed on `entry.value`. If an attacker stores a memory value containing unescaped double quotes, they can break out of the JSON string structure and inject arbitrary JSON keys, corrupting the memory file or overwriting other persistent configurations on the next reload of the memory agent.
* **Remediation**: Never write custom JSON serialization logic using `format!`. Use a safe serializer like `serde_json::to_string` or `simd_json::to_string` to ensure all fields are properly escaped.