# ADDENDUM — answers to your questions about static FDB plumbing

## 1. Does ensure_static_fdb_entries() exist? Which module?

Yes — `crates/op-network/src/unixctl.rs`. All of these are present at current HEAD:

| item | location |
|---|---|
| `pub async fn ensure_static_fdb_entries(entries: &[StaticFdbEntry]) -> usize` | unixctl.rs:231 |
| `pub async fn ensure_static_fdb(entry: &StaticFdbEntry) -> Result<bool>` | unixctl.rs:201 |
| `pub async fn static_fdb_present(bridge: &str, mac: &str) -> Result<bool>` | unixctl.rs:191 |
| `pub fn static_fdb_from_env() -> Result<Vec<StaticFdbEntry>>` (parses `OF_STATIC_FDB`) | unixctl.rs:178 |
| `StaticFdbEntry` type (`bridge:port:vlan:mac`, implements `FromStr`) | same file |

Live env var is confirmed present on the host:
`/etc/op-dbus/network.conf:35` → `OF_STATIC_FDB=ovsbr0:eth0:0:00:00:5e:00:01:0a`

## 2. Is StaticFdbEntry still constructed by the caller of handle_connection?

**NO — you caught a real drift issue.** In current HEAD, `handle_connection`
(controller.rs:248) does NOT take a `static_fdb` parameter:

```rust
async fn handle_connection(
    mut stream: TcpStream,
    flows: Arc<Vec<(String, String, u16)>>,
    static_flows: Arc<Vec<String>>,
    active_conn: Arc<Mutex<Option<mpsc::UnboundedSender<FlowRequest>>>>,
) -> Result<()>
```

The reference commit (`reference/106a0ef6.diff`) was written against an OLDER
revision in which the caller already threaded
`static_fdb: Arc<Vec<StaticFdbEntry>>` through. That plumbing no longer exists.
This is exactly why `git apply --3way` conflicts around hunk 2 — do NOT try to
force-merge it; re-implement the intent on the current signature.

### What the port must do instead (add the plumbing):

1. Add field `static_fdb: Vec<StaticFdbEntry>` to struct `OpenFlowController`
   (controller.rs:454). Populate in `new()` (:466):

   ```rust
   static_fdb: crate::unixctl::static_fdb_from_env().unwrap_or_else(|e| {
       log::warn!("OF controller: OF_STATIC_FDB parse failed, no pins: {e:#}");
       Vec::new()
   }),
   ```

2. In the accept loop of `run()` (~controller.rs:525–540), wrap and clone:

   ```rust
   let static_fdb = Arc::new(self.static_fdb.clone()); // before loop, like flows/static_flows
   let static_fdb = static_fdb.clone();                // per accept iteration
   ```

   and pass as a new argument. The ONLY call site of handle_connection is :540.

3. New signature adds one param:

   ```rust
   async fn handle_connection(
       mut stream: TcpStream,
       flows: Arc<Vec<(String, String, u16)>>,
       static_flows: Arc<Vec<String>>,
       static_fdb: Arc<Vec<StaticFdbEntry>>,
       active_conn: Arc<Mutex<Option<mpsc::UnboundedSender<FlowRequest>>>>,
   ) -> Result<()>
   ```

4. The relocated step-5b block inside handle_connection then calls:

   ```rust
   if !static_fdb.is_empty() {
       let added = crate::unixctl::ensure_static_fdb_entries(&static_fdb).await;
       log::info!("OF controller: {} static FDB pin(s), {} (re)added",
                  static_fdb.len(), added);
   }
   ```

   placed BEFORE the static-flow loop and its barrier.

Import needed in controller.rs: `use crate::unixctl::{ensure_static_fdb_entries, static_fdb_from_env, StaticFdbEntry};`
(check existing imports first; adjust to style).
