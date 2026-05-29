use anyhow::Result;
pub use cozo::{DataValue, Num};
use cozo::{DbInstance, NamedRows, ScriptMutability};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

type Params = BTreeMap<String, DataValue>;

/// Run a CozoDB script, converting the non-std Error to anyhow.
fn cozo_run(db: &DbInstance, script: &str, params: Params) -> Result<NamedRows> {
    db.run_script(script, params, ScriptMutability::Mutable)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Verdict returned by the compliance graph evaluation.
#[derive(Debug, Clone)]
pub struct PolicyVerdict {
    pub allow: bool,
    pub reason: String,
}

/// CozoDB graph database shuttle.
///
/// Manages the unified Datalog relations:
///   - `compliance_rule`     — NIST/EU-AI-Act deny rules evaluated before mutations
///   - `subid_registry`      — canonical OSCAL subid taxonomy entries
///   - `graph_node`          — arbitrary identity-graph nodes
///   - `graph_edge`          — directed relationships between nodes
///   - `audit_event`         — append-only compliance audit log
///   - `users`               — wg_pubkey-keyed user identities (no PII)
///   - `sessions`            — session_id → wg_pubkey, with expiry
///   - `memory_namespaces`   — named MCP memory contexts
///   - `memory_entries`      — key/value entries within a namespace
#[derive(Clone)]
pub struct CozoGraphShuttle {
    pub(crate) db: Arc<DbInstance>,
}

impl CozoGraphShuttle {
    pub fn new_in_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", Default::default())
            .map_err(|e| anyhow::anyhow!("failed to create CozoDB in-memory instance: {e}"))?;
        let s = Self { db: Arc::new(db) };
        s.seed_schema()?;
        Ok(s)
    }

    pub fn new_persistent(path: PathBuf) -> Result<Self> {
        let ps = path.to_string_lossy().to_string();
        // "sled" engine: pure-Rust embedded store, no native lib conflicts with rusqlite
        let db = DbInstance::new("sled", &ps, Default::default())
            .map_err(|e| anyhow::anyhow!("failed to open CozoDB at {ps}: {e}"))?;
        let s = Self { db: Arc::new(db) };
        s.seed_schema()?;
        Ok(s)
    }

    pub fn from_env() -> Result<Self> {
        if let Ok(p) = std::env::var("COGNITIVE_MCP_COZO_DB_PATH") {
            Self::new_persistent(PathBuf::from(p))
        } else {
            Self::new_in_memory()
        }
    }

    fn seed_schema(&self) -> Result<()> {
        let relations = [
            // plugin × op → action(Deny/Allow) + reason + control_ref
            r#":create compliance_rule {
                plugin: String, op: String, action: String
                =>
                reason: String default "",
                control_ref: String default "",
                created_at: String default ""
            }"#,
            // canonical OSCAL subid entries
            r#":create subid_registry {
                subid: String
                =>
                category: String default "",
                component_type: String default "",
                subject: String default "",
                verb: String default "",
                facet: String default "",
                version: Int default 1,
                control_source: String default "",
                control_refs: String default "",
                statement_refs: String default "",
                registered_at: String default ""
            }"#,
            // identity-graph nodes
            r#":create graph_node {
                id: String
                =>
                label: String default "",
                props: String default "{}"
            }"#,
            // directed relationships
            r#":create graph_edge {
                src: String, rel: String, dst: String
                =>
                props: String default "{}"
            }"#,
            // append-only audit log
            r#":create audit_event {
                event_id: String
                =>
                subid: String default "",
                plugin_id: String default "",
                operation: String default "",
                actor: String default "",
                verdict: String default "allow",
                reason: String default "",
                control_ref: String default "",
                timestamp: String default ""
            }"#,
            // wg_pubkey-keyed user identities; no email or PII persisted
            r#":create users {
                wg_pubkey: String
                =>
                created_at: String default ""
            }"#,
            // full privacy router user accounts (PII stored per explicit directive)
            r#":create privacy_users {
                id: String
                =>
                email: String default "",
                email_verified: String default "false",
                wg_public_key: String default "",
                wg_private_key_encrypted: String default "",
                assigned_ip: String default "",
                privacy_quota_bytes: Int default 1073741824,
                privacy_quota_used_bytes: Int default 0,
                privacy_container_name: String default "",
                privacy_route_id: String default "",
                privacy_network_connected: String default "false",
                privacy_network_connected_at: String default "",
                google_id: String default "",
                google_email: String default "",
                api_credentials_json: String default "null",
                created_at: String default ""
            }"#,
            // session_id → wg_pubkey with optional expiry (RFC3339)
            r#":create sessions {
                session_id: String
                =>
                wg_pubkey: String default "",
                created_at: String default "",
                expires_at: String default ""
            }"#,
            // named MCP memory contexts (replaces SQLite memory_namespaces)
            r#":create memory_namespaces {
                name: String
                =>
                id: String default "",
                kind: String default "custom",
                description: String default "",
                linked_task_id: String default "",
                linked_cron: String default "",
                metadata: String default "{}",
                created_at: String default "",
                updated_at: String default ""
            }"#,
            // key/value entries within a namespace (replaces SQLite memory_entries)
            r#":create memory_entries {
                namespace: String, key: String
                =>
                id: String default "",
                value: String default "null",
                tags: String default "[]",
                created_at: String default "",
                updated_at: String default "",
                expires_at: String default "",
                access_count: Int default 0,
                last_accessed: String default ""
            }"#,
            // persistent agent identity / personality (Soul Memory)
            r#":create soul_memories {
                agent_id: String
                =>
                identity: String default "",
                personality: String default "",
                traits: String default "{}",
                version: Int default 1,
                created_at: String default "",
                updated_at: String default ""
            }"#,
            // 1:1 binding from agent → owning memory namespace
            r#":create agent_namespace_bindings {
                agent_id: String
                =>
                namespace: String default "",
                created_at: String default "",
                updated_at: String default ""
            }"#,
        ];

        for script in &relations {
            if let Err(e) = cozo_run(&self.db, script, BTreeMap::new()) {
                let msg = e.to_string();
                if !msg.contains("already exists") && !msg.contains("AlreadyExists") {
                    eprintln!("COZO_SCHEMA_ERR: {}", msg);
                    warn!(error = %msg, "CozoDB schema init warning");
                }
            }
        }

        info!("CozoDB schema ready");
        Ok(())
    }

    // ── Raw query ──────────────────────────────────────────────────────────────

    pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
        let p = params.map(json_obj_to_params).unwrap_or_default();
        let rows = cozo_run(&self.db, query, p)
            .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;
        Ok(named_rows_to_json(rows))
    }

    // ── Compliance ─────────────────────────────────────────────────────────────

    /// Evaluate (plugin_id, operation) against all Deny rules in the compliance graph.
    pub fn evaluate_mutation(&self, plugin_id: &str, operation: &str) -> PolicyVerdict {
        let query = r#"
            deny_rule[reason] :=
                *compliance_rule[plugin, op, action, reason, _, _],
                action = 'Deny',
                (plugin = $plugin || plugin = '*'),
                (op = $op || op = '*')
            ?[reason] := deny_rule[reason]
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("plugin".into(), DataValue::Str(plugin_id.into()));
        p.insert("op".into(), DataValue::Str(operation.into()));

        match cozo_run(&self.db, query, p) {
            Ok(rows) if rows.rows.is_empty() => PolicyVerdict {
                allow: true,
                reason: "no deny rule matched".into(),
            },
            Ok(rows) => {
                let reason = rows.rows[0]
                    .first()
                    .and_then(dv_as_str)
                    .unwrap_or("compliance rule violated")
                    .to_string();
                PolicyVerdict {
                    allow: false,
                    reason,
                }
            }
            Err(_) => PolicyVerdict {
                allow: true,
                reason: "compliance graph not seeded".into(),
            },
        }
    }

    pub fn store_compliance_rule(
        &self,
        plugin: &str,
        op: &str,
        action: &str,
        reason: &str,
        control_ref: &str,
    ) -> Result<()> {
        let query = r#"
            ?[plugin, op, action, reason, control_ref, created_at]
                <- [[$plugin, $op, $action, $reason, $control_ref, $ts]]
            :put compliance_rule { plugin, op, action => reason, control_ref, created_at }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("plugin".into(), DataValue::Str(plugin.into()));
        p.insert("op".into(), DataValue::Str(op.into()));
        p.insert("action".into(), DataValue::Str(action.into()));
        p.insert("reason".into(), DataValue::Str(reason.into()));
        p.insert("control_ref".into(), DataValue::Str(control_ref.into()));
        p.insert("ts".into(), DataValue::Str(now_rfc3339().into()));
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("store compliance rule: {e}"))?;
        Ok(())
    }

    // ── Subid registry ─────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn register_subid(
        &self,
        subid: &str,
        category: &str,
        component_type: &str,
        subject: &str,
        verb: &str,
        facet: &str,
        version: u8,
        control_source: &str,
        control_refs: &str,
        statement_refs: &str,
    ) -> Result<()> {
        let query = r#"
            ?[subid, category, component_type, subject, verb, facet, version,
              control_source, control_refs, statement_refs, registered_at]
                <- [[$sid, $cat, $ctype, $subj, $verb, $facet, $ver,
                     $csrc, $crefs, $srefs, $ts]]
            :put subid_registry {
                subid => category, component_type, subject, verb, facet, version,
                         control_source, control_refs, statement_refs, registered_at
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("sid".into(), DataValue::Str(subid.into()));
        p.insert("cat".into(), DataValue::Str(category.into()));
        p.insert("ctype".into(), DataValue::Str(component_type.into()));
        p.insert("subj".into(), DataValue::Str(subject.into()));
        p.insert("verb".into(), DataValue::Str(verb.into()));
        p.insert("facet".into(), DataValue::Str(facet.into()));
        p.insert("ver".into(), dv_int(version as i64));
        p.insert("csrc".into(), DataValue::Str(control_source.into()));
        p.insert("crefs".into(), DataValue::Str(control_refs.into()));
        p.insert("srefs".into(), DataValue::Str(statement_refs.into()));
        p.insert("ts".into(), DataValue::Str(now_rfc3339().into()));
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("register subid: {e}"))?;
        Ok(())
    }

    // ── Graph ──────────────────────────────────────────────────────────────────

    pub fn store_node(&self, id: &str, label: &str, props: Value) -> Result<()> {
        let query = r#"
            ?[id, label, props] <- [[$id, $label, $props]]
            :put graph_node { id => label, props }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        p.insert("label".into(), DataValue::Str(label.into()));
        p.insert("props".into(), DataValue::Str(props.to_string().into()));
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("store graph node: {e}"))?;
        Ok(())
    }

    pub fn store_edge(&self, src: &str, rel: &str, dst: &str, props: Option<Value>) -> Result<()> {
        let query = r#"
            ?[src, rel, dst, props] <- [[$src, $rel, $dst, $props]]
            :put graph_edge { src, rel, dst => props }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("src".into(), DataValue::Str(src.into()));
        p.insert("rel".into(), DataValue::Str(rel.into()));
        p.insert("dst".into(), DataValue::Str(dst.into()));
        p.insert(
            "props".into(),
            DataValue::Str(
                props
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".into())
                    .into(),
            ),
        );
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("store graph edge: {e}"))?;
        Ok(())
    }

    pub fn query_edges_from(&self, src: &str) -> Result<Value> {
        let mut p: Params = BTreeMap::new();
        p.insert("src".into(), DataValue::Str(src.into()));
        let r = cozo_run(
            &self.db,
            "?[src, rel, dst, props] := *graph_edge[src, rel, dst, props], src = $src",
            p,
        )
        .map_err(|e| anyhow::anyhow!("query edges from: {e}"))?;
        Ok(named_rows_to_json(r))
    }

    pub fn query_edges_to(&self, dst: &str) -> Result<Value> {
        let mut p: Params = BTreeMap::new();
        p.insert("dst".into(), DataValue::Str(dst.into()));
        let r = cozo_run(
            &self.db,
            "?[src, rel, dst, props] := *graph_edge[src, rel, dst, props], dst = $dst",
            p,
        )
        .map_err(|e| anyhow::anyhow!("query edges to: {e}"))?;
        Ok(named_rows_to_json(r))
    }

    pub fn query_node(&self, id: &str) -> Result<Value> {
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        let r = cozo_run(
            &self.db,
            "?[id, label, props] := *graph_node[id, label, props], id = $id",
            p,
        )
        .map_err(|e| anyhow::anyhow!("query node: {e}"))?;
        Ok(named_rows_to_json(r))
    }

    /// BFS traversal using CozoDB's native recursive rules.
    pub fn traverse_graph(&self, start_node: &str, max_depth: u32) -> Result<Value> {
        let query = r#"
            reachable[to, d] := to = $start, d = 0
            reachable[to, d] := reachable[from, d0], *graph_edge[from, _, to, _],
                                d = d0 + 1, d <= $max_depth
            ?[node, depth] := reachable[node, depth]
            :order depth
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("start".into(), DataValue::Str(start_node.into()));
        p.insert("max_depth".into(), dv_int(max_depth as i64));
        let r = cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("traverse graph: {e}"))?;
        Ok(named_rows_to_json(r))
    }

    // ── Audit ──────────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn append_audit_event(
        &self,
        event_id: &str,
        subid: &str,
        plugin_id: &str,
        operation: &str,
        actor: &str,
        verdict: bool,
        reason: &str,
        control_ref: &str,
    ) -> Result<()> {
        let query = r#"
            ?[event_id, subid, plugin_id, operation, actor, verdict, reason, control_ref, timestamp]
                <- [[$eid, $subid, $plugin, $op, $actor, $verdict, $reason, $cref, $ts]]
            :put audit_event {
                event_id => subid, plugin_id, operation, actor, verdict, reason, control_ref, timestamp
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("eid".into(), DataValue::Str(event_id.into()));
        p.insert("subid".into(), DataValue::Str(subid.into()));
        p.insert("plugin".into(), DataValue::Str(plugin_id.into()));
        p.insert("op".into(), DataValue::Str(operation.into()));
        p.insert("actor".into(), DataValue::Str(actor.into()));
        p.insert(
            "verdict".into(),
            DataValue::Str(if verdict { "allow" } else { "deny" }.into()),
        );
        p.insert("reason".into(), DataValue::Str(reason.into()));
        p.insert("cref".into(), DataValue::Str(control_ref.into()));
        p.insert("ts".into(), DataValue::Str(now_rfc3339().into()));
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("append audit event: {e}"))?;
        Ok(())
    }

    // ── Users (wg_pubkey-keyed) ────────────────────────────────────────────────

    /// Insert or refresh a user keyed by WireGuard public key. No PII stored.
    pub fn upsert_user(&self, wg_pubkey: &str) -> Result<()> {
        let query = r#"
            ?[wg_pubkey, created_at] <- [[$wg, $ts]]
            :put users { wg_pubkey => created_at }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("wg".into(), DataValue::Str(wg_pubkey.into()));
        p.insert("ts".into(), DataValue::Str(now_rfc3339().into()));
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("upsert user: {e}"))?;
        Ok(())
    }

    /// True if a user row exists for this wg_pubkey.
    pub fn user_exists(&self, wg_pubkey: &str) -> Result<bool> {
        let mut p: Params = BTreeMap::new();
        p.insert("wg".into(), DataValue::Str(wg_pubkey.into()));
        let r = cozo_run(
            &self.db,
            "?[wg_pubkey] := *users[wg_pubkey, _], wg_pubkey = $wg",
            p,
        )
        .map_err(|e| anyhow::anyhow!("user exists: {e}"))?;
        Ok(!r.rows.is_empty())
    }

    // ── Privacy Users (full PII per explicit directive) ─────────────────────────

    pub fn upsert_privacy_user(
        &self,
        id: &str,
        email: &str,
        email_verified: bool,
        wg_public_key: &str,
        wg_private_key_encrypted: &str,
        assigned_ip: &str,
        privacy_quota_bytes: i64,
        privacy_quota_used_bytes: i64,
        privacy_container_name: &str,
        privacy_route_id: &str,
        privacy_network_connected: bool,
        privacy_network_connected_at: &str,
        google_id: &str,
        google_email: &str,
        api_credentials_json: &str,
        created_at: &str,
    ) -> Result<()> {
        let query = r#"
            ?[id, email, email_verified, wg_public_key, wg_private_key_encrypted,
              assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes,
              privacy_container_name, privacy_route_id, privacy_network_connected,
              privacy_network_connected_at, google_id, google_email,
              api_credentials_json, created_at]
                <- [[$id, $email, $ev, $wg, $wg_priv, $ip, $quota, $used,
                     $container, $route, $pnc, $pnc_at, $gid, $gmail,
                     $api_json, $ts]]
            :put privacy_users {
                id => email, email_verified, wg_public_key, wg_private_key_encrypted,
                      assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes,
                      privacy_container_name, privacy_route_id, privacy_network_connected,
                      privacy_network_connected_at, google_id, google_email,
                      api_credentials_json, created_at
            }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        p.insert("email".into(), DataValue::Str(email.into()));
        p.insert("ev".into(), DataValue::Str(email_verified.to_string().into()));
        p.insert("wg".into(), DataValue::Str(wg_public_key.into()));
        p.insert("wg_priv".into(), DataValue::Str(wg_private_key_encrypted.into()));
        p.insert("ip".into(), DataValue::Str(assigned_ip.into()));
        p.insert("quota".into(), dv_int(privacy_quota_bytes));
        p.insert("used".into(), dv_int(privacy_quota_used_bytes));
        p.insert("container".into(), DataValue::Str(privacy_container_name.into()));
        p.insert("route".into(), DataValue::Str(privacy_route_id.into()));
        p.insert("pnc".into(), DataValue::Str(privacy_network_connected.to_string().into()));
        p.insert("pnc_at".into(), DataValue::Str(privacy_network_connected_at.into()));
        p.insert("gid".into(), DataValue::Str(google_id.into()));
        p.insert("gmail".into(), DataValue::Str(google_email.into()));
        p.insert("api_json".into(), DataValue::Str(api_credentials_json.into()));
        p.insert("ts".into(), DataValue::Str(created_at.into()));
        cozo_run(&self.db, query, p)
            .map_err(|e| anyhow::anyhow!("upsert privacy user: {e}"))?;
        Ok(())
    }

    pub fn get_privacy_user(&self, id: &str) -> Result<Option<Vec<DataValue>>> {
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        let r = cozo_run(
            &self.db,
            "?[email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at] := \
             *privacy_users[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at], id = $id",
            p,
        )
        .map_err(|e| anyhow::anyhow!("get privacy user: {e}"))?;
        Ok(r.rows.into_iter().next())
    }

    pub fn get_privacy_user_by_email(&self, email: &str) -> Result<Option<Vec<DataValue>>> {
        let mut p: Params = BTreeMap::new();
        p.insert("email".into(), DataValue::Str(email.into()));
        let r = cozo_run(
            &self.db,
            "?[id, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at] := \
             *privacy_users[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at], email = $email",
            p,
        )
        .map_err(|e| anyhow::anyhow!("get privacy user by email: {e}"))?;
        Ok(r.rows.into_iter().next())
    }

    pub fn get_privacy_user_by_google_id(
        &self,
        google_id: &str,
    ) -> Result<Option<Vec<DataValue>>> {
        let mut p: Params = BTreeMap::new();
        p.insert("gid".into(), DataValue::Str(google_id.into()));
        let r = cozo_run(
            &self.db,
            "?[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_email, \
             api_credentials_json, created_at] := \
             *privacy_users[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at], google_id = $gid",
            p,
        )
        .map_err(|e| anyhow::anyhow!("get privacy user by google id: {e}"))?;
        Ok(r.rows.into_iter().next())
    }

    pub fn list_privacy_users(&self) -> Result<Vec<Vec<DataValue>>> {
        let r = cozo_run(
            &self.db,
            "?[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at] := \
             *privacy_users[id, email, email_verified, wg_public_key, wg_private_key_encrypted, \
             assigned_ip, privacy_quota_bytes, privacy_quota_used_bytes, \
             privacy_container_name, privacy_route_id, privacy_network_connected, \
             privacy_network_connected_at, google_id, google_email, \
             api_credentials_json, created_at]",
            BTreeMap::new(),
        )
        .map_err(|e| anyhow::anyhow!("list privacy users: {e}"))?;
        Ok(r.rows)
    }

    pub fn delete_privacy_user(&self, id: &str) -> Result<()> {
        let mut p: Params = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        cozo_run(
            &self.db,
            "?[id] <- [[$id]] :rm privacy_users { id }",
            p,
        )
        .map_err(|e| anyhow::anyhow!("delete privacy user: {e}"))?;
        Ok(())
    }

    // ── Sessions ───────────────────────────────────────────────────────────────

    /// Bind a session_id to a wg_pubkey. `expires_at` is RFC3339 ("" = no expiry).
    pub fn create_session(
        &self,
        session_id: &str,
        wg_pubkey: &str,
        expires_at: Option<&str>,
    ) -> Result<()> {
        let query = r#"
            ?[session_id, wg_pubkey, created_at, expires_at]
                <- [[$sid, $wg, $now, $exp]]
            :put sessions { session_id => wg_pubkey, created_at, expires_at }
        "#;
        let mut p: Params = BTreeMap::new();
        p.insert("sid".into(), DataValue::Str(session_id.into()));
        p.insert("wg".into(), DataValue::Str(wg_pubkey.into()));
        p.insert("now".into(), DataValue::Str(now_rfc3339().into()));
        p.insert(
            "exp".into(),
            DataValue::Str(expires_at.unwrap_or("").into()),
        );
        cozo_run(&self.db, query, p).map_err(|e| anyhow::anyhow!("create session: {e}"))?;
        Ok(())
    }

    /// Resolve a session_id to (wg_pubkey, created_at, expires_at). None if absent.
    pub fn lookup_session(&self, session_id: &str) -> Result<Option<(String, String, String)>> {
        let mut p: Params = BTreeMap::new();
        p.insert("sid".into(), DataValue::Str(session_id.into()));
        let r = cozo_run(
            &self.db,
            "?[wg_pubkey, created_at, expires_at] := \
             *sessions[sid, wg_pubkey, created_at, expires_at], sid = $sid",
            p,
        )
        .map_err(|e| anyhow::anyhow!("lookup session: {e}"))?;
        if let Some(row) = r.rows.first() {
            let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
            let created = dv_as_str(&row[1]).unwrap_or("").to_string();
            let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
            Ok(Some((wg, created, expires)))
        } else {
            Ok(None)
        }
    }

    /// Delete a session row.
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let mut p: Params = BTreeMap::new();
        p.insert("sid".into(), DataValue::Str(session_id.into()));
        cozo_run(
            &self.db,
            "?[session_id] <- [[$sid]] :rm sessions { session_id }",
            p,
        )
        .map_err(|e| anyhow::anyhow!("delete session: {e}"))?;
        Ok(())
    }

    /// Return a shared handle to the underlying DbInstance for advanced queries.
    pub fn db(&self) -> Arc<DbInstance> {
        self.db.clone()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn dv_int(i: i64) -> DataValue {
    DataValue::Num(cozo::Num::Int(i))
}

fn dv_as_str(dv: &DataValue) -> Option<&str> {
    if let DataValue::Str(s) = dv {
        Some(s.as_str())
    } else {
        None
    }
}

fn json_obj_to_params(v: Value) -> Params {
    let mut map: Params = BTreeMap::new();
    if let Value::Object(obj) = v {
        for (k, val) in obj {
            map.insert(k, json_to_dv(val));
        }
    }
    map
}

fn json_to_dv(v: Value) -> DataValue {
    match v {
        Value::Null => DataValue::Null,
        Value::Bool(b) => DataValue::Bool(b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                dv_int(i)
            } else {
                DataValue::Num(cozo::Num::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        Value::String(s) => DataValue::Str(s.into()),
        Value::Array(arr) => DataValue::List(arr.into_iter().map(json_to_dv).collect()),
        Value::Object(_) => DataValue::Str(v.to_string().into()),
    }
}

pub fn named_rows_to_json(rows: NamedRows) -> Value {
    let headers = &rows.headers;
    let out: Vec<Value> = rows
        .rows
        .iter()
        .map(|row| {
            let obj: serde_json::Map<String, Value> = headers
                .iter()
                .zip(row.iter())
                .map(|(h, dv)| (h.clone(), dv_to_json(dv)))
                .collect();
            Value::Object(obj)
        })
        .collect();
    Value::Array(out)
}

fn dv_to_json(dv: &DataValue) -> Value {
    match dv {
        DataValue::Null => Value::Null,
        DataValue::Bool(b) => Value::Bool(*b),
        DataValue::Num(cozo::Num::Int(i)) => Value::Number((*i).into()),
        DataValue::Num(cozo::Num::Float(f)) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        DataValue::Str(s) => Value::String(s.to_string()),
        DataValue::List(list) => Value::Array(list.iter().map(dv_to_json).collect()),
        other => Value::String(format!("{other:?}")),
    }
}
