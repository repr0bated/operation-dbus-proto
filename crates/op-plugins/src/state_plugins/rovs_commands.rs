//! rovs_commands Plugin — full RFC 7047 OVSDB protocol surface.
//!
//! This plugin exposes the OVSDB management protocol as typed D-Bus/gRPC methods.
//! Every method input/output is derived from `schemars::JsonSchema`, making the
//! schema the single source of truth for the control contract.
//!
//! High-level convenience helpers (create_bridge, add_port, etc.) are preserved
//! as thin wrappers that compile down to OVSDB `transact` operations.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{PluginSchema, SideEffect};
use rovs_ovsdb::{Client, ClientConfig, Transaction};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

// =============================================================================
// RFC 7047 §3.2 — OVSDB value types
// =============================================================================

/// An OVSDB `<atom>`: the scalar values that can appear in a datum.
#[derive(Debug, Clone, JsonSchema)]
pub enum OvsdbAtom {
    /// 64-bit signed integer.
    Integer(i64),
    /// Floating-point value.
    Real(f64),
    /// Boolean value.
    Boolean(bool),
    /// UTF-8 string.
    String(String),
    /// RFC 7047 "uuid" tuple: `["uuid", "<uuid>"]`.
    Uuid(#[schemars(with = "String")] Uuid),
    /// RFC 7047 "named-uuid" tuple: `["named-uuid", "<name>"]`.
    NamedUuid(String),
    /// RFC 7047 "void" sentinel.
    Void,
}

impl Serialize for OvsdbAtom {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            OvsdbAtom::Integer(v) => v.serialize(serializer),
            OvsdbAtom::Real(v) => v.serialize(serializer),
            OvsdbAtom::Boolean(v) => v.serialize(serializer),
            OvsdbAtom::String(v) => v.serialize(serializer),
            OvsdbAtom::Uuid(v) => ["uuid", &v.to_string()].serialize(serializer),
            OvsdbAtom::NamedUuid(name) => ["named-uuid", name].serialize(serializer),
            OvsdbAtom::Void => serde_json::Value::Null.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OvsdbAtom {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Bool(v) => Ok(OvsdbAtom::Boolean(v)),
            serde_json::Value::Number(n) => {
                if let Some(v) = n.as_i64() {
                    Ok(OvsdbAtom::Integer(v))
                } else if let Some(v) = n.as_f64() {
                    Ok(OvsdbAtom::Real(v))
                } else {
                    Err(serde::de::Error::custom("unsupported number"))
                }
            }
            serde_json::Value::String(s) => Ok(OvsdbAtom::String(s)),
            serde_json::Value::Null => Ok(OvsdbAtom::Void),
            serde_json::Value::Array(arr) => match arr.as_slice() {
                [serde_json::Value::String(tag), serde_json::Value::String(v)] if tag == "uuid" => {
                    v.parse::<Uuid>()
                        .map(OvsdbAtom::Uuid)
                        .map_err(|e| serde::de::Error::custom(format!("invalid uuid: {e}")))
                }
                [serde_json::Value::String(tag), serde_json::Value::String(name)]
                    if tag == "named-uuid" =>
                {
                    Ok(OvsdbAtom::NamedUuid(name.clone()))
                }
                _ => Err(serde::de::Error::custom("unknown atom tuple")),
            },
            serde_json::Value::Object(_) => Err(serde::de::Error::custom("atom cannot be object")),
        }
    }
}

/// An OVSDB `<datum>`: either a single atom, a set of atoms, or a map of atoms.
#[derive(Debug, Clone, JsonSchema)]
pub enum OvsdbDatum {
    Atom(OvsdbAtom),
    Set(Vec<OvsdbAtom>),
    Map(Vec<(OvsdbAtom, OvsdbAtom)>),
}

impl Serialize for OvsdbDatum {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            OvsdbDatum::Atom(atom) => atom.serialize(serializer),
            OvsdbDatum::Set(items) => ("set", items).serialize(serializer),
            OvsdbDatum::Map(items) => ("map", items).serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OvsdbDatum {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Array(arr) => match arr.as_slice() {
                [serde_json::Value::String(tag), payload] if tag == "set" => {
                    let items: Vec<OvsdbAtom> = serde_json::from_value(payload.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(OvsdbDatum::Set(items))
                }
                [serde_json::Value::String(tag), payload] if tag == "map" => {
                    let items: Vec<(OvsdbAtom, OvsdbAtom)> =
                        serde_json::from_value(payload.clone())
                            .map_err(serde::de::Error::custom)?;
                    Ok(OvsdbDatum::Map(items))
                }
                _ => Err(serde::de::Error::custom("unknown datum tuple")),
            },
            _ => {
                let atom: OvsdbAtom =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(OvsdbDatum::Atom(atom))
            }
        }
    }
}

// =============================================================================
// RFC 7047 §3.2 — Database schema representation
// =============================================================================

/// OVSDB database schema returned by `get_schema`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbDbSchema {
    /// Database name, e.g. "Open_vSwitch".
    pub name: String,
    /// Schema version string.
    pub version: String,
    /// Table definitions keyed by table name.
    pub tables: BTreeMap<String, OvsdbTableSchema>,
}

/// OVSDB table schema.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbTableSchema {
    /// Columns keyed by column name.
    pub columns: BTreeMap<String, OvsdbColumnSchema>,
    /// Whether this table is a root table.
    #[serde(rename = "isRoot", default)]
    pub is_root: bool,
    /// Maximum rows allowed (0 means unlimited).
    #[serde(rename = "maxRows", default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<u64>,
    /// Uniqueness indexes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexes: Option<Vec<Vec<String>>>,
}

/// OVSDB column schema.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbColumnSchema {
    /// Column type definition.
    #[serde(rename = "type")]
    pub col_type: OvsdbColumnType,
    /// Whether the column is mutable.
    #[serde(default = "default_true")]
    pub mutable: bool,
    /// Whether the column is ephemeral.
    #[serde(default)]
    pub ephemeral: bool,
}

fn default_true() -> bool {
    true
}

/// OVSDB column type: either a simple atomic string or a complex type.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OvsdbColumnType {
    /// Simple atomic type name, e.g. "string" or "integer".
    Atomic(String),
    /// Complex type definition with key/value/min/max.
    Complex(OvsdbComplexType),
}

/// OVSDB complex column type definition.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbComplexType {
    /// Key or value type specification.
    pub key: OvsdbTypeSpec,
    /// Value type for maps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<OvsdbTypeSpec>,
    /// Minimum number of values (0 for optional, 1 for required).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<u64>,
    /// Maximum number of values (1 for scalar, "unlimited" for sets/maps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<OvsdbMaxValue>,
}

/// OVSDB type specification: simple name or constrained.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OvsdbTypeSpec {
    Simple(String),
    Constrained(OvsdbConstrainedType),
}

/// OVSDB constrained type with references, enums, and length/integer bounds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbConstrainedType {
    /// Base type name.
    #[serde(rename = "type")]
    pub base_type: String,
    /// Reference table for uuid types.
    #[serde(rename = "refTable", default, skip_serializing_if = "Option::is_none")]
    pub ref_table: Option<String>,
    /// Reference strength ("strong" or "weak").
    #[serde(rename = "refType", default, skip_serializing_if = "Option::is_none")]
    pub ref_type: Option<String>,
    /// Allowed enum values.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<serde_json::Value>,
    /// Minimum integer value.
    #[serde(
        rename = "minInteger",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_integer: Option<i64>,
    /// Maximum integer value.
    #[serde(
        rename = "maxInteger",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_integer: Option<i64>,
    /// Minimum string length.
    #[serde(rename = "minLength", default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    /// Maximum string length.
    #[serde(rename = "maxLength", default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
}

/// OVSDB maximum value: either "unlimited" or a concrete limit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OvsdbMaxValue {
    Unlimited(String),
    Limit(u64),
}

// =============================================================================
// RFC 7047 §4.1 — Wire protocol request/response envelopes
// =============================================================================

/// Request body for `list_dbs`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListDbsRequest {}

/// Response body for `list_dbs`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListDbsResponse {
    pub databases: Vec<String>,
}

/// Request body for `get_schema`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetSchemaRequest {
    pub database: String,
}

/// Response body for `get_schema`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetSchemaResponse {
    pub schema: OvsdbDbSchema,
}

/// Request body for `transact`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TransactRequest {
    pub database: String,
    pub operations: Vec<OvsdbOperation>,
}

/// Response body for `transact`: one result per operation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TransactResponse {
    pub results: Vec<OvsdbOperationResult>,
}

/// Request body for `monitor`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorRequest {
    pub database: String,
    /// Value used by the server to tag update notifications.
    pub monitor_id: String,
    /// Per-table monitor requests keyed by table name.
    pub monitor_requests: BTreeMap<String, OvsdbMonitorRequestSpec>,
}

/// Per-table monitor request specification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbMonitorRequestSpec {
    /// Columns to include. Empty or omitted means all columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// Row selection filter: `[column, op, value]` tuples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<OvsdbSelectWhere>,
}

/// Response body for `monitor`: initial table rows.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorResponse {
    /// Table rows keyed by table name, then row UUID.
    pub tables: BTreeMap<String, BTreeMap<String, BTreeMap<String, OvsdbDatum>>>,
}

/// Request body for `monitor_cond`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorCondRequest {
    pub database: String,
    pub monitor_id: String,
    pub monitor_cond_requests: BTreeMap<String, OvsdbMonitorCondRequestSpec>,
}

/// Per-table conditional monitor request specification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbMonitorCondRequestSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub where_clause: Option<OvsdbSelectWhere>,
    /// Monitor protocol version (0, 1, 2, or 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<OvsdbMonitorSelect>,
}

/// Monitor select options (used by monitor_cond_since / monitor_cond).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OvsdbMonitorSelect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modify: Option<bool>,
}

/// Response body for `monitor_cond`.
pub type MonitorCondResponse = MonitorResponse;

/// Request body for `lock` / `steal` / `unlock`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LockRequest {
    pub lock_id: String,
}

/// Response body for `lock` / `steal` / `unlock`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LockResponse {
    pub locked: bool,
}

/// Request body for `echo`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EchoRequest {
    pub params: Vec<OvsdbDatum>,
}

/// Response body for `echo`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EchoResponse {
    pub params: Vec<OvsdbDatum>,
}

/// Request body for `cancel`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CancelRequest {
    /// ID of the in-flight request to cancel.
    pub id: String,
}

/// Response body for `cancel`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CancelResponse {
    pub cancelled: bool,
}

// =============================================================================
// RFC 7047 §5.2 — Database operations
// =============================================================================

/// A single OVSDB database operation inside a `transact` request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum OvsdbOperation {
    /// Insert a new row.
    Insert {
        table: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row: Option<BTreeMap<String, OvsdbDatum>>,
        #[serde(
            rename = "uuid-name",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        uuid_name: Option<String>,
    },
    /// Select rows matching a condition.
    Select {
        table: String,
        #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
        where_clause: Option<OvsdbSelectWhere>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
    },
    /// Update rows matching a condition.
    Update {
        table: String,
        #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
        where_clause: Option<OvsdbSelectWhere>,
        row: BTreeMap<String, OvsdbDatum>,
    },
    /// Mutate rows matching a condition.
    Mutate {
        table: String,
        #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
        where_clause: Option<OvsdbSelectWhere>,
        mutations: Vec<OvsdbMutation>,
    },
    /// Delete rows matching a condition.
    Delete {
        table: String,
        #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
        where_clause: Option<OvsdbSelectWhere>,
    },
    /// Wait until a condition holds or does not hold.
    Wait {
        table: String,
        #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
        where_clause: Option<OvsdbSelectWhere>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
        timeout: i64,
        until: String,
    },
    /// Commit the transaction.
    Commit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        durable: Option<bool>,
    },
    /// Abort the transaction.
    Abort,
    /// Comment (no-op, for audit).
    Comment { comment: String },
    /// Assert that a lock is held.
    Assert { lock: String },
}

/// Where clause: conjunction of `[column, function, value]` tuples.
pub type OvsdbSelectWhere = Vec<OvsdbCondition>;

/// A single OVSDB condition: `[column, function, value]`.
#[derive(Debug, Clone, JsonSchema)]
pub struct OvsdbCondition {
    pub column: String,
    pub function: OvsdbConditionFunction,
    pub value: OvsdbDatum,
}

impl Serialize for OvsdbCondition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.column, &self.function, &self.value).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OvsdbCondition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arr: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
        if arr.len() != 3 {
            return Err(serde::de::Error::custom(
                "condition must be [column, function, value]",
            ));
        }
        let column: String =
            serde_json::from_value(arr[0].clone()).map_err(serde::de::Error::custom)?;
        let function: OvsdbConditionFunction =
            serde_json::from_value(arr[1].clone()).map_err(serde::de::Error::custom)?;
        let value: OvsdbDatum =
            serde_json::from_value(arr[2].clone()).map_err(serde::de::Error::custom)?;
        Ok(OvsdbCondition {
            column,
            function,
            value,
        })
    }
}

/// OVSDB condition functions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum OvsdbConditionFunction {
    // RFC 7047 §5.1: the equality function is "==", not "=".
    #[serde(rename = "==")]
    Equals,
    #[serde(rename = "!=")]
    NotEquals,
    #[serde(rename = "<")]
    LessThan,
    #[serde(rename = ">")]
    GreaterThan,
    #[serde(rename = "<=")]
    LessThanOrEqual,
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
    #[serde(rename = "includes")]
    Includes,
    #[serde(rename = "excludes")]
    Excludes,
}

/// A column mutation inside a `mutate` operation: `[column, mutator, value]`.
#[derive(Debug, Clone, JsonSchema)]
pub struct OvsdbMutation {
    pub column: String,
    pub mutator: OvsdbMutator,
    pub value: OvsdbDatum,
}

impl Serialize for OvsdbMutation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.column, &self.mutator, &self.value).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OvsdbMutation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arr: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
        if arr.len() != 3 {
            return Err(serde::de::Error::custom(
                "mutation must be [column, mutator, value]",
            ));
        }
        let column: String =
            serde_json::from_value(arr[0].clone()).map_err(serde::de::Error::custom)?;
        let mutator: OvsdbMutator =
            serde_json::from_value(arr[1].clone()).map_err(serde::de::Error::custom)?;
        let value: OvsdbDatum =
            serde_json::from_value(arr[2].clone()).map_err(serde::de::Error::custom)?;
        Ok(OvsdbMutation {
            column,
            mutator,
            value,
        })
    }
}

/// OVSDB mutators for set/map manipulation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum OvsdbMutator {
    #[serde(rename = "insert")]
    Insert,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "+=")]
    Add,
    #[serde(rename = "-=")]
    Subtract,
}

/// Result of a single operation inside a `transact` response.
#[derive(Debug, Clone, JsonSchema)]
pub enum OvsdbOperationResult {
    Error {
        error: String,
    },
    Select {
        rows: Vec<BTreeMap<String, OvsdbDatum>>,
    },
    Insert {
        uuid: OvsdbAtom,
    },
    Count {
        count: u64,
    },
    Empty,
}

impl Serialize for OvsdbOperationResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            OvsdbOperationResult::Error { error } => {
                serde_json::json!({"error": error}).serialize(serializer)
            }
            OvsdbOperationResult::Select { rows } => {
                serde_json::json!({"rows": rows}).serialize(serializer)
            }
            OvsdbOperationResult::Insert { uuid } => {
                serde_json::json!({"uuid": uuid}).serialize(serializer)
            }
            OvsdbOperationResult::Count { count } => {
                serde_json::json!({"count": count}).serialize(serializer)
            }
            OvsdbOperationResult::Empty => {
                serde_json::Value::Object(Default::default()).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for OvsdbOperationResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Object(map) => {
                if let Some(err) = map.get("error").and_then(serde_json::Value::as_str) {
                    return Ok(OvsdbOperationResult::Error {
                        error: err.to_string(),
                    });
                }
                if let Some(rows) = map.get("rows") {
                    let rows: Vec<BTreeMap<String, OvsdbDatum>> =
                        serde_json::from_value(rows.clone()).map_err(serde::de::Error::custom)?;
                    return Ok(OvsdbOperationResult::Select { rows });
                }
                if let Some(uuid) = map.get("uuid") {
                    let uuid: OvsdbAtom =
                        serde_json::from_value(uuid.clone()).map_err(serde::de::Error::custom)?;
                    return Ok(OvsdbOperationResult::Insert { uuid });
                }
                if let Some(count) = map.get("count").and_then(serde_json::Value::as_u64) {
                    return Ok(OvsdbOperationResult::Count { count });
                }
                Ok(OvsdbOperationResult::Empty)
            }
            _ => Err(serde::de::Error::custom("operation result must be object")),
        }
    }
}

// =============================================================================
// Convenience / high-level helpers (kept for backward compatibility)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateBridgeInput {
    pub bridge_name: String,
    #[serde(default = "default_datapath_type")]
    pub datapath_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateBridgeOutput {
    pub created: String,
}

fn default_datapath_type() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteBridgeInput {
    pub bridge_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteBridgeOutput {
    pub deleted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortInput {
    pub bridge_name: String,
    pub port_name: String,
    #[serde(default = "default_interface_type")]
    pub interface_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortOutput {
    pub added: String,
    #[serde(rename = "to")]
    pub destination: String,
}

/// One port in an atomic multi-port attach.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PortSpec {
    pub port_name: String,
    #[serde(default = "default_interface_type")]
    pub interface_type: String,
}

/// Attach multiple ports to a bridge in ONE OVSDB transaction.
/// Port names come from the caller (e.g. control-plane-network); this
/// method never hardcodes host interfaces.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortsInput {
    pub bridge_name: String,
    pub ports: Vec<PortSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortsOutput {
    pub added: Vec<String>,
    pub already_attached: Vec<String>,
    #[serde(rename = "to")]
    pub destination: String,
}

/// Create bridge (if missing) AND attach ports in ONE OVSDB transaction.
/// Separate create_bridge + add_port calls leave a window where eth0 is not
/// yet on the bridge — that does not work for boot. Port names come only from
/// the caller (control-plane-network / network.conf).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnsureBridgePortsInput {
    pub bridge_name: String,
    pub ports: Vec<PortSpec>,
    #[serde(default = "default_datapath_type")]
    pub datapath_type: String,
    #[serde(default = "default_fail_mode")]
    pub fail_mode: String,
}

fn default_fail_mode() -> String {
    "standalone".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnsureBridgePortsOutput {
    pub bridge: String,
    pub created_bridge: bool,
    pub added: Vec<String>,
    pub already_attached: Vec<String>,
}

fn default_interface_type() -> String {
    "internal".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemovePortInput {
    pub bridge_name: String,
    pub port_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemovePortOutput {
    pub removed: String,
    #[serde(rename = "from")]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPortsInput {
    pub bridge_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListBridgesOutput {
    pub bridges: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPortsOutput {
    pub ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetControllerInput {
    pub bridge_name: String,
    pub controller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetBridgePropertyInput {
    pub bridge_name: String,
    pub property: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetInterfaceInput {
    pub bridge_name: String,
    pub port_name: String,
    pub property: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetPropertyOutput {
    pub set: String,
    pub on: String,
}

// =============================================================================
// Plugin implementation
// =============================================================================

pub struct RovsCommandsPlugin;

impl Default for RovsCommandsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl RovsCommandsPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn current_state() -> Value {
        simd_json::json!({
            "available": true,
            "schema_version": "1.0.0",
        })
    }

    pub fn schema() -> Option<PluginSchema> {
        Some(rovs_commands_schema())
    }

    /// Execute a typed OVS command directly. The public transport is the
    /// generated `operation.plugin.v1.RovsCommandsPluginMethods` gRPC service.
    pub async fn call_direct(method: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
        match method {
            // RFC 7047 wire methods
            "list_dbs" => Ok(serde_json::to_value(ListDbsResponse {
                databases: Self::list_dbs().await?,
            })?),
            "get_schema" => {
                let input: GetSchemaRequest = serde_json::from_value(args.clone())?;
                let schema = Self::get_schema(&input.database).await?;
                Ok(serde_json::to_value(GetSchemaResponse { schema })?)
            }
            "transact" => {
                let input: TransactRequest = serde_json::from_value(args.clone())?;
                let results = Self::transact(input).await?;
                Ok(serde_json::to_value(TransactResponse { results })?)
            }
            "monitor" => {
                let input: MonitorRequest = serde_json::from_value(args.clone())?;
                let tables = Self::monitor(input).await?;
                Ok(serde_json::to_value(MonitorResponse { tables })?)
            }
            "monitor_cond" => {
                let input: MonitorCondRequest = serde_json::from_value(args.clone())?;
                let tables = Self::monitor_cond(input).await?;
                Ok(serde_json::to_value(MonitorCondResponse { tables })?)
            }
            "lock" => {
                let input: LockRequest = serde_json::from_value(args.clone())?;
                let locked = Self::lock(&input.lock_id).await?;
                Ok(serde_json::to_value(LockResponse { locked })?)
            }
            "steal" => {
                let input: LockRequest = serde_json::from_value(args.clone())?;
                let locked = Self::steal(&input.lock_id).await?;
                Ok(serde_json::to_value(LockResponse { locked })?)
            }
            "unlock" => {
                let input: LockRequest = serde_json::from_value(args.clone())?;
                let locked = Self::unlock(&input.lock_id).await?;
                Ok(serde_json::to_value(LockResponse { locked })?)
            }
            "echo" => {
                let input: EchoRequest = serde_json::from_value(args.clone())?;
                let mut conn = Self::raw_connection().await?;
                let params = serde_json::to_value(input.params)?;
                let result = conn
                    .transact("echo", params)
                    .await
                    .context("echo OVSDB RPC")?;
                let params: Vec<OvsdbDatum> =
                    serde_json::from_value(result).context("parse echo result")?;
                Ok(serde_json::to_value(EchoResponse { params })?)
            }
            "cancel" => {
                let input: CancelRequest = serde_json::from_value(args.clone())?;
                let mut conn = Self::raw_connection().await?;
                conn.notify("cancel", serde_json::json!([input.id]))
                    .await
                    .context("cancel OVSDB RPC")?;
                Ok(serde_json::to_value(CancelResponse { cancelled: true })?)
            }
            // Convenience helpers
            "create_bridge" => {
                let input: CreateBridgeInput = serde_json::from_value(args.clone())?;
                Self::create_bridge(&input).await?;
                Ok(serde_json::to_value(CreateBridgeOutput {
                    created: input.bridge_name,
                })?)
            }
            "delete_bridge" => {
                let input: DeleteBridgeInput = serde_json::from_value(args.clone())?;
                Self::delete_bridge(&input.bridge_name).await?;
                Ok(serde_json::to_value(DeleteBridgeOutput {
                    deleted: input.bridge_name,
                })?)
            }
            "add_port" => {
                let input: AddPortInput = serde_json::from_value(args.clone())?;
                Self::add_port(&input).await?;
                Ok(serde_json::to_value(AddPortOutput {
                    added: input.port_name,
                    destination: input.bridge_name,
                })?)
            }
            "add_ports" => {
                let input: AddPortsInput = serde_json::from_value(args.clone())?;
                let (added, already) = Self::add_ports(&input).await?;
                Ok(serde_json::to_value(AddPortsOutput {
                    added,
                    already_attached: already,
                    destination: input.bridge_name,
                })?)
            }
            "ensure_bridge_ports" => {
                let input: EnsureBridgePortsInput = serde_json::from_value(args.clone())?;
                let out = Self::ensure_bridge_ports(&input).await?;
                Ok(serde_json::to_value(out)?)
            }
            "remove_port" => {
                let input: RemovePortInput = serde_json::from_value(args.clone())?;
                Self::remove_port(&input.bridge_name, &input.port_name).await?;
                Ok(serde_json::to_value(RemovePortOutput {
                    removed: input.port_name,
                    source: input.bridge_name,
                })?)
            }
            "list_bridges" => Ok(serde_json::to_value(ListBridgesOutput {
                bridges: Self::list_bridges().await?,
            })?),
            "list_ports" => {
                let input: ListPortsInput = serde_json::from_value(args.clone())?;
                Ok(serde_json::to_value(ListPortsOutput {
                    ports: Self::list_ports(&input.bridge_name).await?,
                })?)
            }
            "set_controller" => {
                let input: SetControllerInput = serde_json::from_value(args.clone())?;
                Self::set_controller(&input.bridge_name, &input.controller).await?;
                Ok(serde_json::to_value(SetPropertyOutput {
                    set: "controller".to_string(),
                    on: input.bridge_name,
                })?)
            }
            "set_bridge_property" => {
                let input: SetBridgePropertyInput = serde_json::from_value(args.clone())?;
                Self::set_property("Bridge", &input.bridge_name, &input.property, &input.value)
                    .await?;
                Ok(serde_json::to_value(SetPropertyOutput {
                    set: input.property,
                    on: input.bridge_name,
                })?)
            }
            "set_interface" => {
                let input: SetInterfaceInput = serde_json::from_value(args.clone())?;
                Self::set_property("Interface", &input.port_name, &input.property, &input.value)
                    .await?;
                Ok(serde_json::to_value(SetPropertyOutput {
                    set: input.property,
                    on: input.port_name,
                })?)
            }
            _ => bail!("unknown rovs_commands method: {method}"),
        }
    }

    fn endpoint() -> String {
        let configured =
            std::env::var("OVSDB_SOCK").unwrap_or_else(|_| "/run/openvswitch/db.sock".to_string());
        if configured.starts_with("unix:")
            || configured.starts_with("tcp:")
            || configured.starts_with("ssl:")
        {
            configured
        } else {
            format!("unix:{configured}")
        }
    }

    async fn connect(tables: &[&str]) -> Result<Client> {
        let config = ClientConfig::open_vswitch()
            .tables(tables.iter().map(|table| (*table).to_string()).collect());
        Client::connect_with_config(&Self::endpoint(), config)
            .await
            .context("connect direct rovs_commands authority")
    }

    /// Open a raw JSON-RPC connection to the OVSDB socket for RFC 7047 wire methods.
    async fn raw_connection() -> Result<rovs_jsonrpc::Connection> {
        let addr: rovs_transport::Address = Self::endpoint()
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid OVSDB address: {e}"))?;
        let stream = rovs_transport::Stream::connect(&addr)
            .await
            .context("connect raw OVSDB JSON-RPC socket")?;
        Ok(rovs_jsonrpc::Connection::new(stream))
    }

    async fn commit(client: &mut Client, transaction: &mut Transaction) -> Result<()> {
        if !client
            .commit(transaction)
            .await
            .context("commit OVS transaction")?
        {
            bail!("OVS transaction was rejected");
        }
        Ok(())
    }

    async fn create_bridge(input: &CreateBridgeInput) -> Result<()> {
        let mut client = Self::connect(&["Open_vSwitch", "Bridge", "Port", "Interface"]).await?;
        if client
            .idl()
            .rows("Bridge")
            .any(|row| row.get_string("name") == Some(input.bridge_name.as_str()))
        {
            return Ok(());
        }
        let mut transaction = Transaction::new("Open_vSwitch");
        transaction.create_bridge(&input.bridge_name);
        if input.datapath_type == "netdev" {
            transaction.update_by_name(
                "Bridge",
                &input.bridge_name,
                serde_json::json!({"datapath_type": "netdev"}),
            );
        }
        Self::commit(&mut client, &mut transaction).await
    }

    async fn delete_bridge(bridge_name: &str) -> Result<()> {
        let mut client = Self::connect(&["Open_vSwitch", "Bridge", "Port", "Interface"]).await?;
        let Some(bridge) = client
            .idl()
            .rows("Bridge")
            .find(|row| row.get_string("name") == Some(bridge_name))
        else {
            return Ok(());
        };
        let bridge_uuid = bridge.uuid;
        let port_uuids = bridge.get("ports").map(Self::uuid_set).unwrap_or_default();
        let interface_uuids = port_uuids
            .iter()
            .filter_map(|uuid| client.idl().row("Port", uuid))
            .filter_map(|port| port.get("interfaces"))
            .flat_map(Self::uuid_set)
            .collect::<Vec<_>>();
        let mut transaction = Transaction::new("Open_vSwitch");
        transaction.delete_bridge_uuid(bridge_uuid, &port_uuids, &interface_uuids);
        Self::commit(&mut client, &mut transaction).await
    }

    async fn add_port(input: &AddPortInput) -> Result<()> {
        let (added, _) = Self::add_ports(&AddPortsInput {
            bridge_name: input.bridge_name.clone(),
            ports: vec![PortSpec {
                port_name: input.port_name.clone(),
                interface_type: input.interface_type.clone(),
            }],
        })
        .await?;
        let _ = added;
        Ok(())
    }

    /// Attach every port in `input.ports` that is not already on the bridge,
    /// as a single OVSDB transaction (all-or-nothing).
    async fn add_ports(input: &AddPortsInput) -> Result<(Vec<String>, Vec<String>)> {
        if input.ports.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut client = Self::connect(&["Bridge", "Port", "Interface"]).await?;
        let Some(bridge) = client
            .idl()
            .rows("Bridge")
            .find(|row| row.get_string("name") == Some(input.bridge_name.as_str()))
        else {
            bail!("bridge '{}' not found", input.bridge_name);
        };
        let attached_names: std::collections::HashSet<String> = bridge
            .get("ports")
            .map(Self::uuid_set)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|uuid| client.idl().row("Port", &uuid))
            .filter_map(|port| port.get_string("name").map(str::to_string))
            .collect();

        let mut already = Vec::new();
        let mut to_add: Vec<&PortSpec> = Vec::new();
        for p in &input.ports {
            if attached_names.contains(&p.port_name) {
                already.push(p.port_name.clone());
            } else {
                to_add.push(p);
            }
        }
        if to_add.is_empty() {
            return Ok((Vec::new(), already));
        }

        let mut transaction = Transaction::new("Open_vSwitch");
        let mut added = Vec::new();
        for p in to_add {
            transaction.add_port(&input.bridge_name, &p.port_name, &p.interface_type);
            added.push(p.port_name.clone());
        }
        Self::commit(&mut client, &mut transaction).await?;
        Ok((added, already))
    }

    /// ONE OVSDB transaction: create bridge if missing, set datapath/fail_mode,
    /// attach every requested system port. Strictly typed via EnsureBridgePortsInput.
    async fn ensure_bridge_ports(input: &EnsureBridgePortsInput) -> Result<EnsureBridgePortsOutput> {
        let mut client =
            Self::connect(&["Open_vSwitch", "Bridge", "Port", "Interface"]).await?;
        let bridge_exists = client
            .idl()
            .rows("Bridge")
            .any(|row| row.get_string("name") == Some(input.bridge_name.as_str()));

        let mut already = Vec::new();
        let mut to_add: Vec<&PortSpec> = Vec::new();
        if bridge_exists {
            let bridge = client
                .idl()
                .rows("Bridge")
                .find(|row| row.get_string("name") == Some(input.bridge_name.as_str()))
                .expect("bridge exists");
            let attached_names: std::collections::HashSet<String> = bridge
                .get("ports")
                .map(Self::uuid_set)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|uuid| client.idl().row("Port", &uuid))
                .filter_map(|port| port.get_string("name").map(str::to_string))
                .collect();
            for p in &input.ports {
                if attached_names.contains(&p.port_name) {
                    already.push(p.port_name.clone());
                } else {
                    to_add.push(p);
                }
            }
        } else {
            to_add = input.ports.iter().collect();
        }

        // Nothing to do.
        if bridge_exists && to_add.is_empty() {
            // Still refresh fail_mode / datapath if requested.
            let mut transaction = Transaction::new("Open_vSwitch");
            let mut row = serde_json::Map::new();
            row.insert(
                "datapath_type".into(),
                serde_json::Value::String(input.datapath_type.clone()),
            );
            row.insert(
                "fail_mode".into(),
                serde_json::Value::String(input.fail_mode.clone()),
            );
            transaction.update_by_name(
                "Bridge",
                &input.bridge_name,
                serde_json::Value::Object(row),
            );
            Self::commit(&mut client, &mut transaction).await?;
            return Ok(EnsureBridgePortsOutput {
                bridge: input.bridge_name.clone(),
                created_bridge: false,
                added: Vec::new(),
                already_attached: already,
            });
        }

        let mut transaction = Transaction::new("Open_vSwitch");
        let created_bridge = !bridge_exists;
        if !bridge_exists {
            transaction.create_bridge(&input.bridge_name);
        }
        // Properties on the bridge (new or existing) in the same txn.
        let mut row = serde_json::Map::new();
        row.insert(
            "datapath_type".into(),
            serde_json::Value::String(input.datapath_type.clone()),
        );
        row.insert(
            "fail_mode".into(),
            serde_json::Value::String(input.fail_mode.clone()),
        );
        transaction.update_by_name(
            "Bridge",
            &input.bridge_name,
            serde_json::Value::Object(row),
        );
        let mut added = Vec::new();
        for p in to_add {
            transaction.add_port(&input.bridge_name, &p.port_name, &p.interface_type);
            added.push(p.port_name.clone());
        }
        Self::commit(&mut client, &mut transaction).await?;
        Ok(EnsureBridgePortsOutput {
            bridge: input.bridge_name.clone(),
            created_bridge,
            added,
            already_attached: already,
        })
    }

    async fn remove_port(bridge_name: &str, port_name: &str) -> Result<()> {
        let mut client = Self::connect(&["Bridge", "Port", "Interface"]).await?;
        let Some(bridge) = client
            .idl()
            .rows("Bridge")
            .find(|row| row.get_string("name") == Some(bridge_name))
        else {
            bail!("bridge '{bridge_name}' not found");
        };
        let bridge_uuid = bridge.uuid;
        let attached = bridge.get("ports").map(Self::uuid_set).unwrap_or_default();
        let Some(port) = attached
            .iter()
            .filter_map(|uuid| client.idl().row("Port", uuid))
            .find(|row| row.get_string("name") == Some(port_name))
        else {
            return Ok(());
        };
        let port_uuid = port.uuid;
        let interface_uuids = port
            .get("interfaces")
            .map(Self::uuid_set)
            .unwrap_or_default();
        let mut transaction = Transaction::new("Open_vSwitch");
        transaction.mutate(
            "Bridge",
            bridge_uuid,
            vec![serde_json::json!([
                "ports",
                "delete",
                ["set", [["uuid", port_uuid.to_string()]]]
            ])],
        );
        for interface_uuid in interface_uuids {
            transaction.delete("Interface", interface_uuid);
        }
        transaction.delete("Port", port_uuid);
        Self::commit(&mut client, &mut transaction).await
    }

    async fn list_bridges() -> Result<Vec<String>> {
        let client = Self::connect(&["Bridge"]).await?;
        let mut bridges = client
            .idl()
            .rows("Bridge")
            .filter_map(|row| row.get_string("name").map(str::to_string))
            .collect::<Vec<_>>();
        bridges.sort();
        Ok(bridges)
    }

    async fn list_ports(bridge_name: &str) -> Result<Vec<String>> {
        let client = Self::connect(&["Bridge", "Port"]).await?;
        let bridge = client
            .idl()
            .rows("Bridge")
            .find(|row| row.get_string("name") == Some(bridge_name))
            .with_context(|| format!("bridge '{bridge_name}' not found"))?;
        let mut ports = bridge
            .get("ports")
            .map(Self::uuid_set)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|uuid| client.idl().row("Port", &uuid))
            .filter_map(|row| row.get_string("name").map(str::to_string))
            .collect::<Vec<_>>();
        ports.sort();
        Ok(ports)
    }

    async fn list_dbs() -> Result<Vec<String>> {
        let mut conn = Self::raw_connection().await?;
        let result = conn
            .transact("list_dbs", serde_json::json!([]))
            .await
            .context("list_dbs OVSDB RPC")?;
        let mut databases: Vec<String> =
            serde_json::from_value(result).context("parse list_dbs result")?;
        databases.sort();
        Ok(databases)
    }

    async fn get_schema(database: &str) -> Result<OvsdbDbSchema> {
        let mut conn = Self::raw_connection().await?;
        let result = conn
            .transact("get_schema", serde_json::json!([database]))
            .await
            .context("get_schema OVSDB RPC")?;
        let mut schema: OvsdbDbSchema =
            serde_json::from_value(result).context("parse OVSDB schema")?;
        schema.name = database.to_string();
        Ok(schema)
    }

    async fn transact(request: TransactRequest) -> Result<Vec<OvsdbOperationResult>> {
        let mut conn = Self::raw_connection().await?;
        // RFC 7047 §4.1.1: the "transact" params array is [db, op1, op2, ...]
        // -- a FLAT array with the database name followed by each operation
        // as its own top-level element, not the database name plus a
        // nested sub-array of operations.
        let mut params = vec![serde_json::Value::String(request.database.clone())];
        match serde_json::to_value(request.operations)? {
            serde_json::Value::Array(op_values) => params.extend(op_values),
            other => return Err(anyhow::anyhow!("operations did not serialize to an array: {other}")),
        }
        let params = serde_json::Value::Array(params);
        let result = conn
            .transact("transact", params)
            .await
            .context("transact OVSDB RPC")?;
        serde_json::from_value(result).context("parse transact result")
    }

    async fn monitor(
        request: MonitorRequest,
    ) -> Result<BTreeMap<String, BTreeMap<String, BTreeMap<String, OvsdbDatum>>>> {
        let mut conn = Self::raw_connection().await?;
        let requests = serde_json::to_value(request.monitor_requests)?;
        let params = serde_json::json!([request.database, request.monitor_id, requests]);
        let result = conn
            .transact("monitor", params)
            .await
            .context("monitor OVSDB RPC")?;
        serde_json::from_value(result).context("parse monitor result")
    }

    async fn monitor_cond(
        request: MonitorCondRequest,
    ) -> Result<BTreeMap<String, BTreeMap<String, BTreeMap<String, OvsdbDatum>>>> {
        let mut conn = Self::raw_connection().await?;
        let requests = serde_json::to_value(request.monitor_cond_requests)?;
        let params = serde_json::json!([request.database, request.monitor_id, requests]);
        let result = conn
            .transact("monitor_cond", params)
            .await
            .context("monitor_cond OVSDB RPC")?;
        serde_json::from_value(result).context("parse monitor_cond result")
    }

    async fn lock(lock_id: &str) -> Result<bool> {
        let mut conn = Self::raw_connection().await?;
        let result = conn
            .transact("lock", serde_json::json!([lock_id]))
            .await
            .context("lock OVSDB RPC")?;
        serde_json::from_value(result).context("parse lock result")
    }

    async fn steal(lock_id: &str) -> Result<bool> {
        let mut conn = Self::raw_connection().await?;
        let result = conn
            .transact("steal", serde_json::json!([lock_id]))
            .await
            .context("steal OVSDB RPC")?;
        serde_json::from_value(result).context("parse steal result")
    }

    async fn unlock(lock_id: &str) -> Result<bool> {
        let mut conn = Self::raw_connection().await?;
        let result = conn
            .transact("unlock", serde_json::json!([lock_id]))
            .await
            .context("unlock OVSDB RPC")?;
        serde_json::from_value(result).context("parse unlock result")
    }

    async fn set_controller(bridge_name: &str, controller: &str) -> Result<()> {
        let mut client = Self::connect(&["Bridge", "Controller"]).await?;
        let mut transaction = Transaction::new("Open_vSwitch");
        transaction.set_controller(bridge_name, controller);
        Self::commit(&mut client, &mut transaction).await
    }

    async fn set_property(table: &str, name: &str, property: &str, value: &str) -> Result<()> {
        let mut client = Self::connect(&[table]).await?;
        let mut transaction = Transaction::new("Open_vSwitch");
        if let Some((column, key)) = property.split_once(':') {
            transaction.mutate_by_name(
                table,
                name,
                vec![serde_json::json!([
                    column,
                    "insert",
                    ["map", [[key, value]]]
                ])],
            );
        } else {
            let mut row = serde_json::Map::new();
            row.insert(property.to_string(), Self::property_value(value));
            transaction.update_by_name(table, name, serde_json::Value::Object(row));
        }
        Self::commit(&mut client, &mut transaction).await
    }

    fn property_value(value: &str) -> serde_json::Value {
        serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.to_string()))
    }

    fn uuid_set(value: &serde_json::Value) -> Vec<Uuid> {
        let Some(parts) = value.as_array() else {
            return Vec::new();
        };
        match parts.first().and_then(serde_json::Value::as_str) {
            Some("uuid") => parts
                .get(1)
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.parse().ok())
                .into_iter()
                .collect(),
            Some("set") => parts
                .get(1)
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .flat_map(Self::uuid_set)
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[async_trait]
impl StatePlugin for RovsCommandsPlugin {
    fn name(&self) -> &str {
        "rovs_commands"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Self::schema()
    }

    async fn calculate_diff(
        &self,
        _current: &Value,
        _desired: &Value,
    ) -> anyhow::Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-only".to_string(),
                desired_hash: "schema-only".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> anyhow::Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        Ok(Checkpoint {
            id: Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!(null),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> anyhow::Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }

    async fn call_method(&self, method: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
        Self::call_direct(method, args).await
    }
}

pub(crate) fn rovs_commands_schema() -> PluginSchema {
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;

    let mut methods = HashMap::new();

    // RFC 7047 wire methods
    methods.insert(
        "list_dbs".to_string(),
        method_decl_from_schemars_with_output::<ListDbsRequest, ListDbsResponse>(
            "list_dbs",
            SideEffect::Read,
            true,
            "cap.network.ovsdb.dbs.list@v1",
            "obs.network.ovsdb.dbs.list@v1",
        ),
    );
    methods.insert(
        "get_schema".to_string(),
        method_decl_from_schemars_with_output::<GetSchemaRequest, GetSchemaResponse>(
            "get_schema",
            SideEffect::Read,
            true,
            "cap.network.ovsdb.schema.get@v1",
            "obs.network.ovsdb.schema.get@v1",
        ),
    );
    methods.insert(
        "transact".to_string(),
        method_decl_from_schemars_with_output::<TransactRequest, TransactResponse>(
            "transact",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.transact@v1",
            "mut.network.ovsdb.transact@v1",
        ),
    );
    methods.insert(
        "monitor".to_string(),
        method_decl_from_schemars_with_output::<MonitorRequest, MonitorResponse>(
            "monitor",
            SideEffect::Read,
            false,
            "cap.network.ovsdb.monitor@v1",
            "obs.network.ovsdb.monitor@v1",
        ),
    );
    methods.insert(
        "monitor_cond".to_string(),
        method_decl_from_schemars_with_output::<MonitorCondRequest, MonitorCondResponse>(
            "monitor_cond",
            SideEffect::Read,
            false,
            "cap.network.ovsdb.monitor_cond@v1",
            "obs.network.ovsdb.monitor_cond@v1",
        ),
    );
    methods.insert(
        "lock".to_string(),
        method_decl_from_schemars_with_output::<LockRequest, LockResponse>(
            "lock",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.lock@v1",
            "mut.network.ovsdb.lock@v1",
        ),
    );
    methods.insert(
        "steal".to_string(),
        method_decl_from_schemars_with_output::<LockRequest, LockResponse>(
            "steal",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.lock.steal@v1",
            "mut.network.ovsdb.lock.steal@v1",
        ),
    );
    methods.insert(
        "unlock".to_string(),
        method_decl_from_schemars_with_output::<LockRequest, LockResponse>(
            "unlock",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.lock.unlock@v1",
            "mut.network.ovsdb.lock.unlock@v1",
        ),
    );
    methods.insert(
        "echo".to_string(),
        method_decl_from_schemars_with_output::<EchoRequest, EchoResponse>(
            "echo",
            SideEffect::Read,
            true,
            "cap.network.ovsdb.echo@v1",
            "obs.network.ovsdb.echo@v1",
        ),
    );
    methods.insert(
        "cancel".to_string(),
        method_decl_from_schemars_with_output::<CancelRequest, CancelResponse>(
            "cancel",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.cancel@v1",
            "mut.network.ovsdb.cancel@v1",
        ),
    );

    // Convenience helpers
    methods.insert(
        "create_bridge".to_string(),
        method_decl_from_schemars_with_output::<CreateBridgeInput, CreateBridgeOutput>(
            "create_bridge",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.bridge.create@v1",
            "mut.network.ovsdb.bridge.create@v1",
        ),
    );
    methods.insert(
        "delete_bridge".to_string(),
        method_decl_from_schemars_with_output::<DeleteBridgeInput, DeleteBridgeOutput>(
            "delete_bridge",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.bridge.delete@v1",
            "mut.network.ovsdb.bridge.delete@v1",
        ),
    );
    methods.insert(
        "add_port".to_string(),
        method_decl_from_schemars_with_output::<AddPortInput, AddPortOutput>(
            "add_port",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.port.add@v1",
            "mut.network.ovsdb.port.add@v1",
        ),
    );
    methods.insert(
        "add_ports".to_string(),
        method_decl_from_schemars_with_output::<AddPortsInput, AddPortsOutput>(
            "add_ports",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.port.add@v1",
            "mut.network.ovsdb.ports.add@v1",
        ),
    );
    methods.insert(
        "ensure_bridge_ports".to_string(),
        method_decl_from_schemars_with_output::<EnsureBridgePortsInput, EnsureBridgePortsOutput>(
            "ensure_bridge_ports",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.bridge.ensure-ports@v1",
            "mut.network.ovsdb.bridge.ensure-ports@v1",
        ),
    );
    methods.insert(
        "remove_port".to_string(),
        method_decl_from_schemars_with_output::<RemovePortInput, RemovePortOutput>(
            "remove_port",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.port.delete@v1",
            "mut.network.ovsdb.port.delete@v1",
        ),
    );
    methods.insert(
        "list_bridges".to_string(),
        method_decl_from_schemars_with_output::<(), ListBridgesOutput>(
            "list_bridges",
            SideEffect::Read,
            true,
            "cap.network.ovsdb.bridge.list@v1",
            "obs.network.ovsdb.bridge.list@v1",
        ),
    );
    methods.insert(
        "list_ports".to_string(),
        method_decl_from_schemars_with_output::<ListPortsInput, ListPortsOutput>(
            "list_ports",
            SideEffect::Read,
            true,
            "cap.network.ovsdb.port.list@v1",
            "obs.network.ovsdb.port.list@v1",
        ),
    );
    methods.insert(
        "set_controller".to_string(),
        method_decl_from_schemars_with_output::<SetControllerInput, SetPropertyOutput>(
            "set_controller",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.controller.set@v1",
            "mut.network.ovsdb.controller.set@v1",
        ),
    );
    methods.insert(
        "set_bridge_property".to_string(),
        method_decl_from_schemars_with_output::<SetBridgePropertyInput, SetPropertyOutput>(
            "set_bridge_property",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.bridge.property.set@v1",
            "mut.network.ovsdb.bridge.property.set@v1",
        ),
    );
    methods.insert(
        "set_interface".to_string(),
        method_decl_from_schemars_with_output::<SetInterfaceInput, SetPropertyOutput>(
            "set_interface",
            SideEffect::Mutation,
            false,
            "cap.network.ovsdb.interface.property.set@v1",
            "mut.network.ovsdb.interface.property.set@v1",
        ),
    );

    PluginSchema::builder("rovs_commands")
        .version("1.0.0")
        .category("network")
        .description("Full RFC 7047 OVSDB protocol surface with typed convenience helpers")
        .methods(methods)
        .build()
}

inventory::submit! {
    crate::default_registry::PluginReg::new("rovs_commands", |_ctx| std::sync::Arc::new(RovsCommandsPlugin::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::prelude::*;

    #[test]
    fn schema_has_typed_transact() {
        let schema = rovs_commands_schema();
        let transact = schema
            .methods
            .get("transact")
            .expect("transact method should exist");
        let args = transact
            .args
            .get("properties")
            .and_then(Value::as_object)
            .expect("transact args should have properties");
        assert!(args.contains_key("database"));
        assert!(args.contains_key("operations"));
    }

    #[test]
    fn schema_has_all_rfc7047_methods() {
        let schema = rovs_commands_schema();
        for method in [
            "list_dbs",
            "get_schema",
            "transact",
            "monitor",
            "monitor_cond",
            "lock",
            "steal",
            "unlock",
            "echo",
            "cancel",
        ] {
            assert!(
                schema.methods.contains_key(method),
                "missing RFC 7047 method: {method}"
            );
        }
    }
}
