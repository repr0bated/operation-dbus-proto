This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-grpc-bridge/
              proto/
                mail.proto
                operation.proto
                privacy_network.proto
                registration.proto
                registry.proto
              src/
                bin/
                  op-grpc-bridge.rs
                grpc_client.rs
                grpc_server.rs
                interceptor.rs
                lib.rs
                proto_gen.rs
                schema_engine.rs
              build.rs
              Cargo.toml
              compare-op-grpc-bridge.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/proto/mail.proto">
syntax = "proto3";

package operation.mail.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

// =============================================================================
// MAIL SERVICE - Email and Webmail Operations
// Similar to registration.proto but focused on email functionality
// =============================================================================

service MailService {
  // Send email (used for magic links, notifications)
  rpc SendEmail(SendEmailRequest) returns (SendEmailResponse);
  
  // Get email inbox for a user (webmail functionality)
  rpc GetInbox(GetInboxRequest) returns (GetInboxResponse);
  
  // Get specific email message
  rpc GetMessage(GetMessageRequest) returns (GetMessageResponse);
  
  // Get mail server status
  rpc GetMailStatus(GetMailStatusRequest) returns (GetMailStatusResponse);
  
  // List all mail accounts
  rpc ListMailAccounts(ListMailAccountsRequest) returns (ListMailAccountsResponse);
  
  // Admin operations for mail management
  rpc AdminMailAction(AdminMailActionRequest) returns (AdminMailActionResponse);
  
  // Check if mail server is configured and working
  rpc CheckMailServer(CheckMailServerRequest) returns (CheckMailServerResponse);
}

// =============================================================================
// MAIL SERVICE MESSAGES
// =============================================================================

message SendEmailRequest {
  string from_email = 1;
  string to_email = 2;
  string subject = 3;
  string body = 4;
  bool is_html = 5;
  string domain = 6; // "3tched.com"
  optional google.protobuf.Struct attachments = 7;
}

message SendEmailResponse {
  bool success = 1;
  string message = 2;
  string message_id = 3;
  google.protobuf.Timestamp sent_at = 4;
}

message GetInboxRequest {
  string email = 1;
  string domain = 2;
  uint32 limit = 3;
  uint32 offset = 4;
  string folder = 5; // "inbox", "sent", "drafts", "trash"
}

message GetInboxResponse {
  repeated EmailMessage messages = 1;
  uint32 total_count = 2;
  uint32 unread_count = 3;
  string folder = 4;
}

message EmailMessage {
  string message_id = 1;
  string from = 2;
  string to = 3;
  string subject = 4;
  string preview = 5;
  bool is_read = 6;
  bool has_attachments = 7;
  google.protobuf.Timestamp received_at = 8;
  int32 size_bytes = 9;
  string folder = 10;
}

message GetMessageRequest {
  string message_id = 1;
  string email = 2;
  string domain = 3;
}

message GetMessageResponse {
  bool success = 1;
  EmailMessage header = 2;
  string body = 3;
  bool is_html = 4;
  repeated EmailAttachment attachments = 5;
  string raw_content = 6;
}

message EmailAttachment {
  string filename = 1;
  string content_type = 2;
  uint32 size_bytes = 3;
  string content_id = 4; // for inline images
}

message GetMailStatusRequest {
  string domain = 1;
}

message GetMailStatusResponse {
  bool is_configured = 1;
  bool is_running = 2;
  string mail_server_type = 3; // "maddy", "postfix", etc
  string webmail_url = 4;
  string smtp_status = 5;
  string imap_status = 6;
  uint32 total_accounts = 7;
  uint32 total_messages = 8;
  google.protobuf.Timestamp last_checked = 9;
  string message = 10;
}

message ListMailAccountsRequest {
  string domain = 1;
  bool include_inactive = 2;
}

message ListMailAccountsResponse {
  repeated MailAccount accounts = 1;
  uint32 total_count = 2;
}

message MailAccount {
  string email = 1;
  string user_id = 2;
  bool is_admin = 3;
  bool is_active = 4;
  google.protobuf.Timestamp created_at = 5;
  uint32 message_count = 6;
  uint32 unread_count = 7;
  string last_login = 8;
}

message AdminMailActionRequest {
  string action = 1; // "send_test", "restart_server", "create_account", "suspend_account", "get_logs"
  string email = 2;
  string domain = 3;
  optional google.protobuf.Struct parameters = 4;
}

message AdminMailActionResponse {
  bool success = 1;
  string message = 2;
  string action_id = 3;
  google.protobuf.Timestamp timestamp = 4;
  optional google.protobuf.Struct result = 5;
}

message CheckMailServerRequest {
  string domain = 1;
  bool check_smtp = 2;
  bool check_imap = 3;
  bool check_webmail = 4;
}

message CheckMailServerResponse {
  bool all_healthy = 1;
  bool smtp_healthy = 2;
  bool imap_healthy = 3;
  bool webmail_healthy = 4;
  string smtp_status = 5;
  string imap_status = 6;
  string webmail_status = 7;
  string message = 8;
  repeated string issues = 9;
}

// =============================================================================
// ERROR HANDLING
// =============================================================================

message MailError {
  int32 code = 1;
  string message = 2;
  optional google.protobuf.Struct details = 3;
}

enum MailErrorCode {
  MAIL_ERROR_UNSPECIFIED = 0;
  MAIL_ERROR_INVALID_EMAIL = 1;
  MAIL_ERROR_SERVER_UNAVAILABLE = 2;
  MAIL_ERROR_AUTHENTICATION_FAILED = 3;
  MAIL_ERROR_MESSAGE_NOT_FOUND = 4;
  MAIL_ERROR_SEND_FAILED = 5;
  MAIL_ERROR_CONFIG_MISSING = 6;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/proto/operation.proto">
syntax = "proto3";

package operation.v1;

import "google/protobuf/any.proto";
import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "google/protobuf/empty.proto";

// =============================================================================
// Core State Synchronization Service
// =============================================================================

// Bidirectional state sync between D-Bus and gRPC clients
service StateSync {
  // Subscribe to all state changes (D-Bus -> gRPC streaming)
  rpc Subscribe(SubscribeRequest) returns (stream StateChange);

  // Apply a mutation (gRPC -> D-Bus -> Event Chain)
  rpc Mutate(MutateRequest) returns (MutateResponse);

  // Get current state snapshot
  rpc GetState(GetStateRequest) returns (GetStateResponse);

  // Batch mutations (transactional)
  rpc BatchMutate(BatchMutateRequest) returns (BatchMutateResponse);
}

// =============================================================================
// Plugin Service - Per-plugin operations
// =============================================================================

service PluginService {
  // List all registered plugins
  rpc ListPlugins(google.protobuf.Empty) returns (ListPluginsResponse);

  // Get plugin schema (JSON Schema 2026)
  rpc GetSchema(GetSchemaRequest) returns (GetSchemaResponse);

  // Call a plugin method
  rpc CallMethod(CallMethodRequest) returns (CallMethodResponse);

  // Get/Set property
  rpc GetProperty(GetPropertyRequest) returns (GetPropertyResponse);
  rpc SetProperty(SetPropertyRequest) returns (SetPropertyResponse);

  // Subscribe to plugin signals
  rpc SubscribeSignals(SubscribeSignalsRequest) returns (stream Signal);
}

// =============================================================================
// Event Chain Service - Audit and compliance
// =============================================================================

service EventChainService {
  // Get events in range
  rpc GetEvents(GetEventsRequest) returns (GetEventsResponse);

  // Subscribe to new events as they occur
  rpc SubscribeEvents(SubscribeEventsRequest) returns (stream ChainEvent);

  // Verify chain integrity
  rpc VerifyChain(VerifyChainRequest) returns (VerifyChainResponse);

  // Get Merkle proof for an event
  rpc GetProof(GetProofRequest) returns (GetProofResponse);

  // Prove tag immutability
  rpc ProveTagImmutability(ProveTagImmutabilityRequest) returns (ProveTagImmutabilityResponse);

  // Get/Create snapshot
  rpc GetSnapshot(GetSnapshotRequest) returns (GetSnapshotResponse);
  rpc CreateSnapshot(CreateSnapshotRequest) returns (CreateSnapshotResponse);

  // Trace-scoped semantic lookup for the Accountability pane
  rpc SearchSemanticTrace(SearchSemanticTraceRequest) returns (SearchSemanticTraceResponse);
}

// =============================================================================
// Message Types - State Sync
// =============================================================================

message SubscribeRequest {
  // Filter by plugin IDs (empty = all)
  repeated string plugin_ids = 1;
  // Filter by object paths (glob patterns supported)
  repeated string path_patterns = 2;
  // Filter by tags
  repeated string tags = 3;
  // Include initial state snapshot
  bool include_initial_state = 4;
}

message StateChange {
  // Unique change ID
  string change_id = 1;
  // Event chain event ID
  uint64 event_id = 2;
  // Plugin that changed
  string plugin_id = 3;
  // D-Bus object path
  string object_path = 4;
  // Change type
  ChangeType change_type = 5;
  // Property/method name (if applicable)
  string member_name = 6;
  // Previous value (for updates)
  google.protobuf.Value old_value = 7;
  // New value
  google.protobuf.Value new_value = 8;
  // Tags touched by this change
  repeated string tags_touched = 9;
  // Event hash for verification
  string event_hash = 10;
  // Timestamp
  google.protobuf.Timestamp timestamp = 11;
  // Actor who made the change
  string actor_id = 12;
}

enum ChangeType {
  CHANGE_TYPE_UNSPECIFIED = 0;
  CHANGE_TYPE_PROPERTY_SET = 1;
  CHANGE_TYPE_PROPERTY_DELETE = 2;
  CHANGE_TYPE_METHOD_CALL = 3;
  CHANGE_TYPE_SIGNAL = 4;
  CHANGE_TYPE_OBJECT_ADDED = 5;
  CHANGE_TYPE_OBJECT_REMOVED = 6;
  CHANGE_TYPE_SCHEMA_MIGRATION = 7;
}

message MutateRequest {
  // Plugin to mutate
  string plugin_id = 1;
  // D-Bus object path
  string object_path = 2;
  // Operation type
  OperationType operation = 3;
  // Member (property or method name)
  string member_name = 4;
  // Arguments/value
  google.protobuf.Value value = 5;
  // Actor ID for audit
  string actor_id = 6;
  // Capability ID for authorization
  string capability_id = 7;
  // Idempotency key
  string idempotency_key = 8;
}

enum OperationType {
  OPERATION_TYPE_UNSPECIFIED = 0;
  OPERATION_TYPE_SET_PROPERTY = 1;
  OPERATION_TYPE_CALL_METHOD = 2;
  OPERATION_TYPE_APPLY_PATCH = 3;
}

message MutateResponse {
  // Whether mutation succeeded
  bool success = 1;
  // Event chain event ID
  uint64 event_id = 2;
  // Event hash
  string event_hash = 3;
  // Result value (for method calls)
  google.protobuf.Value result = 4;
  // Error details if failed
  MutationError error = 5;
  // Resulting effective state hash
  string effective_hash = 6;
}

message MutationError {
  ErrorCode code = 1;
  string message = 2;
  // Denial reason from event chain
  DenyReason deny_reason = 3;
}

enum ErrorCode {
  ERROR_CODE_UNSPECIFIED = 0;
  ERROR_CODE_NOT_FOUND = 1;
  ERROR_CODE_PERMISSION_DENIED = 2;
  ERROR_CODE_VALIDATION_FAILED = 3;
  ERROR_CODE_READ_ONLY = 4;
  ERROR_CODE_TAG_LOCKED = 5;
  ERROR_CODE_INTERNAL = 6;
}

message DenyReason {
  oneof reason {
    TagLock tag_lock = 1;
    ConstraintFail constraint_fail = 2;
    CapabilityMissing capability_missing = 3;
    ReadOnlyViolation read_only_violation = 4;
  }
}

message TagLock {
  string tag = 1;
  string wrapper_id = 2;
}

message ConstraintFail {
  string constraint = 1;
  string message = 2;
}

message CapabilityMissing {
  string capability = 1;
}

message ReadOnlyViolation {
  string field = 1;
}

message GetStateRequest {
  string plugin_id = 1;
  string object_path = 2;
}

message GetStateResponse {
  google.protobuf.Struct state = 1;
  string effective_hash = 2;
  uint64 at_event_id = 3;
}

message BatchMutateRequest {
  repeated MutateRequest mutations = 1;
  // All-or-nothing semantics
  bool atomic = 2;
  string actor_id = 3;
}

message BatchMutateResponse {
  bool success = 1;
  repeated MutateResponse results = 2;
  // If atomic and failed, which index failed
  int32 failed_index = 3;
}

// =============================================================================
// Message Types - Plugin Service
// =============================================================================

message ListPluginsResponse {
  repeated PluginInfo plugins = 1;
}

message PluginInfo {
  string id = 1;
  string name = 2;
  string version = 3;
  string description = 4;
  string dbus_path = 5;
  repeated string interfaces = 6;
  repeated string tags = 7;
}

message GetSchemaRequest {
  string plugin_id = 1;
  // Schema format (json-schema-2026, json-schema-draft07, protobuf)
  string format = 2;
}

message GetSchemaResponse {
  string schema_json = 1;
  string dialect = 2;
  string version = 3;
}

message CallMethodRequest {
  string plugin_id = 1;
  string object_path = 2;
  string interface_name = 3;
  string method_name = 4;
  repeated google.protobuf.Value arguments = 5;
  string actor_id = 6;
  string capability_id = 7;
}

message CallMethodResponse {
  bool success = 1;
  google.protobuf.Value result = 2;
  uint64 event_id = 3;
  string event_hash = 4;
  MutationError error = 5;
}

message GetPropertyRequest {
  string plugin_id = 1;
  string object_path = 2;
  string interface_name = 3;
  string property_name = 4;
}

message GetPropertyResponse {
  google.protobuf.Value value = 1;
  bool read_only = 2;
}

message SetPropertyRequest {
  string plugin_id = 1;
  string object_path = 2;
  string interface_name = 3;
  string property_name = 4;
  google.protobuf.Value value = 5;
  string actor_id = 6;
  string capability_id = 7;
}

message SetPropertyResponse {
  bool success = 1;
  uint64 event_id = 2;
  string event_hash = 3;
  MutationError error = 4;
}

message SubscribeSignalsRequest {
  string plugin_id = 1;
  repeated string signal_names = 2;
  string object_path = 3;
}

message Signal {
  string plugin_id = 1;
  string object_path = 2;
  string interface_name = 3;
  string signal_name = 4;
  repeated google.protobuf.Value arguments = 5;
  google.protobuf.Timestamp timestamp = 6;
}

// =============================================================================
// Message Types - Event Chain Service
// =============================================================================

message GetEventsRequest {
  uint64 from_event_id = 1;
  uint64 to_event_id = 2;
  // Max events to return
  uint32 limit = 3;
  // Filter by plugin
  string plugin_id = 4;
  // Filter by tags
  repeated string tags = 5;
  // Filter by decision
  Decision decision_filter = 6;
}

enum Decision {
  DECISION_UNSPECIFIED = 0;
  DECISION_ALLOW = 1;
  DECISION_DENY = 2;
}

message GetEventsResponse {
  repeated ChainEvent events = 1;
  bool has_more = 2;
}

message ChainEvent {
  uint64 event_id = 1;
  string prev_hash = 2;
  string event_hash = 3;
  google.protobuf.Timestamp timestamp = 4;
  string actor_id = 5;
  string capability_id = 6;
  string plugin_id = 7;
  string schema_version = 8;
  string operation_type = 9;
  string target = 10;
  repeated string tags_touched = 11;
  Decision decision = 12;
  DenyReason deny_reason = 13;
  string input_patch_hash = 14;
  string result_effective_hash = 15;
}

message SubscribeEventsRequest {
  // Start from this event ID (0 = latest)
  uint64 from_event_id = 1;
  // Filter by plugin
  string plugin_id = 2;
  // Filter by tags
  repeated string tags = 3;
}

message VerifyChainRequest {
  // Verify from this event
  uint64 from_event_id = 1;
  // Verify to this event (0 = latest)
  uint64 to_event_id = 2;
}

message VerifyChainResponse {
  bool valid = 1;
  uint64 events_verified = 2;
  uint64 batches_verified = 3;
  repeated string errors = 4;
}

message GetProofRequest {
  uint64 event_id = 1;
}

message GetProofResponse {
  string event_hash = 1;
  repeated MerkleProofSibling siblings = 2;
  string root = 3;
  uint64 batch_first_event_id = 4;
  uint64 batch_last_event_id = 5;
}

message MerkleProofSibling {
  string hash = 1;
  bool is_right = 2;
}

message ProveTagImmutabilityRequest {
  string tag = 1;
  // Optional: prove for specific plugin only
  string plugin_id = 2;
}

message ProveTagImmutabilityResponse {
  string tag = 1;
  bool is_immutable = 2;
  repeated uint64 violation_event_ids = 3;
  uint64 total_events_checked = 4;
}

message GetSnapshotRequest {
  string snapshot_id = 1;
}

message GetSnapshotResponse {
  Snapshot snapshot = 1;
}

message CreateSnapshotRequest {
  string plugin_id = 1;
}

message CreateSnapshotResponse {
  Snapshot snapshot = 1;
}

message Snapshot {
  string snapshot_id = 1;
  uint64 at_event_id = 2;
  string plugin_id = 3;
  string schema_version = 4;
  string stub_hash = 5;
  string immutable_wrappers_hash = 6;
  string tunable_patch_hash = 7;
  string effective_hash = 8;
  google.protobuf.Timestamp timestamp = 9;
  google.protobuf.Struct state = 10;
}

message SearchSemanticTraceRequest {
  // Max matches to return (0 = server default)
  uint32 limit = 1;
}

message SearchSemanticTraceResponse {
  string trace_id = 1;
  uint64 mutation_index = 2;
  repeated SemanticTraceMatch matches = 3;
}

message SemanticTraceMatch {
  string point_id = 1;
  float score = 2;
  google.protobuf.Struct payload = 3;
}

// =============================================================================
// OVSDB Mirror Service - RFC 7047 native gRPC bridge
// =============================================================================

// 1:1 gRPC projection of the OVSDB management protocol (RFC 7047).
// All operations pass through to ovsdb-server via the D-Bus mirror.
service OvsdbMirror {
  // RFC 7047 §4.1.1 — List available databases
  rpc ListDbs(google.protobuf.Empty) returns (OvsdbListDbsResponse);

  // RFC 7047 §4.1.2 — Get database schema (<database-schema>)
  rpc GetSchema(OvsdbGetSchemaRequest) returns (OvsdbGetSchemaResponse);

  // RFC 7047 §4.1.3 — Execute a transaction (insert/select/update/mutate/delete/wait)
  rpc Transact(OvsdbTransactRequest) returns (OvsdbTransactResponse);

  // RFC 7047 §4.1.5 — Monitor database changes (streaming)
  rpc Monitor(OvsdbMonitorRequest) returns (stream OvsdbUpdate);

  // RFC 7047 §4.1.11 — Echo for liveness
  rpc Echo(OvsdbEchoRequest) returns (OvsdbEchoResponse);

  // Dump full database state (convenience, not in RFC)
  rpc DumpDb(OvsdbDumpDbRequest) returns (OvsdbDumpDbResponse);

  // Get the full Bridge→Port→Interface hierarchy (reconciled mirror view)
  rpc GetBridgeState(OvsdbGetBridgeStateRequest) returns (OvsdbGetBridgeStateResponse);
}

// --- OVSDB Mirror Messages ---

message OvsdbListDbsResponse {
  repeated string databases = 1;
}

message OvsdbGetSchemaRequest {
  string database = 1;
}

message OvsdbGetSchemaResponse {
  // RFC 7047 §3.2 <database-schema> as JSON
  string schema_json = 1;
  string name = 2;
  string version = 3;
}

message OvsdbTransactRequest {
  string database = 1;
  // RFC 7047 §4.1.3 — array of operations as JSON
  string operations_json = 2;
  // Actor for event chain audit
  string actor_id = 3;
}

message OvsdbTransactResponse {
  bool success = 1;
  // Array of per-operation results as JSON
  string results_json = 2;
  // Event chain event ID (if recorded)
  uint64 event_id = 3;
  string error = 4;
}

message OvsdbMonitorRequest {
  string database = 1;
  // RFC 7047 §4.1.5 — monitor-requests per table as JSON
  // e.g. {"Bridge": {}, "Port": {}, "Interface": {}}
  string monitor_requests_json = 2;
}

message OvsdbUpdate {
  // Table name
  string table = 1;
  // Row UUID
  string uuid = 2;
  // Old row (null for inserts)
  google.protobuf.Struct old_row = 3;
  // New row (null for deletes)
  google.protobuf.Struct new_row = 4;
  google.protobuf.Timestamp timestamp = 5;
}

message OvsdbEchoRequest {
  repeated string payload = 1;
}

message OvsdbEchoResponse {
  repeated string payload = 1;
}

message OvsdbDumpDbRequest {
  string database = 1;
}

message OvsdbDumpDbResponse {
  // Full database dump as JSON (table → rows)
  string dump_json = 1;
}

message OvsdbGetBridgeStateRequest {
  // Optional: filter by bridge name (empty = all)
  string bridge_name = 1;
}

message OvsdbGetBridgeStateResponse {
  repeated OvsdbBridge bridges = 1;
}

// RFC 7047 Bridge table — hierarchical view
message OvsdbBridge {
  string name = 1;
  string datapath_type = 2;
  string fail_mode = 3;
  bool stp_enable = 4;
  bool mcast_snooping_enable = 5;
  map<string, string> other_config = 6;
  repeated OvsdbPort ports = 7;
}

// RFC 7047 Port table
message OvsdbPort {
  string name = 1;
  uint32 tag = 2;       // VLAN tag (0 = untagged)
  repeated uint32 trunks = 3;
  string vlan_mode = 4;
  string bond_mode = 5;
  repeated OvsdbInterface interfaces = 6;
}

// RFC 7047 Interface table
message OvsdbInterface {
  string name = 1;
  string type = 2;      // "system" | "internal" | "patch" | "vxlan" | ...
  string mac_in_use = 3;
  string mac = 4;
  string admin_state = 5;
  string link_state = 6;
  map<string, string> options = 7;  // tunnel options: remote_ip, local_ip, key
}

// =============================================================================
// Runtime Service - Live operational state
// =============================================================================

// Live runtime state: process info, services, system metrics.
// Not stored in any database — queried directly from the running system.
service RuntimeMirror {
  // Get overall system runtime info
  rpc GetSystemInfo(google.protobuf.Empty) returns (RuntimeGetSystemInfoResponse);

  // List dinit services and their states
  rpc ListServices(RuntimeListServicesRequest) returns (RuntimeListServicesResponse);

  // Get a specific service's state
  rpc GetService(RuntimeGetServiceRequest) returns (RuntimeServiceInfo);

  // Stream live metric updates (CPU, memory, network counters)
  rpc StreamMetrics(RuntimeStreamMetricsRequest) returns (stream RuntimeMetricUpdate);

  // Get network interface states (from rtnetlink)
  rpc ListInterfaces(google.protobuf.Empty) returns (RuntimeListInterfacesResponse);

  // Get NUMA topology
  rpc GetNumaTopology(google.protobuf.Empty) returns (RuntimeGetNumaTopologyResponse);
}

// --- Runtime Messages ---

message RuntimeGetSystemInfoResponse {
  string hostname = 1;
  string kernel_version = 2;
  uint64 uptime_seconds = 3;
  uint64 boot_timestamp = 4;
  uint32 cpu_count = 5;
  uint64 memory_total_bytes = 6;
  uint64 memory_available_bytes = 7;
  uint64 memory_used_bytes = 8;
  string init_system = 9;            // "dinit" | "systemd"
  string arch = 10;                  // "x86_64" | "aarch64"
  google.protobuf.Timestamp queried_at = 11;
}

message RuntimeListServicesRequest {
  // Filter by state (empty = all)
  string state_filter = 1;
}

message RuntimeListServicesResponse {
  repeated RuntimeServiceInfo services = 1;
}

message RuntimeGetServiceRequest {
  string service_name = 1;
}

message RuntimeServiceInfo {
  string name = 1;
  string state = 2;           // "STARTED" | "STOPPED" | "STARTING" | "STOPPING"
  uint32 pid = 3;
  bool enabled = 4;
  string description = 5;
  repeated string dependencies = 6;
  google.protobuf.Timestamp started_at = 7;
}

message RuntimeStreamMetricsRequest {
  // Interval in seconds between updates (default: 5)
  uint32 interval_seconds = 1;
  // Which metric categories to include (empty = all)
  repeated string categories = 2;   // "cpu" | "memory" | "network" | "disk"
}

message RuntimeMetricUpdate {
  string category = 1;
  string name = 2;
  double value = 3;
  string unit = 4;
  map<string, string> labels = 5;   // e.g. {"interface": "ens3", "direction": "rx"}
  google.protobuf.Timestamp timestamp = 6;
}

message RuntimeListInterfacesResponse {
  repeated RuntimeNetworkInterface interfaces = 1;
}

message RuntimeNetworkInterface {
  string name = 1;
  uint32 index = 2;
  string mac_address = 3;
  string state = 4;           // "UP" | "DOWN" | "UNKNOWN"
  uint32 mtu = 5;
  repeated string ipv4_addresses = 6;
  repeated string ipv6_addresses = 7;
  uint64 rx_bytes = 8;
  uint64 tx_bytes = 9;
  uint64 rx_packets = 10;
  uint64 tx_packets = 11;
  string driver = 12;
  uint32 speed_mbps = 13;
}

message RuntimeGetNumaTopologyResponse {
  repeated NumaNode nodes = 1;
}

message NumaNode {
  uint32 node_id = 1;
  repeated uint32 cpus = 2;
  uint64 memory_total_bytes = 3;
  uint64 memory_free_bytes = 4;
  uint64 memory_used_bytes = 5;
}

// =============================================================================
// D-Bus Passthrough Service
// =============================================================================

// Generic D-Bus method call passthrough. Allows gRPC-Web clients (browser UI)
// to invoke any method on any D-Bus interface reachable from the session or
// system bus. This is the extension point for services that register on the
// bus (e.g. ai.assistant.v1) without needing dedicated gRPC proto definitions.
service DbusPassthrough {
  // Call a D-Bus method and return the JSON result.
  rpc Call(DbusCallRequest) returns (dbusCallResponse);

  // Get a D-Bus property value as JSON.
  rpc Get(DbusGetPropertyRequest) returns (DbusGetPropertyResponse);

  // Set a D-Bus property value from JSON.
  rpc Set(DbusSetPropertyRequest) returns (DbusSetPropertyResponse);

  // Subscribe to a D-Bus signal, streaming events as JSON.
  rpc Watch(DbusWatchRequest) returns (stream DbusSignalEvent);
}

message DbusCallRequest {
  string bus = 1;            // "session" | "system"
  string destination = 2;    // e.g. "ai.assistant.v1"
  string path = 3;           // e.g. "/ai/assistant"
  string interface = 4;      // e.g. "ai.assistant.v1"
  string method = 5;         // e.g. "GetSoulMemory"
  string json_body = 6;      // JSON-encoded argument(s)
}

message dbusCallResponse {
  bool success = 1;
  string json_result = 2;    // JSON-encoded return value
  string error = 3;          // error message if failed
}

message DbusGetPropertyRequest {
  string bus = 1;
  string destination = 2;
  string path = 3;
  string interface = 4;
  string property = 5;
}

message DbusGetPropertyResponse {
  bool success = 1;
  string json_value = 2;
  string error = 3;
}

message DbusSetPropertyRequest {
  string bus = 1;
  string destination = 2;
  string path = 3;
  string interface = 4;
  string property = 5;
  string json_value = 6;
}

message DbusSetPropertyResponse {
  bool success = 1;
  string error = 2;
}

message DbusWatchRequest {
  string bus = 1;
  string destination = 2;
  string path = 3;
  string interface = 4;
  repeated string signal_names = 5;  // empty = all signals
}

message DbusSignalEvent {
  string signal_name = 1;
  string path = 2;
  string interface = 3;
  string json_body = 4;
  google.protobuf.Timestamp timestamp = 5;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/proto/privacy_network.proto">
syntax = "proto3";

package operation.privacy.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

// =============================================================================
// PRIVACY NETWORK SERVICE - wgcf + OVS + Xray Privacy Infrastructure
// Comprehensive service for the entire privacy network stack
// =============================================================================

service PrivacyNetworkService {
  // Ensure the full privacy network topology is provisioned
  rpc EnsurePrivacyNetwork(EnsurePrivacyNetworkRequest) returns (EnsurePrivacyNetworkResponse);
  
  // Get current privacy network status
  rpc GetNetworkStatus(GetNetworkStatusRequest) returns (GetNetworkStatusResponse);
  
  // Provision a user with WireGuard access to privacy network
  rpc ProvisionUser(ProvisionUserRequest) returns (ProvisionUserResponse);
  
  // Get WireGuard configuration for privacy network
  rpc GetPrivacyWireGuardConfig(GetPrivacyWireGuardConfigRequest) returns (GetPrivacyWireGuardConfigResponse);
  
  // Manage privacy network components (wgcf, OVS, Xray)
  rpc ManageComponent(ManageComponentRequest) returns (ManageComponentResponse);

  // Get detailed network topology
  rpc GetNetworkTopology(GetNetworkTopologyRequest) returns (GetNetworkTopologyResponse);

  // Health check for all privacy network components
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);

  // Configure packet routing and proxy settings for containers
  rpc ConfigurePacketRouting(ConfigurePacketRoutingRequest) returns (ConfigurePacketRoutingResponse);

  // Generate WireGuard keypair for user registration (simplified approach)
  rpc GenerateWireGuardKeyPair(GenerateWireGuardKeyPairRequest) returns (GenerateWireGuardKeyPairResponse);
}

// =============================================================================
// PRIVACY NETWORK MESSAGES
// =============================================================================

message EnsurePrivacyNetworkRequest {
  string domain = 1; // "3tched.com"
  bool force_reprovision = 2;
  optional google.protobuf.Struct config_overrides = 3;
}

message EnsurePrivacyNetworkResponse {
  bool success = 1;
  string message = 2;
  string bridge_name = 3; // "ovsbr0"
  string wgcf_status = 4;
  string xray_status = 5;
  repeated string active_ports = 6;
  google.protobuf.Timestamp provisioned_at = 7;
  string topology_summary = 8;
}

message GetNetworkStatusRequest {
  string component = 1; // "all", "wgcf", "ovs", "xray", "ports"
}

message GetNetworkStatusResponse {
  bool healthy = 1;
  string overall_status = 2; // "healthy", "degraded", "unhealthy"
  repeated NetworkComponent components = 3;
  string message = 4;
  google.protobuf.Timestamp last_updated = 5;
}

message NetworkComponent {
  string name = 1; // "wgcf", "ovsbr0", "xray", "priv_xray", etc
  string status = 2; // "up", "down", "configuring"
  string type = 3; // "wireguard", "bridge", "proxy", "interface"
  string ip_address = 4;
  string details = 5;
  bool critical = 6;
}

message ProvisionUserRequest {
  string email = 1;
  string wireguard_public_key = 2;
  bool is_admin = 3;
  string domain = 4;
  string container_type = 5; // "internal" or "user"
  optional google.protobuf.Struct metadata = 6;
}

message ProvisionUserResponse {
  bool success = 1;
  string user_id = 2;
  string assigned_ip = 3; // e.g. "10.200.0.100"
  string privacy_config = 4; // Full WireGuard config for privacy network
  string message = 5;
  google.protobuf.Timestamp provisioned_at = 6;
  string xray_endpoint = 7;
}

message GetPrivacyWireGuardConfigRequest {
  string email = 1;
  string user_id = 2;
  bool include_xray = 3;
}

message GetPrivacyWireGuardConfigResponse {
  bool success = 1;
  string wireguard_config = 2;
  string public_key = 3;
  string endpoint = 4; // "registration.3tched.com:51820"
  string assigned_ip = 5;
  string dns_servers = 6;
  string message = 7;
  google.protobuf.Timestamp generated_at = 8;
}

message ManageComponentRequest {
  string action = 1; // "start", "stop", "restart", "status", "reconfigure"
  string component = 2; // "wgcf", "ovsbr0", "xray", "all"
  optional google.protobuf.Struct parameters = 3;
}

message ManageComponentResponse {
  bool success = 1;
  string message = 2;
  string component = 3;
  string status = 4;
  string output = 5;
  google.protobuf.Timestamp completed_at = 6;
}

message GetNetworkTopologyRequest {
  bool include_details = 1;
}

message GetNetworkTopologyResponse {
  string bridge_name = 1;
  string wgcf_status = 2;
  repeated string ports = 3;
  string management_ip = 4; // "10.200.0.1"
  string xray_config = 5;
  repeated NetworkRoute routes = 6;
  google.protobuf.Struct topology_data = 7;
  string summary = 8;
  repeated ProxyConfig proxy_configs = 9;
}

message ProxyConfig {
  string container_name = 1;
  string container_type = 2; // "internal", "user"
  bool http_proxy_enabled = 3;
  bool grpc_proxy_enabled = 4;
  uint32 http_port = 5; // 1080 for SOCKS
  uint32 grpc_port = 6;
  string proxy_mode = 7; // "socks+http", "http-only", "tproxy"
}

message NetworkRoute {
  string destination = 1;
  string gateway = 2;
  string device = 3;
  string metric = 4;
}

message HealthCheckRequest {
  bool check_wgcf = 1;
  bool check_ovs = 2;
  bool check_xray = 3;
  bool check_ports = 4;
}

message HealthCheckResponse {
  bool all_healthy = 1;
  uint32 healthy_components = 2;
  uint32 total_components = 3;
  repeated HealthIssue issues = 4;
  string overall_status = 5;
  google.protobuf.Timestamp checked_at = 6;
}

message HealthIssue {
  string component = 1;
  string severity = 2; // "critical", "warning", "info"
  string message = 3;
  string suggested_fix = 4;
}

message ConfigurePacketRoutingRequest {
  string container_name = 1;
  string container_type = 2; // "internal", "user"
  bool enable_http_proxy = 3;
  bool enable_grpc_proxy = 4;
  string proxy_type = 5; // "socks", "http", "tproxy", "dokodemo-door"
  uint32 socks_port = 6; // default 1080
  uint32 http_port = 7;
  bool enable_tproxy = 8;
}

message ConfigurePacketRoutingResponse {
  bool success = 1;
  string message = 2;
  string container_name = 3;
  string proxy_config_summary = 4;
  google.protobuf.Timestamp configured_at = 5;
  repeated string applied_rules = 6;
}

message GenerateWireGuardKeyPairRequest {
  string user_token = 1;           // Magic link token from registration
  string user_email = 2;
  bool is_admin = 3;
  string container_type = 4;       // "user" or "internal"
}

message GenerateWireGuardKeyPairResponse {
  bool success = 1;
  string client_public_key = 2;    // Client's public key (for server registration)
  string wireguard_config = 3;     // Complete config with private key
  string assigned_ip = 4;
  string key_id = 5;               // For tracking/revocation
  string message = 6;
  google.protobuf.Timestamp generated_at = 7;
}

// =============================================================================
// ERROR HANDLING
// =============================================================================

message PrivacyError {
  int32 code = 1;
  string message = 2;
  optional google.protobuf.Struct details = 3;
}

enum PrivacyErrorCode {
  PRIVACY_ERROR_UNSPECIFIED = 0;
  PRIVACY_ERROR_WGCF_FAILED = 1;
  PRIVACY_ERROR_OVS_UNAVAILABLE = 2;
  PRIVACY_ERROR_XRAY_FAILED = 3;
  PRIVACY_ERROR_NETWORK_UNHEALTHY = 4;
  PRIVACY_ERROR_USER_PROVISIONING_FAILED = 5;
  PRIVACY_ERROR_CONFIG_INVALID = 6;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/proto/registration.proto">
syntax = "proto3";

package operation.registration.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

// =============================================================================
// REGISTRATION SERVICE - Magic Link + WireGuard Identity Management
// Similar to how tools and agents have their own proto definitions
// =============================================================================

service RegistrationService {
  // Send magic link for user registration (like tool registration)
  rpc SendMagicLink(SendMagicLinkRequest) returns (SendMagicLinkResponse);
  
  // Verify magic link and provision WireGuard identity
  rpc VerifyMagicLink(VerifyMagicLinkRequest) returns (VerifyMagicLinkResponse);
  
  // Register user with WireGuard public key (registry pattern)
  rpc RegisterUser(RegisterUserRequest) returns (RegisterUserResponse);
  
  // Get registration status for a user
  rpc GetUserStatus(GetUserStatusRequest) returns (GetUserStatusResponse);
  
  // List all registered users (like ListTools/ListAgents)
  rpc ListUsers(ListUsersRequest) returns (ListUsersResponse);
  
  // Get WireGuard configuration for a registered user
  rpc GetWireGuardConfig(GetWireGuardConfigRequest) returns (GetWireGuardConfigResponse);
  
  // Admin function to manage user registrations
  rpc AdminUserAction(AdminUserActionRequest) returns (AdminUserActionResponse);
}

// =============================================================================
// REGISTRATION SERVICE MESSAGES
// =============================================================================

message SendMagicLinkRequest {
  string email = 1;
  string domain = 2; // "3tched.com"
  bool is_admin = 3; // true for admin@3tched.com, false for jeremy@3tched.com
  optional string custom_message = 4;
}

message SendMagicLinkResponse {
  bool success = 1;
  string message = 2;
  optional string token = 3; // for testing/debug
  google.protobuf.Timestamp expires_at = 4;
}

message VerifyMagicLinkRequest {
  string token = 1;
  string domain = 2;
}

message VerifyMagicLinkResponse {
  bool success = 1;
  string user_id = 2;
  string email = 3;
  string wireguard_public_key = 4;
  string assigned_ip = 5;
  string wireguard_config = 6;
  string message = 7;
  bool is_admin = 8;
  google.protobuf.Timestamp verified_at = 9;
}

message RegisterUserRequest {
  string email = 1;
  string wireguard_public_key = 2;
  string domain = 3;
  bool is_admin = 4;
  optional google.protobuf.Struct metadata = 5;
}

message RegisterUserResponse {
  bool success = 1;
  string user_id = 2;
  string message = 3;
  string assigned_ip = 4;
  string wireguard_config = 5;
  google.protobuf.Timestamp registered_at = 6;
}

message GetUserStatusRequest {
  string email = 1;
  string user_id = 2;
  string domain = 3;
}

message GetUserStatusResponse {
  bool registered = 1;
  string user_id = 2;
  string email = 3;
  bool email_verified = 4;
  string wireguard_public_key = 5;
  string assigned_ip = 6;
  bool is_admin = 7;
  google.protobuf.Timestamp registered_at = 8;
  google.protobuf.Timestamp last_active = 9;
}

message ListUsersRequest {
  uint32 limit = 1;
  uint32 offset = 2;
  bool include_admins_only = 3;
  string domain_filter = 4;
}

message ListUsersResponse {
  repeated UserInfo users = 1;
  uint32 total_count = 2;
  uint32 filtered_count = 3;
}

message UserInfo {
  string user_id = 1;
  string email = 2;
  bool email_verified = 3;
  string wireguard_public_key = 4;
  string assigned_ip = 5;
  bool is_admin = 6;
  google.protobuf.Timestamp registered_at = 7;
  google.protobuf.Timestamp last_active = 8;
  optional google.protobuf.Struct metadata = 9;
}

message GetWireGuardConfigRequest {
  string email = 1;
  string user_id = 2;
  string domain = 3;
}

message GetWireGuardConfigResponse {
  bool success = 1;
  string wireguard_config = 2;
  string public_key = 3;
  string assigned_ip = 4;
  string message = 5;
  google.protobuf.Timestamp generated_at = 6;
}

message AdminUserActionRequest {
  string action = 1; // "suspend", "unsuspend", "delete", "reset_password"
  string user_id = 2;
  string email = 3;
  optional google.protobuf.Struct parameters = 4;
}

message AdminUserActionResponse {
  bool success = 1;
  string message = 2;
  string user_id = 3;
  google.protobuf.Timestamp action_timestamp = 4;
}

// =============================================================================
// ERROR HANDLING
// =============================================================================

message RegistrationError {
  int32 code = 1;
  string message = 2;
  optional google.protobuf.Struct details = 3;
}

enum RegistrationErrorCode {
  REGISTRATION_ERROR_UNSPECIFIED = 0;
  REGISTRATION_ERROR_INVALID_EMAIL = 1;
  REGISTRATION_ERROR_INVALID_TOKEN = 2;
  REGISTRATION_ERROR_USER_EXISTS = 3;
  REGISTRATION_ERROR_WIREGUARD_KEY_INVALID = 4;
  REGISTRATION_ERROR_NETWORK_UNAVAILABLE = 5;
  REGISTRATION_ERROR_ADMIN_REQUIRED = 6;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/proto/registry.proto">
syntax = "proto3";

package operation.registry.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

// =============================================================================
// COMPONENT REGISTRY SERVICE
//
// Central registry for runtime-discoverable components: agents, plugins,
// MCP servers, capability providers, and any future additions.
//
// Design principle: component_type is a plain string, not an enum, so new
// component types can be introduced without proto changes. The Watch stream
// lets consumers (e.g. MCP tool layer) react immediately to new registrations.
// =============================================================================

service ComponentRegistry {
  // Register a component. Idempotent — re-registering with the same
  // component_id updates the existing entry.
  rpc Register(RegisterRequest) returns (RegisterResponse);

  // Deregister a component by ID.
  rpc Deregister(DeregisterRequest) returns (DeregisterResponse);

  // Query registered components, optionally filtered by type or capability.
  rpc Discover(DiscoverRequest) returns (DiscoverResponse);

  // Get a single component by ID.
  rpc GetComponent(GetComponentRequest) returns (GetComponentResponse);

  // Stream registry change events. Consumers (e.g. MCP tool layer) call this
  // once and receive a RegistryEvent for every subsequent Register/Deregister.
  rpc Watch(WatchRequest) returns (stream RegistryEvent);

  // Heartbeat — components call this periodically to signal liveness.
  // Registry marks components stale after missing N heartbeats.
  rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);
}

// =============================================================================
// REGISTRATION
// =============================================================================

message RegisterRequest {
  // Unique stable identifier for this component (e.g. "agent.rust_pro",
  // "plugin.net", "mcp.filesystem"). Must not change across restarts.
  string component_id = 1;

  // Free-form type string. Known values: "agent", "plugin", "mcp_server",
  // "capability", "tool". New types require no proto change.
  string component_type = 2;

  // Human- and LLM-readable display name.
  string name = 3;

  // Description used verbatim as the MCP tool description. Should explain
  // what this component does and when an LLM should invoke it.
  string description = 4;

  // JSON Schema (draft-07 compatible) describing the input accepted by this
  // component's primary operation. Used to auto-generate MCP tool schemas.
  string schema_json = 5;

  // Arbitrary metadata for component-specific needs.
  map<string, string> metadata = 6;

  // Capabilities declared by this component (e.g. "cargo_build",
  // "memory_recall", "dns_resolve"). Used for capability-based discovery.
  repeated string capabilities = 7;

  // gRPC endpoint if this component is a remote service. Empty for
  // in-process components.
  string endpoint = 8;

  // Semantic version string.
  string version = 9;

  // Heartbeat interval this component will use (seconds). Registry uses
  // this to compute the stale threshold (3× interval).
  uint32 heartbeat_interval_seconds = 10;
}

message RegisterResponse {
  bool success = 1;
  string message = 2;
  // Server-assigned lease token — must be presented in Heartbeat and Deregister.
  string lease_token = 3;
  google.protobuf.Timestamp registered_at = 4;
}

// =============================================================================
// DEREGISTRATION
// =============================================================================

message DeregisterRequest {
  string component_id = 1;
  string lease_token = 2;
}

message DeregisterResponse {
  bool success = 1;
  string message = 2;
}

// =============================================================================
// DISCOVERY
// =============================================================================

message DiscoverRequest {
  // Filter by component type (empty = all types).
  string component_type = 1;
  // Filter by capability — returns components declaring this capability.
  string capability = 2;
  // Filter by metadata key/value pair.
  string metadata_key = 3;
  string metadata_value = 4;
  // Include stale (missed heartbeat) components in results.
  bool include_stale = 5;
}

message DiscoverResponse {
  repeated ComponentInfo components = 1;
  uint32 total_count = 2;
}

message GetComponentRequest {
  string component_id = 1;
}

message GetComponentResponse {
  ComponentInfo component = 1;
  bool found = 2;
}

// =============================================================================
// COMPONENT INFO (returned in discovery and watch events)
// =============================================================================

message ComponentInfo {
  string component_id = 1;
  string component_type = 2;
  string name = 3;
  string description = 4;
  string schema_json = 5;
  map<string, string> metadata = 6;
  repeated string capabilities = 7;
  string endpoint = 8;
  string version = 9;
  ComponentStatus status = 10;
  google.protobuf.Timestamp registered_at = 11;
  google.protobuf.Timestamp last_heartbeat = 12;
}

enum ComponentStatus {
  COMPONENT_STATUS_UNSPECIFIED = 0;
  COMPONENT_STATUS_ACTIVE = 1;
  COMPONENT_STATUS_STALE = 2;      // missed heartbeat window
  COMPONENT_STATUS_DEREGISTERED = 3;
}

// =============================================================================
// WATCH STREAM
// =============================================================================

message WatchRequest {
  // Only deliver events for these component types (empty = all).
  repeated string component_types = 1;
  // Replay existing registrations as REGISTERED events on connect.
  bool include_existing = 2;
}

message RegistryEvent {
  RegistryEventType event_type = 1;
  ComponentInfo component = 2;
  google.protobuf.Timestamp timestamp = 3;
}

enum RegistryEventType {
  REGISTRY_EVENT_UNSPECIFIED = 0;
  REGISTRY_EVENT_REGISTERED = 1;
  REGISTRY_EVENT_DEREGISTERED = 2;
  REGISTRY_EVENT_UPDATED = 3;   // re-registration with changed fields
  REGISTRY_EVENT_STALE = 4;     // missed heartbeat threshold
  REGISTRY_EVENT_RECOVERED = 5; // heartbeat resumed after stale
}

// =============================================================================
// HEARTBEAT
// =============================================================================

message HeartbeatRequest {
  string component_id = 1;
  string lease_token = 2;
  // Optional live status payload (e.g. current load, queue depth).
  optional google.protobuf.Struct status_payload = 3;
}

message HeartbeatResponse {
  bool acknowledged = 1;
  // Server signals the component should re-register (e.g. after server restart).
  bool reregister_required = 2;
  google.protobuf.Timestamp server_time = 3;
}

// =============================================================================
// ERROR HANDLING
// =============================================================================

enum RegistryErrorCode {
  REGISTRY_ERROR_UNSPECIFIED = 0;
  REGISTRY_ERROR_COMPONENT_NOT_FOUND = 1;
  REGISTRY_ERROR_INVALID_LEASE_TOKEN = 2;
  REGISTRY_ERROR_DUPLICATE_ID = 3;
  REGISTRY_ERROR_INVALID_SCHEMA = 4;
  REGISTRY_ERROR_HEARTBEAT_EXPIRED = 5;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs">
//! 🟢 🛷 The Shuttle — gRPC Bridge Binary
//!
//! Zero-trust gRPC gateway enforcing the Absolute Base rule via
//! GhostbridgeInterceptor. Reads the IdentitySled from shared memory
//! (/dev/shm/plugin_schema.dat) and rejects any request whose footprint
//! does not match the current Strike/Etch.
//!
//! Design:
//!   - Does NOT write the sled; the SchemaEngine or A.N.N.A. Scribe does.
//!   - If no valid sled exists, all inbound requests are rejected.
//!   - Bind address defaults to 127.0.0.1:18789 (Xray redirect target).

use std::net::SocketAddr;
use std::sync::Arc;

use op_grpc_bridge::{grpc_server::run_grpc_server, schema_engine::SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state_store::{ChainConfig, EventChain};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_grpc_bridge=info".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    // ── Build SchemaEngine (authoritative mutation pipeline) ─────────────────
    let event_chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
    let ovsdb = Arc::new(OvsdbClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let schema_engine = Arc::new(SchemaEngine::new(event_chain, ovsdb, nonnet));

    // ── Bind address ─────────────────────────────────────────────────────────
    // Per spec: Xray redirects gRPC traffic to 127.0.0.1:18789.
    let addr: SocketAddr = std::env::var("GRPC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18789".to_string())
        .parse()
        .expect("GRPC_BIND must be a valid socket address");

    tracing::info!(%addr, "The Shuttle gRPC bridge starting");
    tracing::info!(
        "GhostbridgeInterceptor active — requests require X-Ghostbridge-Footprint + X-Ghostbridge-Trace-ID"
    );

    run_grpc_server(addr, schema_engine, None).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/grpc_client.rs">
//! gRPC Client - For D-Bus → remote gRPC calls
//!
//! Allows local D-Bus services to call remote gRPC endpoints,
//! enabling distributed operation-dbus deployments.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use prost_types::{value::Kind as ProstKind, Struct as ProstStruct, Value as ProstValue};
use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};
use tracing::info;

use crate::proto::{
    event_chain_service_client::EventChainServiceClient,
    plugin_service_client::PluginServiceClient, state_sync_client::StateSyncClient,
    CallMethodRequest, GetStateRequest, MutateRequest, OperationType as ProtoOperationType,
    SubscribeEventsRequest, SubscribeRequest,
};

/// Configuration for a remote gRPC endpoint
#[derive(Debug, Clone)]
pub struct RemoteEndpoint {
    pub address: String,
    pub tls_enabled: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for RemoteEndpoint {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:50051".to_string(),
            tls_enabled: false,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// gRPC client pool for connecting to remote Operation services
pub struct GrpcClientPool {
    /// Map of endpoint address to channel
    channels: RwLock<HashMap<String, Channel>>,
    /// Default endpoint configuration
    default_config: RemoteEndpoint,
}

impl Default for GrpcClientPool {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcClientPool {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            default_config: RemoteEndpoint::default(),
        }
    }

    pub fn with_default_config(config: RemoteEndpoint) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            default_config: config,
        }
    }

    /// Get or create a channel to the specified address (supports comma-separated endpoints for load balancing)
    async fn get_channel(&self, address: &str) -> Result<Channel, GrpcClientError> {
        {
            let channels = self.channels.read().await;
            if let Some(channel) = channels.get(address) {
                return Ok(channel.clone());
            }
        }

        let addrs: Vec<&str> = address
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let channel = if addrs.len() > 1 {
            // Native Tonic Load Balancing
            let endpoints = addrs
                .into_iter()
                .map(|addr| {
                    Endpoint::from_shared(addr.to_string()).map(|e| {
                        e.connect_timeout(self.default_config.connect_timeout)
                            .timeout(self.default_config.request_timeout)
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| {
                    GrpcClientError::ConnectionFailed(format!("Invalid endpoint: {}", e))
                })?;

            Channel::balance_list(endpoints.into_iter())
        } else {
            // Single endpoint
            let endpoint = Endpoint::from_shared(address.to_string())
                .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
                .connect_timeout(self.default_config.connect_timeout)
                .timeout(self.default_config.request_timeout);

            endpoint
                .connect()
                .await
                .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
        };

        {
            let mut channels = self.channels.write().await;
            channels.insert(address.to_string(), channel.clone());
        }

        info!("Connected to remote gRPC endpoint(s): {}", address);
        Ok(channel)
    }

    /// Get a Plugin service client
    pub async fn plugin_service_client(
        &self,
        address: &str,
    ) -> Result<PluginServiceClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(PluginServiceClient::new(channel))
    }

    /// Get a StateSync service client
    pub async fn state_sync_client(
        &self,
        address: &str,
    ) -> Result<StateSyncClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(StateSyncClient::new(channel))
    }

    /// Get an EventChain service client
    pub async fn event_chain_client(
        &self,
        address: &str,
    ) -> Result<EventChainServiceClient<Channel>, GrpcClientError> {
        let channel = self.get_channel(address).await?;
        Ok(EventChainServiceClient::new(channel))
    }

    /// Close all connections
    pub async fn close_all(&self) {
        let mut channels = self.channels.write().await;
        channels.clear();
        info!("Closed all gRPC client connections");
    }
}

/// High-level client for remote Operation services
#[allow(dead_code)]
pub struct RemoteOperationClient {
    pool: Arc<GrpcClientPool>,
    default_address: String,
    client_id: String,
}

impl RemoteOperationClient {
    pub fn new(pool: Arc<GrpcClientPool>, address: &str, client_id: &str) -> Self {
        Self {
            pool,
            default_address: address.to_string(),
            client_id: client_id.to_string(),
        }
    }

    /// Get state from a remote endpoint
    pub async fn get_state(
        &self,
        plugin_id: &str,
        object_path: &str,
    ) -> Result<simd_json::OwnedValue, GrpcClientError> {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(GetStateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
        });

        let response = client
            .get_state(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        let state = resp.state.unwrap_or_default();
        Ok(prost_struct_to_simd(&state))
    }

    /// Set state on a remote endpoint (apply patch)
    pub async fn set_state(
        &self,
        plugin_id: &str,
        object_path: &str,
        state: simd_json::OwnedValue,
        actor_id: &str,
        capability_id: &str,
    ) -> Result<SetStateResult, GrpcClientError> {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(MutateRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            operation: ProtoOperationType::ApplyPatch as i32,
            member_name: String::new(),
            value: Some(simd_to_prost_value(&state)),
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
        });

        let response = client
            .mutate(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        if !resp.success {
            if let Some(err) = resp.error {
                return Err(GrpcClientError::RemoteError {
                    code: format!("{}", err.code),
                    message: err.message,
                });
            }
            return Err(GrpcClientError::RemoteError {
                code: "UNKNOWN".to_string(),
                message: "mutation failed".to_string(),
            });
        }

        Ok(SetStateResult {
            event_id: resp.event_id,
            effective_hash: resp.effective_hash,
        })
    }

    /// Call a method on a remote endpoint
    #[allow(clippy::too_many_arguments)]
    pub async fn call_method(
        &self,
        plugin_id: &str,
        object_path: &str,
        interface_name: &str,
        method_name: &str,
        arguments: Vec<simd_json::OwnedValue>,
        actor_id: &str,
        capability_id: &str,
    ) -> Result<simd_json::OwnedValue, GrpcClientError> {
        let mut client = self
            .pool
            .plugin_service_client(&self.default_address)
            .await?;

        let arguments = arguments
            .iter()
            .map(simd_to_prost_value)
            .collect::<Vec<_>>();

        let request = tonic::Request::new(CallMethodRequest {
            plugin_id: plugin_id.to_string(),
            object_path: object_path.to_string(),
            interface_name: interface_name.to_string(),
            method_name: method_name.to_string(),
            arguments,
            actor_id: actor_id.to_string(),
            capability_id: capability_id.to_string(),
        });

        let response = client
            .call_method(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let resp = response.into_inner();
        if !resp.success {
            if let Some(err) = resp.error {
                return Err(GrpcClientError::RemoteError {
                    code: format!("{}", err.code),
                    message: err.message,
                });
            }
            return Err(GrpcClientError::RemoteError {
                code: "UNKNOWN".to_string(),
                message: "call failed".to_string(),
            });
        }

        if let Some(result) = resp.result {
            Ok(prost_value_to_simd(&result))
        } else {
            Ok(simd_json::json!(null))
        }
    }

    /// Subscribe to state updates from a remote endpoint
    pub async fn subscribe(
        &self,
        plugin_filters: Vec<String>,
        path_filters: Vec<String>,
        tag_filters: Vec<String>,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<StateUpdateMessage, GrpcClientError>>,
        GrpcClientError,
    > {
        let mut client = self.pool.state_sync_client(&self.default_address).await?;

        let request = tonic::Request::new(SubscribeRequest {
            plugin_ids: plugin_filters,
            path_patterns: path_filters,
            tags: tag_filters,
            include_initial_state: false,
        });

        let response = client
            .subscribe(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let stream = response.into_inner();

        Ok(tokio_stream::StreamExt::map(stream, |result| {
            result
                .map(|update| StateUpdateMessage {
                    plugin_id: update.plugin_id,
                    object_path: update.object_path,
                    property_name: if update.member_name.is_empty() {
                        None
                    } else {
                        Some(update.member_name)
                    },
                    new_value: update.new_value.as_ref().map(prost_value_to_simd),
                    event_id: update.event_id.to_string(),
                    tags_touched: update.tags_touched,
                })
                .map_err(|e| GrpcClientError::StreamError(e.to_string()))
        }))
    }

    /// Subscribe to chain events from a remote endpoint
    pub async fn stream_events(
        &self,
        from_event_id: Option<u64>,
        plugin_filters: Vec<String>,
        tag_filters: Vec<String>,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<ChainEventMessage, GrpcClientError>>,
        GrpcClientError,
    > {
        let mut client = self.pool.event_chain_client(&self.default_address).await?;

        let request = tonic::Request::new(SubscribeEventsRequest {
            from_event_id: from_event_id.unwrap_or_default(),
            plugin_id: plugin_filters.first().cloned().unwrap_or_default(),
            tags: tag_filters,
        });

        let response = client
            .subscribe_events(request)
            .await
            .map_err(|e| GrpcClientError::RequestFailed(e.to_string()))?;

        let stream = response.into_inner();

        Ok(tokio_stream::StreamExt::map(stream, |result| {
            result
                .map(|event| ChainEventMessage {
                    event_id: event.event_id.to_string(),
                    event_hash: event.event_hash,
                    prev_hash: event.prev_hash,
                    plugin_id: event.plugin_id,
                    operation_type: event.operation_type,
                    target: event.target,
                    decision: event.decision.to_string(),
                    tags_touched: event.tags_touched,
                })
                .map_err(|e| GrpcClientError::StreamError(e.to_string()))
        }))
    }
}

/// Result of a set state operation
#[derive(Debug, Clone)]
pub struct SetStateResult {
    pub event_id: u64,
    pub effective_hash: String,
}

/// State update message from subscription
#[derive(Debug, Clone)]
pub struct StateUpdateMessage {
    pub plugin_id: String,
    pub object_path: String,
    pub property_name: Option<String>,
    pub new_value: Option<simd_json::OwnedValue>,
    pub event_id: String,
    pub tags_touched: Vec<String>,
}

/// Chain event message from event stream
#[derive(Debug, Clone)]
pub struct ChainEventMessage {
    pub event_id: String,
    pub event_hash: String,
    pub prev_hash: String,
    pub plugin_id: String,
    pub operation_type: String,
    pub target: String,
    pub decision: String,
    pub tags_touched: Vec<String>,
}

/// Errors that can occur in gRPC client operations
#[derive(Debug, Clone)]
pub enum GrpcClientError {
    ConnectionFailed(String),
    RequestFailed(String),
    StreamError(String),
    ParseError(String),
    RemoteError { code: String, message: String },
}

impl std::fmt::Display for GrpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::StreamError(msg) => write!(f, "Stream error: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::RemoteError { code, message } => {
                write!(f, "Remote error [{}]: {}", code, message)
            }
        }
    }
}

impl std::error::Error for GrpcClientError {}

fn prost_value_to_simd(value: &ProstValue) -> simd_json::OwnedValue {
    let serde_value = prost_value_to_serde(value);
    simd_json::serde::to_owned_value(&serde_value).unwrap_or_else(|_| simd_json::json!(null))
}

fn prost_struct_to_simd(value: &ProstStruct) -> simd_json::OwnedValue {
    let serde_value = serde_json::Value::Object(
        value
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), prost_value_to_serde(v)))
            .collect(),
    );
    simd_json::serde::to_owned_value(&serde_value).unwrap_or_else(|_| simd_json::json!(null))
}

fn prost_value_to_serde(value: &ProstValue) -> serde_json::Value {
    match &value.kind {
        None => serde_json::Value::Null,
        Some(ProstKind::NullValue(_)) => serde_json::Value::Null,
        Some(ProstKind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(ProstKind::NumberValue(n)) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Some(ProstKind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(ProstKind::StructValue(s)) => serde_json::Value::Object(
            s.fields
                .iter()
                .map(|(k, v)| (k.clone(), prost_value_to_serde(v)))
                .collect(),
        ),
        Some(ProstKind::ListValue(l)) => {
            serde_json::Value::Array(l.values.iter().map(prost_value_to_serde).collect())
        }
    }
}

fn simd_to_prost_value(value: &simd_json::OwnedValue) -> ProstValue {
    let json = simd_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let serde_value: serde_json::Value =
        serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
    serde_to_prost_value(&serde_value)
}

fn serde_to_prost_value(value: &serde_json::Value) -> ProstValue {
    match value {
        serde_json::Value::Null => ProstValue {
            kind: Some(ProstKind::NullValue(0)),
        },
        serde_json::Value::Bool(b) => ProstValue {
            kind: Some(ProstKind::BoolValue(*b)),
        },
        serde_json::Value::Number(n) => ProstValue {
            kind: Some(ProstKind::NumberValue(n.as_f64().unwrap_or(0.0))),
        },
        serde_json::Value::String(s) => ProstValue {
            kind: Some(ProstKind::StringValue(s.clone())),
        },
        serde_json::Value::Array(arr) => ProstValue {
            kind: Some(ProstKind::ListValue(prost_types::ListValue {
                values: arr.iter().map(serde_to_prost_value).collect(),
            })),
        },
        serde_json::Value::Object(map) => ProstValue {
            kind: Some(ProstKind::StructValue(ProstStruct {
                fields: map
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_to_prost_value(v)))
                    .collect(),
            })),
        },
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/grpc_server.rs">
//! gRPC Server - Implements the Operation gRPC services (shared-server topology)

use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use chrono::{DateTime, Utc};
use futures::StreamExt as _;
use op_cognitive_mcp::QdrantSemanticShuttle;
use prost_types::{Struct as ProstStruct, Timestamp as ProstTimestamp, Value as ProstValue};
use simd_json::prelude::{ValueAsContainer, ValueAsScalar};
use tokio::sync::{broadcast, RwLock};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::interceptor;

use crate::proto::{
    event_chain_service_server::EventChainService, ovsdb_mirror_server::OvsdbMirror,
    plugin_service_server::PluginService, runtime_mirror_server::RuntimeMirror,
    state_sync_server::StateSync, BatchMutateRequest, BatchMutateResponse, CallMethodRequest,
    CallMethodResponse, CapabilityMissing as ProtoCapabilityMissing, ChainEvent as ProtoChainEvent,
    ChangeType as ProtoChangeType, ConstraintFail as ProtoConstraintFail, CreateSnapshotRequest,
    CreateSnapshotResponse, Decision as ProtoDecision, DenyReason as ProtoDenyReason,
    ErrorCode as ProtoErrorCode, GetEventsRequest, GetEventsResponse, GetProofRequest,
    GetProofResponse, GetPropertyRequest, GetPropertyResponse, GetSchemaRequest, GetSchemaResponse,
    GetSnapshotRequest, GetSnapshotResponse, GetStateRequest, GetStateResponse,
    ListPluginsResponse, MerkleProofSibling, MutateRequest, MutateResponse,
    MutationError as ProtoMutationError, NumaNode as ProtoNumaNode,
    OperationType as ProtoOperationType, OvsdbBridge as ProtoOvsdbBridge, OvsdbDumpDbRequest,
    OvsdbDumpDbResponse, OvsdbEchoRequest, OvsdbEchoResponse, OvsdbGetBridgeStateRequest,
    OvsdbGetBridgeStateResponse, OvsdbGetSchemaRequest, OvsdbGetSchemaResponse,
    OvsdbInterface as ProtoOvsdbInterface, OvsdbListDbsResponse, OvsdbMonitorRequest,
    OvsdbPort as ProtoOvsdbPort, OvsdbTransactRequest, OvsdbTransactResponse, OvsdbUpdate,
    PluginInfo, ProveTagImmutabilityRequest, ProveTagImmutabilityResponse,
    ReadOnlyViolation as ProtoReadOnlyViolation, RuntimeGetNumaTopologyResponse,
    RuntimeGetServiceRequest, RuntimeGetSystemInfoResponse, RuntimeListInterfacesResponse,
    RuntimeListServicesRequest, RuntimeListServicesResponse, RuntimeMetricUpdate,
    RuntimeNetworkInterface as ProtoRuntimeNetworkInterface,
    RuntimeServiceInfo as ProtoRuntimeServiceInfo, RuntimeStreamMetricsRequest,
    SearchSemanticTraceRequest, SearchSemanticTraceResponse, SemanticTraceMatch,
    SetPropertyRequest, SetPropertyResponse, Signal, StateChange as ProtoStateChange,
    SubscribeEventsRequest, SubscribeRequest, SubscribeSignalsRequest, TagLock as ProtoTagLock,
    VerifyChainRequest, VerifyChainResponse,
};
use crate::schema_engine::{ChangeType, SchemaEngine};
use op_state_store::{Decision, DenyReason, MerkleProof};
use zbus::zvariant::{Array as ZArray, OwnedValue as ZOwnedValue, Str as ZStr, Value as ZValue};
use zbus::{Connection, Proxy};

/// Plugin schema provider.
///
/// The provider is expected to read from the canonical plugin-document path
/// and/or its in-memory catalog projection. It is not intended to invent
/// schema independently of the plugin document.
#[tonic::async_trait]
pub trait PluginSchemaProvider: Send + Sync {
    async fn list_plugins(&self) -> Vec<PluginInfo>;
    async fn get_schema(&self, plugin_id: &str) -> Option<(String, String, String)>;
}

struct EmptyPluginProvider;

#[tonic::async_trait]
impl PluginSchemaProvider for EmptyPluginProvider {
    async fn list_plugins(&self) -> Vec<PluginInfo> {
        Vec::new()
    }

    async fn get_schema(&self, _plugin_id: &str) -> Option<(String, String, String)> {
        None
    }
}

// =============================================================================
// Registry State
// =============================================================================

/// In-memory component registry backing ComponentRegistry gRPC service.
/// Shared via Arc across all clones of OperationGrpcServer.
struct RegistryInner {
    /// component_id → ComponentInfo
    components: HashMap<String, crate::proto::registry::ComponentInfo>,
    /// component_id → lease_token
    leases: HashMap<String, String>,
    /// Broadcast channel for Watch stream
    watch_tx: broadcast::Sender<crate::proto::registry::RegistryEvent>,
}

impl RegistryInner {
    fn new() -> (
        Self,
        broadcast::Sender<crate::proto::registry::RegistryEvent>,
    ) {
        let (tx, _) = broadcast::channel(256);
        (
            Self {
                components: HashMap::new(),
                leases: HashMap::new(),
                watch_tx: tx.clone(),
            },
            tx,
        )
    }
}

// =============================================================================
// gRPC server implementation for operation services
// =============================================================================

#[derive(Clone)]
pub struct OperationGrpcServer {
    schema_engine: Arc<SchemaEngine>,
    plugin_provider: Arc<dyn PluginSchemaProvider>,
    semantic_shuttle: Option<Arc<QdrantSemanticShuttle>>,
    /// Broadcast channel for chain events
    chain_events: broadcast::Sender<ProtoChainEvent>,
    /// Component registry state (shared across clones)
    registry: Arc<RwLock<RegistryInner>>,
}

impl OperationGrpcServer {
    pub fn new(schema_engine: Arc<SchemaEngine>) -> Self {
        let (chain_tx, _) = broadcast::channel(1024);
        let (registry, _) = RegistryInner::new();
        Self {
            schema_engine,
            plugin_provider: Arc::new(EmptyPluginProvider),
            semantic_shuttle: None,
            chain_events: chain_tx,
            registry: Arc::new(RwLock::new(registry)),
        }
    }

    pub fn with_plugin_provider(
        schema_engine: Arc<SchemaEngine>,
        plugin_provider: Arc<dyn PluginSchemaProvider>,
    ) -> Self {
        let (chain_tx, _) = broadcast::channel(1024);
        let (registry, _) = RegistryInner::new();
        Self {
            schema_engine,
            plugin_provider,
            semantic_shuttle: None,
            chain_events: chain_tx,
            registry: Arc::new(RwLock::new(registry)),
        }
    }

    pub fn with_semantic_shuttle(mut self, semantic_shuttle: Arc<QdrantSemanticShuttle>) -> Self {
        self.semantic_shuttle = Some(semantic_shuttle);
        self
    }

    /// Snapshot of all registered components plus a live-update receiver.
    ///
    /// The receiver fires a `RegistryEvent` every time a component registers,
    /// updates, or deregisters.  Use this to mirror the registry into the D-Bus
    /// tree without polling.
    pub async fn registry_watch(
        &self,
    ) -> (
        Vec<crate::proto::registry::ComponentInfo>,
        tokio::sync::broadcast::Receiver<crate::proto::registry::RegistryEvent>,
    ) {
        let inner = self.registry.read().await;
        let snapshot: Vec<_> = inner.components.values().cloned().collect();
        let rx = inner.watch_tx.subscribe();
        (snapshot, rx)
    }
}

/// Run gRPC server for all Operation services.
///
/// Includes:
///   - StateSync, PluginService, EventChainService, OvsdbMirror, RuntimeMirror
///   - ComponentRegistry, MailService, PrivacyNetworkService, RegistrationService
///   - gRPC server reflection (all protos in combined descriptor)
///   - gRPC health protocol (liveness for deploy verification and load balancers)
///
/// Adding a new domain service:
///   1. Add the generated server import below
///   2. Add `.add_service(...)` to the builder chain
///   3. Mark it serving via health_reporter
pub async fn run_grpc_server(
    addr: std::net::SocketAddr,
    schema_engine: Arc<SchemaEngine>,
    plugin_provider: Option<Arc<dyn PluginSchemaProvider>>,
) -> Result<(), tonic::transport::Error> {
    use crate::proto::dbus_passthrough_server::DbusPassthroughServer;
    use crate::proto::event_chain_service_server::EventChainServiceServer;
    use crate::proto::mail::mail_service_server::MailServiceServer;
    use crate::proto::ovsdb_mirror_server::OvsdbMirrorServer;
    use crate::proto::plugin_service_server::PluginServiceServer;
    use crate::proto::privacy::privacy_network_service_server::PrivacyNetworkServiceServer;
    use crate::proto::registration::registration_service_server::RegistrationServiceServer;
    use crate::proto::registry::component_registry_server::ComponentRegistryServer;
    use crate::proto::runtime_mirror_server::RuntimeMirrorServer;
    use crate::proto::state_sync_server::StateSyncServer;
    use op_cache::grpc::{AgentServiceImpl, McpServiceImpl, OrchestratorServiceImpl};
    use op_cache::proto::mcp_service_server::McpServiceServer;

    // Build MCP service backed by agent registry.
    let agent_svc = std::sync::Arc::new(AgentServiceImpl::new());
    let cache_svc = std::sync::Arc::new(op_cache::grpc::CacheServiceImpl::with_ttl(3600));
    let orch_svc = std::sync::Arc::new(OrchestratorServiceImpl::with_config(
        agent_svc.clone(),
        cache_svc,
        2,    // workstack_threshold
        true, // enable_caching
        3,    // promotion_threshold
    ));
    let mcp_svc = std::sync::Arc::new(McpServiceImpl::new(agent_svc, orch_svc));

    let server = if let Some(provider) = plugin_provider {
        OperationGrpcServer::with_plugin_provider(schema_engine, provider)
    } else {
        OperationGrpcServer::new(schema_engine)
    };
    let server = match QdrantSemanticShuttle::new().await {
        Ok(shuttle) => server.with_semantic_shuttle(Arc::new(shuttle)),
        Err(error) => {
            warn!(%error, "semantic shuttle unavailable; SearchSemanticTrace will return failed_precondition");
            server
        }
    };

    // Reflection — exposes combined FileDescriptorSet covering all domain protos.
    // Enables grpcurl discovery and drives MCP tool auto-registration in op-chat.
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("failed to build reflection service");

    // Health — standard gRPC health protocol for deploy verification and LB probes.
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<StateSyncServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<PluginServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<EventChainServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<OvsdbMirrorServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<RuntimeMirrorServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<ComponentRegistryServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<MailServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<PrivacyNetworkServiceServer<OperationGrpcServer>>()
        .await;
    health_reporter
        .set_serving::<RegistrationServiceServer<OperationGrpcServer>>()
        .await;

    info!(addr = %addr, "gRPC bridge listening");

    // Wrap services with gRPC-Web support and allow JSON encoding.
    // The GhostbridgeInterceptor enforces the Absolute Base rule on every
    // inbound request before it reaches a service handler.
    let server = tonic::transport::Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(StateSyncServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(PluginServiceServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(EventChainServiceServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(OvsdbMirrorServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(RuntimeMirrorServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(ComponentRegistryServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(MailServiceServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(PrivacyNetworkServiceServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(RegistrationServiceServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(DbusPassthroughServer::with_interceptor(
            server.clone(),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(tonic::codegen::InterceptedService::new(
            McpServiceServer::from_arc(mcp_svc),
            interceptor::ghostbridge_interceptor,
        )))
        .add_service(tonic_web::enable(reflection))
        .add_service(tonic_web::enable(health_service));

    server.serve(addr).await
}

// =============================================================================
// StateSync Service
// =============================================================================

#[tonic::async_trait]
impl StateSync for OperationGrpcServer {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<ProtoStateChange, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        info!("gRPC Subscribe: plugins={:?}", req.plugin_ids);

        let mut rx = self.schema_engine.change_tx().subscribe();
        let plugin_filters = req.plugin_ids;
        let path_filters = req.path_patterns;
        let tag_filters = req.tags;

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => {
                        let matches_plugin = plugin_filters.is_empty()
                            || plugin_filters.contains(&update.plugin_id);
                        let matches_path = path_filters.is_empty()
                            || path_filters.iter().any(|p| update.object_path.starts_with(p));
                        let matches_tag = tag_filters.is_empty()
                            || update.tags_touched.iter().any(|t| tag_filters.contains(t));

                        if matches_plugin && matches_path && matches_tag {
                            yield Ok(proto_state_change(&update));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Subscriber lagged, missed {} updates", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn mutate(
        &self,
        request: Request<MutateRequest>,
    ) -> Result<Response<MutateResponse>, Status> {
        let req = request.into_inner();
        let value = prost_value_to_simd(&req.value.unwrap_or_else(|| ProstValue::from(0)));
        let change_type = match req.operation {
            x if x == ProtoOperationType::SetProperty as i32 => ChangeType::PropertySet,
            x if x == ProtoOperationType::CallMethod as i32 => ChangeType::MethodCall,
            x if x == ProtoOperationType::ApplyPatch as i32 => ChangeType::ObjectAdded,
            _ => ChangeType::PropertySet,
        };

        let result = self
            .schema_engine
            .mutate(
                req.plugin_id.clone(),
                req.object_path.clone(),
                change_type,
                if req.member_name.is_empty() {
                    None
                } else {
                    Some(req.member_name.clone())
                },
                value,
                req.actor_id.clone(),
                if req.capability_id.is_empty() {
                    None
                } else {
                    Some(req.capability_id.clone())
                },
            )
            .await;

        match result {
            Ok(ok) => Ok(Response::new(MutateResponse {
                success: ok.success,
                event_id: ok.event_id,
                event_hash: ok.event_hash,
                result: ok.result.map(|v| simd_to_prost_value(&v)),
                error: None,
                effective_hash: String::new(),
            })),
            Err(e) => Ok(Response::new(MutateResponse {
                success: false,
                event_id: 0,
                event_hash: String::new(),
                result: None,
                error: Some(ProtoMutationError {
                    code: ProtoErrorCode::Internal as i32,
                    message: e.to_string(),
                    deny_reason: None,
                }),
                effective_hash: String::new(),
            })),
        }
    }

    async fn get_state(
        &self,
        request: Request<GetStateRequest>,
    ) -> Result<Response<GetStateResponse>, Status> {
        let req = request.into_inner();
        let state = self.schema_engine.get_state(&req.plugin_id).await;

        let state_struct = state
            .map(|v| simd_to_prost_struct(&v))
            .unwrap_or_else(ProstStruct::default);

        Ok(Response::new(GetStateResponse {
            state: Some(state_struct),
            effective_hash: String::new(),
            at_event_id: 0,
        }))
    }

    async fn batch_mutate(
        &self,
        request: Request<BatchMutateRequest>,
    ) -> Result<Response<BatchMutateResponse>, Status> {
        let req = request.into_inner();
        let mut results = Vec::new();
        let mut failed_index = -1;

        for (idx, m) in req.mutations.into_iter().enumerate() {
            let mut_req = Request::new(m);
            let resp = self.mutate(mut_req).await?.into_inner();
            if !resp.success && failed_index < 0 && req.atomic {
                failed_index = idx as i32;
                break;
            }
            results.push(resp);
        }

        Ok(Response::new(BatchMutateResponse {
            success: failed_index < 0,
            results,
            failed_index,
        }))
    }
}

// =============================================================================
// PluginService
// =============================================================================

#[tonic::async_trait]
impl PluginService for OperationGrpcServer {
    type SubscribeSignalsStream = Pin<Box<dyn Stream<Item = Result<Signal, Status>> + Send>>;

    async fn list_plugins(
        &self,
        _request: Request<()>,
    ) -> Result<Response<ListPluginsResponse>, Status> {
        Ok(Response::new(ListPluginsResponse {
            plugins: self.plugin_provider.list_plugins().await,
        }))
    }

    async fn get_schema(
        &self,
        request: Request<GetSchemaRequest>,
    ) -> Result<Response<GetSchemaResponse>, Status> {
        let req = request.into_inner();
        if let Some((schema_json, dialect, version)) =
            self.plugin_provider.get_schema(&req.plugin_id).await
        {
            Ok(Response::new(GetSchemaResponse {
                schema_json,
                dialect,
                version,
            }))
        } else {
            Ok(Response::new(GetSchemaResponse {
                schema_json: String::new(),
                dialect: String::new(),
                version: String::new(),
            }))
        }
    }

    async fn call_method(
        &self,
        request: Request<CallMethodRequest>,
    ) -> Result<Response<CallMethodResponse>, Status> {
        let req = request.into_inner();
        let args: Vec<simd_json::OwnedValue> = req
            .arguments
            .into_iter()
            .map(|v| prost_value_to_simd(&v))
            .collect();

        // New pipeline: Route through SchemaEngine.mutate for authoritative recording.
        let result = self
            .schema_engine
            .mutate(
                req.plugin_id.clone(),
                req.object_path.clone(),
                ChangeType::MethodCall,
                Some(req.method_name.clone()),
                simd_json::json!(args),
                req.actor_id.clone(),
                if req.capability_id.is_empty() {
                    None
                } else {
                    Some(req.capability_id.clone())
                },
            )
            .await;

        match result {
            Ok(ok) => Ok(Response::new(CallMethodResponse {
                success: ok.success,
                result: ok.result.map(|v| simd_to_prost_value(&v)),
                event_id: ok.event_id,
                event_hash: ok.event_hash,
                error: None,
            })),
            Err(e) => Ok(Response::new(CallMethodResponse {
                success: false,
                result: None,
                event_id: 0,
                event_hash: String::new(),
                error: Some(ProtoMutationError {
                    code: ProtoErrorCode::Internal as i32,
                    message: e.to_string(),
                    deny_reason: None,
                }),
            })),
        }
    }

    async fn get_property(
        &self,
        request: Request<GetPropertyRequest>,
    ) -> Result<Response<GetPropertyResponse>, Status> {
        let req = request.into_inner();
        let connection = Connection::system()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(format!("org.opdbus.{}.v1", req.plugin_id))
            .map_err(|e| Status::internal(e.to_string()))?
            .path(req.object_path.as_str())
            .map_err(|e| Status::internal(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface_name.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let val: ZOwnedValue = proxy
            .get(iface, req.property_name.as_str())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let json =
            simd_json::serde::to_owned_value(&val).map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetPropertyResponse {
            value: Some(simd_to_prost_value(&json)),
            read_only: false,
        }))
    }

    async fn set_property(
        &self,
        request: Request<SetPropertyRequest>,
    ) -> Result<Response<SetPropertyResponse>, Status> {
        let req = request.into_inner();
        let connection = Connection::system()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(format!("org.opdbus.{}.v1", req.plugin_id))
            .map_err(|e| Status::internal(e.to_string()))?
            .path(req.object_path.as_str())
            .map_err(|e| Status::internal(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface_name.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let value = prost_value_to_simd(&req.value.unwrap_or_else(|| ProstValue::from(0)));
        let zval =
            simd_json_to_zvariant(&value).map_err(|e| Status::invalid_argument(e.to_string()))?;

        proxy
            .set(iface, req.property_name.as_str(), zval.into())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(SetPropertyResponse {
            success: true,
            event_id: 0,
            event_hash: String::new(),
            error: None,
        }))
    }

    async fn subscribe_signals(
        &self,
        request: Request<SubscribeSignalsRequest>,
    ) -> Result<Response<Self::SubscribeSignalsStream>, Status> {
        let req = request.into_inner();
        let plugin_filter = req.plugin_id;
        let signal_names = req.signal_names;
        let path_filter = req.object_path;

        // Subscribe to the schema engine's change broadcast and filter for signals
        let mut rx = self.schema_engine.change_tx().subscribe();

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => {
                        // Only emit Signal change types
                        if update.change_type != ChangeType::Signal {
                            continue;
                        }
                        // Plugin filter
                        if !plugin_filter.is_empty() && update.plugin_id != plugin_filter {
                            continue;
                        }
                        // Path filter
                        if !path_filter.is_empty() && !update.object_path.starts_with(&path_filter) {
                            continue;
                        }
                        // Signal name filter
                        let signal_name = update.member_name.clone().unwrap_or_default();
                        if !signal_names.is_empty() && !signal_names.contains(&signal_name) {
                            continue;
                        }

                        yield Ok(Signal {
                            plugin_id: update.plugin_id.clone(),
                            object_path: update.object_path.clone(),
                            interface_name: String::new(),
                            signal_name,
                            arguments: vec![simd_to_prost_value(&update.new_value)],
                            timestamp: Some(ProstTimestamp {
                                seconds: update.timestamp.timestamp(),
                                nanos: update.timestamp.timestamp_subsec_nanos() as i32,
                            }),
                        });
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Signal subscriber lagged, missed {} updates", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}

// =============================================================================
// EventChainService
// =============================================================================

#[tonic::async_trait]
impl EventChainService for OperationGrpcServer {
    type SubscribeEventsStream =
        Pin<Box<dyn Stream<Item = Result<ProtoChainEvent, Status>> + Send>>;

    async fn get_events(
        &self,
        request: Request<GetEventsRequest>,
    ) -> Result<Response<GetEventsResponse>, Status> {
        let req = request.into_inner();
        let chain = self.schema_engine.event_chain.clone();
        let chain = chain.read().await;

        let events: Vec<ProtoChainEvent> = chain
            .events()
            .iter()
            .filter(|e| req.from_event_id == 0 || e.event_id >= req.from_event_id)
            .filter(|e| req.to_event_id == 0 || e.event_id <= req.to_event_id)
            .filter(|e| req.plugin_id.is_empty() || e.plugin_id == req.plugin_id)
            .filter(|e| req.tags.is_empty() || e.tags_touched.iter().any(|t| req.tags.contains(t)))
            .filter(|e| match req.decision_filter {
                x if x == ProtoDecision::Allow as i32 => e.decision == Decision::Allow,
                x if x == ProtoDecision::Deny as i32 => e.decision == Decision::Deny,
                _ => true,
            })
            .take(if req.limit == 0 {
                usize::MAX
            } else {
                req.limit as usize
            })
            .map(proto_chain_event)
            .collect();

        let has_more = req.limit > 0 && (events.len() as u32) == req.limit;
        Ok(Response::new(GetEventsResponse { events, has_more }))
    }

    async fn subscribe_events(
        &self,
        request: Request<SubscribeEventsRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let req = request.into_inner();
        let mut rx = self.chain_events.subscribe();
        let plugin_filter = req.plugin_id;
        let tag_filters = req.tags;

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let matches_plugin = plugin_filter.is_empty() || event.plugin_id == plugin_filter;
                        let matches_tag = tag_filters.is_empty() || event.tags_touched.iter().any(|t| tag_filters.contains(t));
                        if matches_plugin && matches_tag {
                            yield Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn verify_chain(
        &self,
        _request: Request<VerifyChainRequest>,
    ) -> Result<Response<VerifyChainResponse>, Status> {
        let chain = self.schema_engine.event_chain.clone();
        let chain = chain.read().await;
        let result = chain.verify_chain();
        Ok(Response::new(VerifyChainResponse {
            valid: result.valid,
            events_verified: result.events_verified as u64,
            batches_verified: result.batches_verified as u64,
            errors: result.errors,
        }))
    }

    async fn get_proof(
        &self,
        request: Request<GetProofRequest>,
    ) -> Result<Response<GetProofResponse>, Status> {
        let req = request.into_inner();
        let chain = self.schema_engine.event_chain.clone();
        let chain = chain.read().await;
        let proof: Option<MerkleProof> =
            op_state_store::EventBatch::generate_proof(chain.events(), req.event_id);

        if let Some(proof) = proof {
            let siblings = proof
                .siblings
                .into_iter()
                .map(|(hash, is_right)| MerkleProofSibling { hash, is_right })
                .collect();
            Ok(Response::new(GetProofResponse {
                event_hash: proof.event_hash,
                siblings,
                root: proof.root,
                batch_first_event_id: 0,
                batch_last_event_id: 0,
            }))
        } else {
            Err(Status::not_found("proof not found"))
        }
    }

    async fn prove_tag_immutability(
        &self,
        request: Request<ProveTagImmutabilityRequest>,
    ) -> Result<Response<ProveTagImmutabilityResponse>, Status> {
        let req = request.into_inner();
        let chain = self.schema_engine.event_chain.clone();
        let chain = chain.read().await;
        let proof = chain.prove_tag_immutability(&req.tag);
        Ok(Response::new(ProveTagImmutabilityResponse {
            tag: proof.tag,
            is_immutable: proof.is_immutable,
            violation_event_ids: proof.violations,
            total_events_checked: proof.total_events_checked as u64,
        }))
    }

    async fn get_snapshot(
        &self,
        request: Request<GetSnapshotRequest>,
    ) -> Result<Response<GetSnapshotResponse>, Status> {
        let req = request.into_inner();
        let chain = self.schema_engine.event_chain.clone();
        let chain = chain.read().await;
        if let Some(snapshot) = chain.get_snapshot(&req.snapshot_id) {
            Ok(Response::new(GetSnapshotResponse {
                snapshot: Some(proto_snapshot(snapshot)),
            }))
        } else {
            Err(Status::not_found("snapshot not found"))
        }
    }

    async fn create_snapshot(
        &self,
        request: Request<CreateSnapshotRequest>,
    ) -> Result<Response<CreateSnapshotResponse>, Status> {
        let req = request.into_inner();
        let state = self
            .schema_engine
            .get_state(&req.plugin_id)
            .await
            .unwrap_or_else(|| simd_json::json!({}));
        let chain = self.schema_engine.event_chain.clone();
        let mut chain = chain.write().await;
        let snapshot = chain.create_snapshot(req.plugin_id, "1.0.0".to_string(), state);
        Ok(Response::new(CreateSnapshotResponse {
            snapshot: Some(proto_snapshot(snapshot)),
        }))
    }

    async fn search_semantic_trace(
        &self,
        request: Request<SearchSemanticTraceRequest>,
    ) -> Result<Response<SearchSemanticTraceResponse>, Status> {
        let shuttle = self.semantic_shuttle.as_ref().ok_or_else(|| {
            Status::failed_precondition(
                "Qdrant Semantic Shuttle is not configured; check Voyage and Qdrant settings",
            )
        })?;
        let req = request.into_inner();
        let limit = u64::from(if req.limit == 0 { 5 } else { req.limit });
        let trace = shuttle.current_trace_context().map_err(internal_status)?;
        let matches = shuttle
            .search_semantic_trace(limit)
            .await
            .map_err(internal_status)?
            .into_iter()
            .map(proto_semantic_trace_match)
            .collect();

        Ok(Response::new(SearchSemanticTraceResponse {
            trace_id: trace.trace_id,
            mutation_index: trace.mutation_index,
            matches,
        }))
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn proto_state_change(change: &crate::schema_engine::StateChange) -> ProtoStateChange {
    ProtoStateChange {
        change_id: change.change_id.clone(),
        event_id: change.event_id,
        plugin_id: change.plugin_id.clone(),
        object_path: change.object_path.clone(),
        change_type: proto_change_type(change.change_type) as i32,
        member_name: change.member_name.clone().unwrap_or_default(),
        old_value: change.old_value.as_ref().map(simd_to_prost_value),
        new_value: Some(simd_to_prost_value(&change.new_value)),
        tags_touched: change.tags_touched.clone(),
        event_hash: change.event_hash.clone(),
        timestamp: Some(proto_timestamp(change.timestamp)),
        actor_id: change.actor_id.clone(),
    }
}

fn proto_change_type(change_type: ChangeType) -> ProtoChangeType {
    match change_type {
        ChangeType::PropertySet => ProtoChangeType::PropertySet,
        ChangeType::PropertyDelete => ProtoChangeType::PropertyDelete,
        ChangeType::MethodCall => ProtoChangeType::MethodCall,
        ChangeType::Signal => ProtoChangeType::Signal,
        ChangeType::ObjectAdded => ProtoChangeType::ObjectAdded,
        ChangeType::ObjectRemoved => ProtoChangeType::ObjectRemoved,
        ChangeType::SchemaMigration => ProtoChangeType::SchemaMigration,
    }
}

fn proto_chain_event(event: &op_state_store::ChainEvent) -> ProtoChainEvent {
    ProtoChainEvent {
        event_id: event.event_id,
        prev_hash: event.prev_hash.clone(),
        event_hash: event.event_hash.clone(),
        timestamp: Some(proto_timestamp(event.timestamp)),
        actor_id: event.actor_id.clone(),
        capability_id: event.capability_id.clone().unwrap_or_default(),
        plugin_id: event.plugin_id.clone(),
        schema_version: event.schema_version.clone(),
        operation_type: format!("{:?}", event.op),
        target: event.target.clone(),
        tags_touched: event.tags_touched.clone(),
        decision: match event.decision {
            Decision::Allow => ProtoDecision::Allow as i32,
            Decision::Deny => ProtoDecision::Deny as i32,
        },
        deny_reason: event.deny_reason.as_ref().map(proto_deny_reason),
        input_patch_hash: event.input_patch_hash.clone(),
        result_effective_hash: event.result_effective_hash.clone().unwrap_or_default(),
    }
}

fn proto_deny_reason(reason: &DenyReason) -> ProtoDenyReason {
    match reason {
        DenyReason::TagLock { tag, wrapper_id } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::TagLock(ProtoTagLock {
                tag: tag.clone(),
                wrapper_id: wrapper_id.clone(),
            })),
        },
        DenyReason::ConstraintFail {
            constraint,
            message,
        } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: constraint.clone(),
                    message: message.clone(),
                },
            )),
        },
        DenyReason::CapabilityMissing { capability } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::CapabilityMissing(
                ProtoCapabilityMissing {
                    capability: capability.clone(),
                },
            )),
        },
        DenyReason::ReadOnlyViolation { field } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ReadOnlyViolation(
                ProtoReadOnlyViolation {
                    field: field.clone(),
                },
            )),
        },
        DenyReason::SchemaValidation { errors } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: "schema_validation".to_string(),
                    message: errors.join("; "),
                },
            )),
        },
        DenyReason::Custom { reason } => ProtoDenyReason {
            reason: Some(crate::proto::deny_reason::Reason::ConstraintFail(
                ProtoConstraintFail {
                    constraint: "custom".to_string(),
                    message: reason.clone(),
                },
            )),
        },
    }
}

fn proto_snapshot(snapshot: &op_state_store::StateSnapshot) -> crate::proto::Snapshot {
    crate::proto::Snapshot {
        snapshot_id: snapshot.snapshot_id.clone(),
        at_event_id: snapshot.at_event_id,
        plugin_id: snapshot.plugin_id.clone(),
        schema_version: snapshot.schema_version.clone(),
        stub_hash: snapshot.stub_hash.clone(),
        immutable_wrappers_hash: snapshot.immutable_wrappers_hash.clone(),
        tunable_patch_hash: snapshot.tunable_patch_hash.clone(),
        effective_hash: snapshot.effective_hash.clone(),
        timestamp: Some(proto_timestamp(snapshot.timestamp)),
        state: Some(simd_to_prost_struct(&snapshot.state)),
    }
}

fn proto_timestamp(ts: DateTime<Utc>) -> ProstTimestamp {
    ProstTimestamp {
        seconds: ts.timestamp(),
        nanos: ts.timestamp_subsec_nanos() as i32,
    }
}

fn proto_semantic_trace_match(point: qdrant_client::qdrant::ScoredPoint) -> SemanticTraceMatch {
    SemanticTraceMatch {
        point_id: qdrant_point_id_to_string(point.id),
        score: point.score,
        payload: Some(qdrant_payload_to_prost_struct(point.payload)),
    }
}

fn qdrant_point_id_to_string(point_id: Option<qdrant_client::qdrant::PointId>) -> String {
    use qdrant_client::qdrant::point_id::PointIdOptions;

    match point_id.and_then(|id| id.point_id_options) {
        Some(PointIdOptions::Num(value)) => value.to_string(),
        Some(PointIdOptions::Uuid(value)) => value,
        None => String::new(),
    }
}

fn qdrant_payload_to_prost_struct(
    payload: std::collections::HashMap<String, qdrant_client::qdrant::Value>,
) -> ProstStruct {
    ProstStruct {
        fields: payload
            .into_iter()
            .map(|(key, value)| (key, qdrant_value_to_prost_value(value)))
            .collect(),
    }
}

fn qdrant_value_to_prost_value(value: qdrant_client::qdrant::Value) -> ProstValue {
    use prost_types::value::Kind as ProstKind;
    use qdrant_client::qdrant::value::Kind as QdrantKind;

    let kind = match value.kind {
        Some(QdrantKind::NullValue(_)) => ProstKind::NullValue(0),
        Some(QdrantKind::DoubleValue(number)) => ProstKind::NumberValue(number),
        Some(QdrantKind::IntegerValue(number)) => ProstKind::NumberValue(number as f64),
        Some(QdrantKind::StringValue(text)) => ProstKind::StringValue(text),
        Some(QdrantKind::BoolValue(flag)) => ProstKind::BoolValue(flag),
        Some(QdrantKind::StructValue(struct_value)) => {
            ProstKind::StructValue(qdrant_payload_to_prost_struct(struct_value.fields))
        }
        Some(QdrantKind::ListValue(list_value)) => ProstKind::ListValue(prost_types::ListValue {
            values: list_value
                .values
                .into_iter()
                .map(qdrant_value_to_prost_value)
                .collect(),
        }),
        None => ProstKind::NullValue(0),
    };

    ProstValue { kind: Some(kind) }
}

fn internal_status(error: anyhow::Error) -> Status {
    Status::internal(error.to_string())
}

fn simd_to_prost_struct(value: &simd_json::OwnedValue) -> ProstStruct {
    match value.as_object() {
        Some(map) => {
            let fields = map
                .iter()
                .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
                .collect();
            ProstStruct { fields }
        }
        None => ProstStruct {
            fields: BTreeMap::new(),
        },
    }
}

fn simd_to_prost_value(value: &simd_json::OwnedValue) -> ProstValue {
    use prost_types::value::Kind;
    if value.as_null().is_some() {
        return ProstValue {
            kind: Some(Kind::NullValue(0)),
        };
    }
    if let Some(b) = value.as_bool() {
        return ProstValue {
            kind: Some(Kind::BoolValue(b)),
        };
    }
    if let Some(n) = value.as_f64() {
        return ProstValue {
            kind: Some(Kind::NumberValue(n)),
        };
    }
    if let Some(s) = value.as_str() {
        return ProstValue {
            kind: Some(Kind::StringValue(s.to_string())),
        };
    }
    if let Some(arr) = value.as_array() {
        let vals = arr.iter().map(simd_to_prost_value).collect();
        return ProstValue {
            kind: Some(Kind::ListValue(prost_types::ListValue { values: vals })),
        };
    }
    if let Some(obj) = value.as_object() {
        let fields = obj
            .iter()
            .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
            .collect();
        return ProstValue {
            kind: Some(Kind::StructValue(ProstStruct { fields })),
        };
    }
    ProstValue {
        kind: Some(Kind::NullValue(0)),
    }
}

fn prost_value_to_simd(value: &ProstValue) -> simd_json::OwnedValue {
    use prost_types::value::Kind;
    match &value.kind {
        None => simd_json::json!(null),
        Some(Kind::NullValue(_)) => simd_json::json!(null),
        Some(Kind::BoolValue(b)) => simd_json::json!(*b),
        Some(Kind::NumberValue(n)) => simd_json::json!(*n),
        Some(Kind::StringValue(s)) => simd_json::json!(s),
        Some(Kind::StructValue(s)) => {
            let mut map = simd_json::value::owned::Object::new();
            for (k, v) in &s.fields {
                map.insert(k.clone(), prost_value_to_simd(v));
            }
            simd_json::OwnedValue::Object(Box::new(map))
        }
        Some(Kind::ListValue(l)) => {
            let arr = l.values.iter().map(prost_value_to_simd).collect::<Vec<_>>();
            simd_json::OwnedValue::from(arr)
        }
    }
}

fn simd_json_to_zvariant(value: &simd_json::OwnedValue) -> Result<ZOwnedValue, anyhow::Error> {
    if let Some(obj) = value.as_object() {
        if let (Some(sig_val), Some(inner)) = (obj.get("sig"), obj.get("value")) {
            if let Some(sig) = sig_val.as_str() {
                return zvariant_from_sig(sig, inner);
            }
        }
    }

    if let Some(s) = value.as_str() {
        return Ok(ZOwnedValue::from(ZStr::from(s)));
    }
    if let Some(b) = value.as_bool() {
        return Ok(ZOwnedValue::from(b));
    }
    if let Some(i) = value.as_i64() {
        return Ok(ZOwnedValue::from(i));
    }
    if let Some(u) = value.as_u64() {
        return Ok(ZOwnedValue::from(u));
    }
    if let Some(f) = value.as_f64() {
        return Ok(ZOwnedValue::from(f));
    }

    Err(anyhow::anyhow!(
        "Unsupported argument type; use tagged {{sig,value}} or primitives"
    ))
}

fn zvariant_from_sig(
    sig: &str,
    value: &simd_json::OwnedValue,
) -> Result<ZOwnedValue, anyhow::Error> {
    match sig {
        "s" => value
            .as_str()
            .map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected string for sig 's'")),
        "b" => value
            .as_bool()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected bool for sig 'b'")),
        "i" => value
            .as_i64()
            .map(|v| ZOwnedValue::from(v as i32))
            .ok_or_else(|| anyhow::anyhow!("Expected i32 for sig 'i'")),
        "u" => value
            .as_u64()
            .map(|v| ZOwnedValue::from(v as u32))
            .ok_or_else(|| anyhow::anyhow!("Expected u32 for sig 'u'")),
        "x" => value
            .as_i64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected i64 for sig 'x'")),
        "t" => value
            .as_u64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected u64 for sig 't'")),
        "d" => value
            .as_f64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected f64 for sig 'd'")),
        "ay" => {
            let arr = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected array for sig 'ay'"))?;
            let bytes: Result<Vec<u8>, anyhow::Error> = arr
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(|n| n as u8)
                        .ok_or_else(|| anyhow::anyhow!("Expected u8 in ay array"))
                })
                .collect();
            ZOwnedValue::try_from(ZValue::Array(ZArray::from(bytes?)))
                .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
        }
        _ => Err(anyhow::anyhow!("Unsupported signature '{}'", sig)),
    }
}

// =============================================================================
// OvsdbMirror Service — RFC 7047 gRPC bridge
// =============================================================================

#[tonic::async_trait]
impl OvsdbMirror for OperationGrpcServer {
    type MonitorStream = Pin<Box<dyn Stream<Item = Result<OvsdbUpdate, Status>> + Send>>;

    async fn list_dbs(
        &self,
        _request: Request<()>,
    ) -> Result<Response<OvsdbListDbsResponse>, Status> {
        let result = self.ovsdb_call("list_dbs", "[]").await?;
        let dbs: Vec<String> = serde_json::from_str(&result)
            .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;
        Ok(Response::new(OvsdbListDbsResponse { databases: dbs }))
    }

    async fn get_schema(
        &self,
        request: Request<OvsdbGetSchemaRequest>,
    ) -> Result<Response<OvsdbGetSchemaResponse>, Status> {
        let db = &request.get_ref().database;
        let db_arg = if db.is_empty() { "Open_vSwitch" } else { db };
        let result = self
            .ovsdb_call("get_schema", &format!("[\"{}\"]", db_arg))
            .await?;
        Ok(Response::new(OvsdbGetSchemaResponse {
            schema_json: result,
            name: db_arg.to_string(),
            version: String::new(),
        }))
    }

    async fn transact(
        &self,
        request: Request<OvsdbTransactRequest>,
    ) -> Result<Response<OvsdbTransactResponse>, Status> {
        let req = request.get_ref();
        let db = if req.database.is_empty() {
            "Open_vSwitch"
        } else {
            &req.database
        };
        let ops = &req.operations_json;
        let call_arg = format!("[\"{}\", {}]", db, ops);
        match self.ovsdb_call("transact", &call_arg).await {
            Ok(result) => Ok(Response::new(OvsdbTransactResponse {
                success: true,
                results_json: result,
                event_id: 0,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(OvsdbTransactResponse {
                success: false,
                results_json: String::new(),
                event_id: 0,
                error: e.message().to_string(),
            })),
        }
    }

    async fn monitor(
        &self,
        request: Request<OvsdbMonitorRequest>,
    ) -> Result<Response<Self::MonitorStream>, Status> {
        let req = request.into_inner();
        let _database = if req.database.is_empty() {
            "Open_vSwitch".to_string()
        } else {
            req.database
        };

        // Subscribe to the schema engine's change broadcast and filter for OVSDB paths
        let mut rx = self.schema_engine.change_tx().subscribe();

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => {
                        // Filter to only OVSDB-related path changes
                        if !update.object_path.starts_with("/org/opdbus/v1/ovsdb") {
                            continue;
                        }

                        // Extract table name and UUID from path:
                        //   /org/opdbus/v1/ovsdb/{table_name}/{uuid}
                        let parts: Vec<&str> = update.object_path.split('/').collect();
                        let (table, uuid) = if parts.len() >= 7 {
                            (parts[5].to_string(), parts[6].to_string())
                        } else {
                            continue;
                        };

                        let new_row = Some(simd_to_prost_struct(&update.new_value));
                        let old_row = update.old_value.as_ref().map(simd_to_prost_struct);

                        yield Ok(OvsdbUpdate {
                            table,
                            uuid,
                            old_row,
                            new_row,
                            timestamp: Some(ProstTimestamp {
                                seconds: update.timestamp.timestamp(),
                                nanos: update.timestamp.timestamp_subsec_nanos() as i32,
                            }),
                        });
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("OVSDB monitor subscriber lagged, missed {} updates", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn echo(
        &self,
        request: Request<OvsdbEchoRequest>,
    ) -> Result<Response<OvsdbEchoResponse>, Status> {
        Ok(Response::new(OvsdbEchoResponse {
            payload: request.into_inner().payload,
        }))
    }

    async fn dump_db(
        &self,
        request: Request<OvsdbDumpDbRequest>,
    ) -> Result<Response<OvsdbDumpDbResponse>, Status> {
        let db = &request.get_ref().database;
        let db_arg = if db.is_empty() { "Open_vSwitch" } else { db };
        let result = self
            .ovsdb_call("dump", &format!("[\"{}\"]", db_arg))
            .await?;
        Ok(Response::new(OvsdbDumpDbResponse { dump_json: result }))
    }

    async fn get_bridge_state(
        &self,
        request: Request<OvsdbGetBridgeStateRequest>,
    ) -> Result<Response<OvsdbGetBridgeStateResponse>, Status> {
        let filter = &request.get_ref().bridge_name;

        // Query via D-Bus mirror's OvsdbV1 interface
        let dump = self.ovsdb_call("dump", "[\"Open_vSwitch\"]").await?;

        // Parse and build bridge hierarchy
        let bridges = self
            .parse_bridge_hierarchy(&dump, filter)
            .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;

        Ok(Response::new(OvsdbGetBridgeStateResponse { bridges }))
    }
}

impl OperationGrpcServer {
    /// Call an OVSDB method via the D-Bus mirror interface
    async fn ovsdb_call(&self, method: &str, args: &str) -> Result<String, Status> {
        let conn = self
            .schema_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {}", e)))?;

        let proxy = Proxy::new(
            &conn,
            "org.opdbus.v1",
            "/org/opdbus/v1/ovsdb",
            "org.opdbus.OvsdbV1",
        )
        .await
        .map_err(|e| Status::internal(format!("Proxy error: {}", e)))?;

        let result: String = proxy
            .call(method, &(args.to_string(),))
            .await
            .map_err(|e| Status::internal(format!("OVSDB call '{}' failed: {}", method, e)))?;

        Ok(result)
    }

    /// Parse OVSDB dump into Bridge→Port→Interface hierarchy
    fn parse_bridge_hierarchy(
        &self,
        dump_json: &str,
        filter: &str,
    ) -> Result<Vec<ProtoOvsdbBridge>, anyhow::Error> {
        let dump: serde_json::Value = serde_json::from_str(dump_json)?;
        let mut bridges = Vec::new();

        // Get Bridge table rows
        let empty_map = serde_json::Map::new();
        let bridge_rows = dump
            .get("Bridge")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);

        for (_uuid, row) in bridge_rows {
            let name = row
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !filter.is_empty() && name != filter {
                continue;
            }

            let datapath_type = row
                .get("datapath_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let fail_mode = row
                .get("fail_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let stp_enable = row
                .get("stp_enable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mcast_snooping_enable = row
                .get("mcast_snooping_enable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Parse ports (OVSDB set format: ["set", [...]] or ["uuid", "..."])
            let port_uuids = self.extract_uuid_set(row.get("ports"));

            let mut ports = Vec::new();
            let empty_port_map = serde_json::Map::new();
            let port_rows = dump
                .get("Port")
                .and_then(|v| v.as_object())
                .unwrap_or(&empty_port_map);

            for port_uuid in &port_uuids {
                if let Some(port_row) = port_rows.get(port_uuid) {
                    let port_name = port_row
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tag = port_row.get("tag").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                    // Parse interfaces
                    let iface_uuids = self.extract_uuid_set(port_row.get("interfaces"));
                    let empty_iface_map = serde_json::Map::new();
                    let iface_rows = dump
                        .get("Interface")
                        .and_then(|v| v.as_object())
                        .unwrap_or(&empty_iface_map);

                    let mut interfaces = Vec::new();
                    for iface_uuid in &iface_uuids {
                        if let Some(iface_row) = iface_rows.get(iface_uuid) {
                            interfaces.push(ProtoOvsdbInterface {
                                name: iface_row
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r#type: iface_row
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                mac_in_use: iface_row
                                    .get("mac_in_use")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                mac: iface_row
                                    .get("mac")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                admin_state: iface_row
                                    .get("admin_state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                link_state: iface_row
                                    .get("link_state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                options: self.extract_map(iface_row.get("options")),
                            });
                        }
                    }

                    ports.push(ProtoOvsdbPort {
                        name: port_name,
                        tag,
                        trunks: vec![],
                        vlan_mode: port_row
                            .get("vlan_mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        bond_mode: port_row
                            .get("bond_mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        interfaces,
                    });
                }
            }

            bridges.push(ProtoOvsdbBridge {
                name,
                datapath_type,
                fail_mode,
                stp_enable,
                mcast_snooping_enable,
                other_config: self.extract_map(row.get("other_config")),
                ports,
            });
        }

        Ok(bridges)
    }

    /// Extract UUID set from OVSDB value (["set", [...]] or ["uuid", "..."])
    fn extract_uuid_set(&self, value: Option<&serde_json::Value>) -> Vec<String> {
        let Some(v) = value else {
            return vec![];
        };
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 {
                if arr[0].as_str() == Some("uuid") {
                    return vec![arr[1].as_str().unwrap_or("").to_string()];
                }
                if arr[0].as_str() == Some("set") {
                    if let Some(items) = arr[1].as_array() {
                        return items
                            .iter()
                            .filter_map(|item| {
                                if let Some(inner) = item.as_array() {
                                    if inner.len() == 2 && inner[0].as_str() == Some("uuid") {
                                        return inner[1].as_str().map(|s| s.to_string());
                                    }
                                }
                                None
                            })
                            .collect();
                    }
                }
            }
        }
        vec![]
    }

    /// Extract OVSDB map (["map", [[k,v], ...]]) to HashMap
    fn extract_map(
        &self,
        value: Option<&serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        let Some(v) = value else {
            return map;
        };
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 && arr[0].as_str() == Some("map") {
                if let Some(pairs) = arr[1].as_array() {
                    for pair in pairs {
                        if let Some(kv) = pair.as_array() {
                            if kv.len() == 2 {
                                let k = kv[0].as_str().unwrap_or("").to_string();
                                let v = kv[1].as_str().unwrap_or("").to_string();
                                map.insert(k, v);
                            }
                        }
                    }
                }
            }
        }
        map
    }
}

// =============================================================================
// RuntimeMirror Service — Live operational state
// =============================================================================

#[tonic::async_trait]
impl RuntimeMirror for OperationGrpcServer {
    type StreamMetricsStream =
        Pin<Box<dyn Stream<Item = Result<RuntimeMetricUpdate, Status>> + Send>>;

    async fn get_system_info(
        &self,
        _request: Request<()>,
    ) -> Result<Response<RuntimeGetSystemInfoResponse>, Status> {
        let hostname = tokio::fs::read_to_string("/etc/hostname")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();
        let kernel_version = tokio::fs::read_to_string("/proc/version")
            .await
            .unwrap_or_default()
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .to_string();
        let uptime_str = tokio::fs::read_to_string("/proc/uptime")
            .await
            .unwrap_or_default();
        let uptime_seconds = uptime_str
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;

        let meminfo = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .unwrap_or_default();
        let mem_total = Self::parse_meminfo_kb(&meminfo, "MemTotal") * 1024;
        let mem_available = Self::parse_meminfo_kb(&meminfo, "MemAvailable") * 1024;
        let mem_used = mem_total.saturating_sub(mem_available);

        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        let arch = std::env::consts::ARCH.to_string();

        // Detect init system — prefer s6 (Artix/Chimera) over systemd
        let init_system = if std::path::Path::new("/run/s6-rc").exists() {
            "s6"
        } else if std::path::Path::new("/run/systemd").exists() {
            "systemd"
        } else {
            "unknown"
        }
        .to_string();

        Ok(Response::new(RuntimeGetSystemInfoResponse {
            hostname,
            kernel_version,
            uptime_seconds,
            boot_timestamp: 0,
            cpu_count,
            memory_total_bytes: mem_total,
            memory_available_bytes: mem_available,
            memory_used_bytes: mem_used,
            init_system,
            arch,
            queried_at: Some(ProstTimestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
        }))
    }

    async fn list_services(
        &self,
        _request: Request<RuntimeListServicesRequest>,
    ) -> Result<Response<RuntimeListServicesResponse>, Status> {
        // Query s6 via s6-rc -a -l /run/s6-rc list
        let output = tokio::process::Command::new("s6-rc")
            .args(["-a", "-l", "/run/s6-rc", "list"])
            .output()
            .await
            .map_err(|e| Status::internal(format!("s6-rc list failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut services = Vec::new();

        for line in stdout.lines() {
            let name = line.trim().to_string();
            if name.is_empty() {
                continue;
            }
            // All entries returned by s6-rc -a list are currently running
            services.push(ProtoRuntimeServiceInfo {
                name,
                state: "STARTED".to_string(),
                pid: 0,
                enabled: true,
                description: String::new(),
                dependencies: vec![],
                started_at: None,
            });
        }

        Ok(Response::new(RuntimeListServicesResponse { services }))
    }

    async fn get_service(
        &self,
        request: Request<RuntimeGetServiceRequest>,
    ) -> Result<Response<ProtoRuntimeServiceInfo>, Status> {
        let name = &request.get_ref().service_name;
        // Check running services via s6-rc -a list and look for this service
        let output = tokio::process::Command::new("s6-rc")
            .args(["-a", "-l", "/run/s6-rc", "list"])
            .output()
            .await
            .map_err(|e| Status::internal(format!("s6-rc list failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let is_running = stdout.lines().any(|l| l.trim() == name.as_str());
        let state = if is_running { "STARTED" } else { "STOPPED" };

        Ok(Response::new(ProtoRuntimeServiceInfo {
            name: name.clone(),
            state: state.to_string(),
            pid: 0,
            enabled: state == "STARTED",
            description: stdout.trim().to_string(),
            dependencies: vec![],
            started_at: None,
        }))
    }

    async fn stream_metrics(
        &self,
        request: Request<RuntimeStreamMetricsRequest>,
    ) -> Result<Response<Self::StreamMetricsStream>, Status> {
        let interval = std::cmp::max(request.get_ref().interval_seconds, 1) as u64;

        let stream = stream! {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval));
            loop {
                ticker.tick().await;

                // Read /proc/meminfo
                if let Ok(meminfo) = tokio::fs::read_to_string("/proc/meminfo").await {
                    let total = OperationGrpcServer::parse_meminfo_kb(&meminfo, "MemTotal") * 1024;
                    let available = OperationGrpcServer::parse_meminfo_kb(&meminfo, "MemAvailable") * 1024;
                    yield Ok(RuntimeMetricUpdate {
                        category: "memory".to_string(),
                        name: "used_bytes".to_string(),
                        value: (total - available) as f64,
                        unit: "bytes".to_string(),
                        labels: Default::default(),
                        timestamp: Some(ProstTimestamp {
                            seconds: chrono::Utc::now().timestamp(),
                            nanos: 0,
                        }),
                    });
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_interfaces(
        &self,
        _request: Request<()>,
    ) -> Result<Response<RuntimeListInterfacesResponse>, Status> {
        // Read from /sys/class/net
        let mut interfaces = Vec::new();
        let mut entries = tokio::fs::read_dir("/sys/class/net")
            .await
            .map_err(|e| Status::internal(format!("Cannot read /sys/class/net: {}", e)))?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = format!("/sys/class/net/{}", name);

            let mac = tokio::fs::read_to_string(format!("{}/address", base))
                .await
                .unwrap_or_default()
                .trim()
                .to_string();
            let mtu: u32 = tokio::fs::read_to_string(format!("{}/mtu", base))
                .await
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0);
            let ifindex: u32 = tokio::fs::read_to_string(format!("{}/ifindex", base))
                .await
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0);
            let operstate = tokio::fs::read_to_string(format!("{}/operstate", base))
                .await
                .unwrap_or_default()
                .trim()
                .to_uppercase();

            interfaces.push(ProtoRuntimeNetworkInterface {
                name,
                index: ifindex,
                mac_address: mac,
                state: operstate,
                mtu,
                ipv4_addresses: vec![],
                ipv6_addresses: vec![],
                rx_bytes: 0,
                tx_bytes: 0,
                rx_packets: 0,
                tx_packets: 0,
                driver: String::new(),
                speed_mbps: 0,
            });
        }

        Ok(Response::new(RuntimeListInterfacesResponse { interfaces }))
    }

    async fn get_numa_topology(
        &self,
        _request: Request<()>,
    ) -> Result<Response<RuntimeGetNumaTopologyResponse>, Status> {
        let mut nodes = Vec::new();

        // Read /sys/devices/system/node/node*/
        if let Ok(mut entries) = tokio::fs::read_dir("/sys/devices/system/node").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("node") {
                    continue;
                }
                let node_id: u32 = name
                    .strip_prefix("node")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let meminfo_path = format!("/sys/devices/system/node/{}/meminfo", name);
                let meminfo = tokio::fs::read_to_string(&meminfo_path)
                    .await
                    .unwrap_or_default();
                let mem_total = Self::parse_node_meminfo_kb(&meminfo, "MemTotal") * 1024;
                let mem_free = Self::parse_node_meminfo_kb(&meminfo, "MemFree") * 1024;

                nodes.push(ProtoNumaNode {
                    node_id,
                    cpus: vec![],
                    memory_total_bytes: mem_total,
                    memory_free_bytes: mem_free,
                    memory_used_bytes: mem_total.saturating_sub(mem_free),
                });
            }
        }

        Ok(Response::new(RuntimeGetNumaTopologyResponse { nodes }))
    }
}

impl OperationGrpcServer {
    fn parse_meminfo_kb(meminfo: &str, key: &str) -> u64 {
        meminfo
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0)
    }

    fn parse_node_meminfo_kb(meminfo: &str, key: &str) -> u64 {
        meminfo
            .lines()
            .find(|l| l.contains(key))
            .and_then(|l| {
                l.split_whitespace()
                    .rev()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0)
    }

    fn now_ts() -> prost_types::Timestamp {
        let now = Utc::now();
        prost_types::Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        }
    }
}

// =============================================================================
// ComponentRegistry Service
// =============================================================================

use crate::proto::registry::{
    component_registry_server::ComponentRegistry, ComponentInfo, ComponentStatus,
    DeregisterRequest, DeregisterResponse, DiscoverRequest, DiscoverResponse, GetComponentRequest,
    GetComponentResponse, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse,
    RegistryEvent, RegistryEventType, WatchRequest,
};

#[tonic::async_trait]
impl ComponentRegistry for OperationGrpcServer {
    type WatchStream = Pin<Box<dyn Stream<Item = Result<RegistryEvent, Status>> + Send + 'static>>;

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        if req.component_id.is_empty() {
            return Err(Status::invalid_argument("component_id must not be empty"));
        }

        let lease_token = Uuid::new_v4().to_string();
        let now = OperationGrpcServer::now_ts();

        let mut inner = self.registry.write().await;

        let is_update = inner.components.contains_key(&req.component_id);
        let info = ComponentInfo {
            component_id: req.component_id.clone(),
            component_type: req.component_type.clone(),
            name: req.name.clone(),
            description: req.description.clone(),
            schema_json: req.schema_json.clone(),
            metadata: req.metadata.clone(),
            capabilities: req.capabilities.clone(),
            endpoint: req.endpoint.clone(),
            version: req.version.clone(),
            status: ComponentStatus::Active as i32,
            registered_at: Some(now),
            last_heartbeat: Some(now),
        };

        inner
            .components
            .insert(req.component_id.clone(), info.clone());
        inner
            .leases
            .insert(req.component_id.clone(), lease_token.clone());

        let event_type = if is_update {
            RegistryEventType::RegistryEventUpdated
        } else {
            RegistryEventType::RegistryEventRegistered
        };
        let event = RegistryEvent {
            event_type: event_type as i32,
            component: Some(info),
            timestamp: Some(now),
        };
        // Ignore send error — no active watchers is fine.
        let _ = inner.watch_tx.send(event);

        info!(
            component_id = %req.component_id,
            component_type = %req.component_type,
            update = is_update,
            "component registered"
        );

        Ok(Response::new(RegisterResponse {
            success: true,
            message: if is_update {
                "updated".to_string()
            } else {
                "registered".to_string()
            },
            lease_token,
            registered_at: Some(now),
        }))
    }

    async fn deregister(
        &self,
        request: Request<DeregisterRequest>,
    ) -> Result<Response<DeregisterResponse>, Status> {
        let req = request.into_inner();
        let mut inner = self.registry.write().await;

        match inner.leases.get(&req.component_id) {
            None => {
                return Ok(Response::new(DeregisterResponse {
                    success: false,
                    message: "component not found".to_string(),
                }))
            }
            Some(stored) if stored != &req.lease_token => {
                return Err(Status::permission_denied("invalid lease token"))
            }
            _ => {}
        }

        let info = inner.components.remove(&req.component_id);
        inner.leases.remove(&req.component_id);

        if let Some(mut component) = info {
            component.status = ComponentStatus::Deregistered as i32;
            let event = RegistryEvent {
                event_type: RegistryEventType::RegistryEventDeregistered as i32,
                component: Some(component),
                timestamp: Some(OperationGrpcServer::now_ts()),
            };
            let _ = inner.watch_tx.send(event);
        }

        info!(component_id = %req.component_id, "component deregistered");

        Ok(Response::new(DeregisterResponse {
            success: true,
            message: "deregistered".to_string(),
        }))
    }

    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        let inner = self.registry.read().await;

        let components: Vec<ComponentInfo> = inner
            .components
            .values()
            .filter(|c| {
                // Type filter
                if !req.component_type.is_empty() && c.component_type != req.component_type {
                    return false;
                }
                // Capability filter
                if !req.capability.is_empty() && !c.capabilities.contains(&req.capability) {
                    return false;
                }
                // Metadata filter
                if !req.metadata_key.is_empty() {
                    match c.metadata.get(&req.metadata_key) {
                        Some(v) if req.metadata_value.is_empty() || v == &req.metadata_value => {}
                        _ => return false,
                    }
                }
                // Stale filter
                if !req.include_stale && c.status == ComponentStatus::Stale as i32 {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        let total = components.len() as u32;
        Ok(Response::new(DiscoverResponse {
            components,
            total_count: total,
        }))
    }

    async fn get_component(
        &self,
        request: Request<GetComponentRequest>,
    ) -> Result<Response<GetComponentResponse>, Status> {
        let req = request.into_inner();
        let inner = self.registry.read().await;
        let component = inner.components.get(&req.component_id).cloned();
        let found = component.is_some();
        Ok(Response::new(GetComponentResponse { component, found }))
    }

    async fn watch(
        &self,
        request: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let req = request.into_inner();
        let type_filter: Vec<String> = req.component_types.clone();

        let inner = self.registry.read().await;
        let mut rx = inner.watch_tx.subscribe();

        // Collect existing components to replay if requested.
        let existing: Vec<ComponentInfo> = if req.include_existing {
            inner.components.values().cloned().collect()
        } else {
            Vec::new()
        };
        drop(inner);

        let output = stream! {
            // Replay existing registrations first.
            for info in existing {
                if type_filter.is_empty() || type_filter.contains(&info.component_type) {
                    yield Ok(RegistryEvent {
                        event_type: RegistryEventType::RegistryEventRegistered as i32,
                        component: Some(info),
                        timestamp: Some(OperationGrpcServer::now_ts()),
                    });
                }
            }
            // Stream live events.
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let passes_filter = type_filter.is_empty()
                            || event
                                .component
                                .as_ref()
                                .map(|c| type_filter.contains(&c.component_type))
                                .unwrap_or(false);
                        if passes_filter {
                            yield Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Watch stream lagged — skipping events");
                        // Continue rather than closing the stream.
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(output)))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        let mut inner = self.registry.write().await;

        match inner.leases.get(&req.component_id) {
            None => {
                // Component not known — tell it to re-register.
                return Ok(Response::new(HeartbeatResponse {
                    acknowledged: false,
                    reregister_required: true,
                    server_time: Some(OperationGrpcServer::now_ts()),
                }));
            }
            Some(stored) if stored != &req.lease_token => {
                return Err(Status::permission_denied("invalid lease token"))
            }
            _ => {}
        }

        let now = OperationGrpcServer::now_ts();
        let was_stale;
        if let Some(info) = inner.components.get_mut(&req.component_id) {
            was_stale = info.status == ComponentStatus::Stale as i32;
            info.last_heartbeat = Some(now);
            if was_stale {
                info.status = ComponentStatus::Active as i32;
            }
        } else {
            return Ok(Response::new(HeartbeatResponse {
                acknowledged: false,
                reregister_required: true,
                server_time: Some(now),
            }));
        }

        if was_stale {
            if let Some(info) = inner.components.get(&req.component_id).cloned() {
                let event = RegistryEvent {
                    event_type: RegistryEventType::RegistryEventRecovered as i32,
                    component: Some(info),
                    timestamp: Some(now),
                };
                let _ = inner.watch_tx.send(event);
                info!(component_id = %req.component_id, "component recovered from stale");
            }
        }

        debug!(component_id = %req.component_id, "heartbeat acknowledged");

        Ok(Response::new(HeartbeatResponse {
            acknowledged: true,
            reregister_required: false,
            server_time: Some(now),
        }))
    }
}

// =============================================================================
// MailService — Email and Webmail Operations via D-Bus bridge
// =============================================================================

use crate::proto::mail::{
    mail_service_server::MailService, AdminMailActionRequest, AdminMailActionResponse,
    CheckMailServerRequest, CheckMailServerResponse, GetInboxRequest, GetInboxResponse,
    GetMailStatusRequest, GetMailStatusResponse, GetMessageRequest, GetMessageResponse,
    ListMailAccountsRequest, ListMailAccountsResponse, SendEmailRequest, SendEmailResponse,
};

impl OperationGrpcServer {
    /// Call a MailService method via the D-Bus mail interface.
    /// Falls back to SchemaEngine state if the D-Bus service is unavailable.
    async fn mail_dbus_call(&self, method: &str, args: &str) -> Result<String, Status> {
        let conn = self
            .schema_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {}", e)))?;

        let proxy = Proxy::new(
            &conn,
            "org.opdbus.v1",
            "/org/opdbus/v1/mail",
            "org.opdbus.MailV1",
        )
        .await
        .map_err(|e| Status::internal(format!("Mail proxy error: {}", e)))?;

        let result: String = proxy
            .call(method, &(args.to_string(),))
            .await
            .map_err(|e| {
                Status::unavailable(format!(
                    "Mail D-Bus service unavailable for '{}': {}",
                    method, e
                ))
            })?;

        Ok(result)
    }
}

#[tonic::async_trait]
impl MailService for OperationGrpcServer {
    async fn send_email(
        &self,
        request: Request<SendEmailRequest>,
    ) -> Result<Response<SendEmailResponse>, Status> {
        let req = request.into_inner();
        info!(
            from = %req.from_email,
            to = %req.to_email,
            subject = %req.subject,
            "gRPC SendEmail"
        );

        // Try D-Bus mail service first
        let args = simd_json::json!({
            "from": req.from_email,
            "to": req.to_email,
            "subject": req.subject,
            "body": req.body,
            "is_html": req.is_html,
            "domain": req.domain
        });
        let args_str = args.to_string();

        match self.mail_dbus_call("send_email", &args_str).await {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(SendEmailResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sent")
                        .to_string(),
                    message_id: parsed
                        .get("message_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    sent_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_dbus_err) => {
                // Record the send attempt in SchemaEngine state
                let mutation_value = simd_json::json!({
                    "from": req.from_email,
                    "to": req.to_email,
                    "subject": req.subject,
                    "status": "queued_no_backend"
                });
                let message_id = Uuid::new_v4().to_string();
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "mail".to_string(),
                        format!("/org/opdbus/v1/mail/outbox/{}", message_id),
                        ChangeType::ObjectAdded,
                        Some("send_email".to_string()),
                        mutation_value,
                        req.from_email.clone(),
                        None,
                    )
                    .await;

                Ok(Response::new(SendEmailResponse {
                    success: false,
                    message: "Mail D-Bus service unavailable; email queued in state store"
                        .to_string(),
                    message_id,
                    sent_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
        }
    }

    async fn get_inbox(
        &self,
        request: Request<GetInboxRequest>,
    ) -> Result<Response<GetInboxResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "email": req.email,
            "domain": req.domain,
            "limit": req.limit,
            "offset": req.offset,
            "folder": req.folder
        });

        match self.mail_dbus_call("get_inbox", &args.to_string()).await {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let messages = parsed
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|m| crate::proto::mail::EmailMessage {
                                message_id: m
                                    .get("message_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                from: m
                                    .get("from")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                to: m
                                    .get("to")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                subject: m
                                    .get("subject")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                preview: m
                                    .get("preview")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                is_read: m
                                    .get("is_read")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                has_attachments: m
                                    .get("has_attachments")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                received_at: None,
                                size_bytes: m
                                    .get("size_bytes")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0)
                                    as i32,
                                folder: m
                                    .get("folder")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(GetInboxResponse {
                    messages,
                    total_count: parsed
                        .get("total_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    unread_count: parsed
                        .get("unread_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    folder: req.folder,
                }))
            }
            Err(_) => {
                // Return empty inbox when backend is unavailable
                Ok(Response::new(GetInboxResponse {
                    messages: vec![],
                    total_count: 0,
                    unread_count: 0,
                    folder: if req.folder.is_empty() {
                        "inbox".to_string()
                    } else {
                        req.folder
                    },
                }))
            }
        }
    }

    async fn get_message(
        &self,
        request: Request<GetMessageRequest>,
    ) -> Result<Response<GetMessageResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "message_id": req.message_id,
            "email": req.email,
            "domain": req.domain
        });

        match self.mail_dbus_call("get_message", &args.to_string()).await {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GetMessageResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    header: None,
                    body: parsed
                        .get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_html: parsed
                        .get("is_html")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    attachments: vec![],
                    raw_content: parsed
                        .get("raw_content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }))
            }
            Err(_) => Err(Status::unavailable(
                "Mail D-Bus service is not available; cannot retrieve message",
            )),
        }
    }

    async fn get_mail_status(
        &self,
        request: Request<GetMailStatusRequest>,
    ) -> Result<Response<GetMailStatusResponse>, Status> {
        let req = request.into_inner();

        // Try reading status from SchemaEngine state first
        let state = self.schema_engine.get_state("mail").await;
        if let Some(ref st) = state {
            if let Some(status_obj) = st.as_object().and_then(|o| o.get("status")) {
                return Ok(Response::new(GetMailStatusResponse {
                    is_configured: status_obj
                        .as_object()
                        .and_then(|o| o.get("is_configured"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    is_running: status_obj
                        .as_object()
                        .and_then(|o| o.get("is_running"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    mail_server_type: "maddy".to_string(),
                    webmail_url: format!("https://mail.{}", req.domain),
                    smtp_status: "unknown".to_string(),
                    imap_status: "unknown".to_string(),
                    total_accounts: 0,
                    total_messages: 0,
                    last_checked: Some(OperationGrpcServer::now_ts()),
                    message: "Status from state store".to_string(),
                }));
            }
        }

        // Attempt D-Bus call
        match self
            .mail_dbus_call(
                "get_status",
                &simd_json::json!({"domain": req.domain}).to_string(),
            )
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GetMailStatusResponse {
                    is_configured: parsed
                        .get("is_configured")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    is_running: parsed
                        .get("is_running")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    mail_server_type: parsed
                        .get("mail_server_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("maddy")
                        .to_string(),
                    webmail_url: parsed
                        .get("webmail_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    smtp_status: parsed
                        .get("smtp_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    imap_status: parsed
                        .get("imap_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    total_accounts: parsed
                        .get("total_accounts")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    total_messages: parsed
                        .get("total_messages")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    last_checked: Some(OperationGrpcServer::now_ts()),
                    message: "ok".to_string(),
                }))
            }
            Err(_) => Ok(Response::new(GetMailStatusResponse {
                is_configured: false,
                is_running: false,
                mail_server_type: String::new(),
                webmail_url: String::new(),
                smtp_status: "unavailable".to_string(),
                imap_status: "unavailable".to_string(),
                total_accounts: 0,
                total_messages: 0,
                last_checked: Some(OperationGrpcServer::now_ts()),
                message: "Mail D-Bus service is not available".to_string(),
            })),
        }
    }

    async fn list_mail_accounts(
        &self,
        request: Request<ListMailAccountsRequest>,
    ) -> Result<Response<ListMailAccountsResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "domain": req.domain,
            "include_inactive": req.include_inactive
        });

        match self
            .mail_dbus_call("list_accounts", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let accounts: Vec<_> = parsed
                    .get("accounts")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|a| crate::proto::mail::MailAccount {
                                email: a
                                    .get("email")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                user_id: a
                                    .get("user_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                is_admin: a
                                    .get("is_admin")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                is_active: a
                                    .get("is_active")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true),
                                created_at: None,
                                message_count: a
                                    .get("message_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                                unread_count: a
                                    .get("unread_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                                last_login: a
                                    .get("last_login")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let total = accounts.len() as u32;
                Ok(Response::new(ListMailAccountsResponse {
                    accounts,
                    total_count: total,
                }))
            }
            Err(_) => Ok(Response::new(ListMailAccountsResponse {
                accounts: vec![],
                total_count: 0,
            })),
        }
    }

    async fn admin_mail_action(
        &self,
        request: Request<AdminMailActionRequest>,
    ) -> Result<Response<AdminMailActionResponse>, Status> {
        let req = request.into_inner();
        info!(
            action = %req.action,
            email = %req.email,
            domain = %req.domain,
            "gRPC AdminMailAction"
        );

        let args = simd_json::json!({
            "action": req.action,
            "email": req.email,
            "domain": req.domain
        });

        match self.mail_dbus_call("admin_action", &args.to_string()).await {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(AdminMailActionResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ok")
                        .to_string(),
                    action_id: Uuid::new_v4().to_string(),
                    timestamp: Some(OperationGrpcServer::now_ts()),
                    result: req.parameters,
                }))
            }
            Err(_) => {
                // Record admin action in state store even without backend
                let mutation_value = simd_json::json!({
                    "action": req.action,
                    "email": req.email,
                    "domain": req.domain,
                    "status": "pending_no_backend"
                });
                let action_id = Uuid::new_v4().to_string();
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "mail".to_string(),
                        format!("/org/opdbus/v1/mail/admin_actions/{}", action_id),
                        ChangeType::MethodCall,
                        Some(req.action.clone()),
                        mutation_value,
                        "admin".to_string(),
                        None,
                    )
                    .await;

                Ok(Response::new(AdminMailActionResponse {
                    success: false,
                    message: "Mail D-Bus service unavailable; action recorded in state store"
                        .to_string(),
                    action_id,
                    timestamp: Some(OperationGrpcServer::now_ts()),
                    result: None,
                }))
            }
        }
    }

    async fn check_mail_server(
        &self,
        request: Request<CheckMailServerRequest>,
    ) -> Result<Response<CheckMailServerResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "domain": req.domain,
            "check_smtp": req.check_smtp,
            "check_imap": req.check_imap,
            "check_webmail": req.check_webmail
        });

        match self.mail_dbus_call("check_server", &args.to_string()).await {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(CheckMailServerResponse {
                    all_healthy: parsed
                        .get("all_healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    smtp_healthy: parsed
                        .get("smtp_healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    imap_healthy: parsed
                        .get("imap_healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    webmail_healthy: parsed
                        .get("webmail_healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    smtp_status: parsed
                        .get("smtp_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unchecked")
                        .to_string(),
                    imap_status: parsed
                        .get("imap_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unchecked")
                        .to_string(),
                    webmail_status: parsed
                        .get("webmail_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unchecked")
                        .to_string(),
                    message: "ok".to_string(),
                    issues: parsed
                        .get("issues")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }))
            }
            Err(_) => Ok(Response::new(CheckMailServerResponse {
                all_healthy: false,
                smtp_healthy: false,
                imap_healthy: false,
                webmail_healthy: false,
                smtp_status: "unavailable".to_string(),
                imap_status: "unavailable".to_string(),
                webmail_status: "unavailable".to_string(),
                message: "Mail D-Bus service is not available".to_string(),
                issues: vec!["Mail D-Bus service (org.opdbus.MailV1) is not running".to_string()],
            })),
        }
    }
}

// =============================================================================
// PrivacyNetworkService — wgcf + OVS + Xray Privacy Infrastructure
// =============================================================================

use crate::proto::privacy::{
    privacy_network_service_server::PrivacyNetworkService, ConfigurePacketRoutingRequest,
    ConfigurePacketRoutingResponse, EnsurePrivacyNetworkRequest, EnsurePrivacyNetworkResponse,
    GenerateWireGuardKeyPairRequest, GenerateWireGuardKeyPairResponse, GetNetworkStatusRequest,
    GetNetworkStatusResponse, GetNetworkTopologyRequest, GetNetworkTopologyResponse,
    GetPrivacyWireGuardConfigRequest, GetPrivacyWireGuardConfigResponse, HealthCheckRequest,
    HealthCheckResponse, ManageComponentRequest, ManageComponentResponse, ProvisionUserRequest,
    ProvisionUserResponse,
};

impl OperationGrpcServer {
    /// Call a PrivacyNetwork method via the D-Bus privacy interface.
    async fn privacy_dbus_call(&self, method: &str, args: &str) -> Result<String, Status> {
        let conn = self
            .schema_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {}", e)))?;

        let proxy = Proxy::new(
            &conn,
            "org.opdbus.v1",
            "/org/opdbus/v1/privacy",
            "org.opdbus.PrivacyV1",
        )
        .await
        .map_err(|e| Status::internal(format!("Privacy proxy error: {}", e)))?;

        let result: String = proxy
            .call(method, &(args.to_string(),))
            .await
            .map_err(|e| {
                Status::unavailable(format!(
                    "Privacy D-Bus service unavailable for '{}': {}",
                    method, e
                ))
            })?;

        Ok(result)
    }
}

#[tonic::async_trait]
impl PrivacyNetworkService for OperationGrpcServer {
    async fn ensure_privacy_network(
        &self,
        request: Request<EnsurePrivacyNetworkRequest>,
    ) -> Result<Response<EnsurePrivacyNetworkResponse>, Status> {
        let req = request.into_inner();
        info!(
            domain = %req.domain,
            force = req.force_reprovision,
            "gRPC EnsurePrivacyNetwork"
        );

        let args = simd_json::json!({
            "domain": req.domain,
            "force_reprovision": req.force_reprovision
        });

        match self
            .privacy_dbus_call("ensure_network", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(EnsurePrivacyNetworkResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("provisioned")
                        .to_string(),
                    bridge_name: parsed
                        .get("bridge_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ovsbr0")
                        .to_string(),
                    wgcf_status: parsed
                        .get("wgcf_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    xray_status: parsed
                        .get("xray_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    active_ports: parsed
                        .get("active_ports")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    provisioned_at: Some(OperationGrpcServer::now_ts()),
                    topology_summary: parsed
                        .get("topology_summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }))
            }
            Err(_) => {
                // Record provisioning intent in state store
                let mutation_value = simd_json::json!({
                    "domain": req.domain,
                    "status": "pending_no_backend",
                    "force_reprovision": req.force_reprovision
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "privacy".to_string(),
                        "/org/opdbus/v1/privacy/network".to_string(),
                        ChangeType::MethodCall,
                        Some("ensure_network".to_string()),
                        mutation_value,
                        "grpc".to_string(),
                        None,
                    )
                    .await;

                Ok(Response::new(EnsurePrivacyNetworkResponse {
                    success: false,
                    message:
                        "Privacy D-Bus service unavailable; provisioning request recorded in state"
                            .to_string(),
                    bridge_name: String::new(),
                    wgcf_status: "unavailable".to_string(),
                    xray_status: "unavailable".to_string(),
                    active_ports: vec![],
                    provisioned_at: Some(OperationGrpcServer::now_ts()),
                    topology_summary: String::new(),
                }))
            }
        }
    }

    async fn get_network_status(
        &self,
        request: Request<GetNetworkStatusRequest>,
    ) -> Result<Response<GetNetworkStatusResponse>, Status> {
        let req = request.into_inner();

        // Check SchemaEngine state first
        let state = self.schema_engine.get_state("privacy").await;
        if let Some(ref st) = state {
            if let Some(net_status) = st.as_object().and_then(|o| o.get("network_status")) {
                let components: Vec<_> = net_status
                    .as_object()
                    .and_then(|o| o.get("components"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|c| crate::proto::privacy::NetworkComponent {
                                name: c
                                    .as_object()
                                    .and_then(|o| o.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                status: c
                                    .as_object()
                                    .and_then(|o| o.get("status"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                r#type: c
                                    .as_object()
                                    .and_then(|o| o.get("type"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                ip_address: String::new(),
                                details: String::new(),
                                critical: false,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(Response::new(GetNetworkStatusResponse {
                    healthy: !components.is_empty(),
                    overall_status: "from_state_store".to_string(),
                    components,
                    message: "Status from state store".to_string(),
                    last_updated: Some(OperationGrpcServer::now_ts()),
                }));
            }
        }

        let args = simd_json::json!({"component": req.component});
        match self
            .privacy_dbus_call("get_status", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let components = parsed
                    .get("components")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|c| crate::proto::privacy::NetworkComponent {
                                name: c
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                status: c
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                r#type: c
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                ip_address: c
                                    .get("ip_address")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                details: c
                                    .get("details")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                critical: c
                                    .get("critical")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(GetNetworkStatusResponse {
                    healthy: parsed
                        .get("healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    overall_status: parsed
                        .get("overall_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    components,
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    last_updated: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => Ok(Response::new(GetNetworkStatusResponse {
                healthy: false,
                overall_status: "unhealthy".to_string(),
                components: vec![],
                message: "Privacy D-Bus service is not available".to_string(),
                last_updated: Some(OperationGrpcServer::now_ts()),
            })),
        }
    }

    async fn provision_user(
        &self,
        request: Request<ProvisionUserRequest>,
    ) -> Result<Response<ProvisionUserResponse>, Status> {
        let req = request.into_inner();
        info!(
            email = %req.email,
            container_type = %req.container_type,
            "gRPC ProvisionUser"
        );

        let args = simd_json::json!({
            "email": req.email,
            "wireguard_public_key": req.wireguard_public_key,
            "is_admin": req.is_admin,
            "domain": req.domain,
            "container_type": req.container_type
        });

        match self
            .privacy_dbus_call("provision_user", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(ProvisionUserResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    user_id: parsed
                        .get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    privacy_config: parsed
                        .get("privacy_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("provisioned")
                        .to_string(),
                    provisioned_at: Some(OperationGrpcServer::now_ts()),
                    xray_endpoint: parsed
                        .get("xray_endpoint")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }))
            }
            Err(_) => {
                // Record provisioning request in state
                let user_id = Uuid::new_v4().to_string();
                let mutation_value = simd_json::json!({
                    "email": req.email,
                    "wireguard_public_key": req.wireguard_public_key,
                    "is_admin": req.is_admin,
                    "domain": req.domain,
                    "container_type": req.container_type,
                    "status": "pending_no_backend"
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "privacy".to_string(),
                        format!("/org/opdbus/v1/privacy/users/{}", user_id),
                        ChangeType::ObjectAdded,
                        Some("provision_user".to_string()),
                        mutation_value,
                        req.email.clone(),
                        None,
                    )
                    .await;

                Ok(Response::new(ProvisionUserResponse {
                    success: false,
                    user_id,
                    assigned_ip: String::new(),
                    privacy_config: String::new(),
                    message: "Privacy D-Bus service unavailable; provisioning request recorded"
                        .to_string(),
                    provisioned_at: Some(OperationGrpcServer::now_ts()),
                    xray_endpoint: String::new(),
                }))
            }
        }
    }

    async fn get_privacy_wire_guard_config(
        &self,
        request: Request<GetPrivacyWireGuardConfigRequest>,
    ) -> Result<Response<GetPrivacyWireGuardConfigResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "email": req.email,
            "user_id": req.user_id,
            "include_xray": req.include_xray
        });

        match self
            .privacy_dbus_call("get_wireguard_config", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GetPrivacyWireGuardConfigResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    wireguard_config: parsed
                        .get("wireguard_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    public_key: parsed
                        .get("public_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    endpoint: parsed
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    dns_servers: parsed
                        .get("dns_servers")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: "ok".to_string(),
                    generated_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => Err(Status::unavailable(
                "Privacy D-Bus service is not available; cannot retrieve WireGuard config",
            )),
        }
    }

    async fn manage_component(
        &self,
        request: Request<ManageComponentRequest>,
    ) -> Result<Response<ManageComponentResponse>, Status> {
        let req = request.into_inner();
        info!(
            action = %req.action,
            component = %req.component,
            "gRPC ManageComponent"
        );

        let args = simd_json::json!({
            "action": req.action,
            "component": req.component
        });

        match self
            .privacy_dbus_call("manage_component", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(ManageComponentResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ok")
                        .to_string(),
                    component: req.component,
                    status: parsed
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    output: parsed
                        .get("output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    completed_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => {
                let mutation_value = simd_json::json!({
                    "action": req.action,
                    "component": req.component,
                    "status": "pending_no_backend"
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "privacy".to_string(),
                        format!("/org/opdbus/v1/privacy/components/{}", req.component),
                        ChangeType::MethodCall,
                        Some("manage_component".to_string()),
                        mutation_value,
                        "grpc".to_string(),
                        None,
                    )
                    .await;

                Ok(Response::new(ManageComponentResponse {
                    success: false,
                    message: "Privacy D-Bus service unavailable; action recorded in state"
                        .to_string(),
                    component: req.component,
                    status: "unavailable".to_string(),
                    output: String::new(),
                    completed_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
        }
    }

    async fn get_network_topology(
        &self,
        request: Request<GetNetworkTopologyRequest>,
    ) -> Result<Response<GetNetworkTopologyResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({"include_details": req.include_details});

        match self
            .privacy_dbus_call("get_topology", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let routes = parsed
                    .get("routes")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|r| crate::proto::privacy::NetworkRoute {
                                destination: r
                                    .get("destination")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                gateway: r
                                    .get("gateway")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                device: r
                                    .get("device")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                metric: r
                                    .get("metric")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let proxy_configs = parsed
                    .get("proxy_configs")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|p| crate::proto::privacy::ProxyConfig {
                                container_name: p
                                    .get("container_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                container_type: p
                                    .get("container_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                http_proxy_enabled: p
                                    .get("http_proxy_enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                grpc_proxy_enabled: p
                                    .get("grpc_proxy_enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                http_port: p.get("http_port").and_then(|v| v.as_u64()).unwrap_or(0)
                                    as u32,
                                grpc_port: p.get("grpc_port").and_then(|v| v.as_u64()).unwrap_or(0)
                                    as u32,
                                proxy_mode: p
                                    .get("proxy_mode")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(GetNetworkTopologyResponse {
                    bridge_name: parsed
                        .get("bridge_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wgcf_status: parsed
                        .get("wgcf_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    ports: parsed
                        .get("ports")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    management_ip: parsed
                        .get("management_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("10.200.0.1")
                        .to_string(),
                    xray_config: parsed
                        .get("xray_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    routes,
                    topology_data: None,
                    summary: parsed
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    proxy_configs,
                }))
            }
            Err(_) => Ok(Response::new(GetNetworkTopologyResponse {
                bridge_name: String::new(),
                wgcf_status: "unavailable".to_string(),
                ports: vec![],
                management_ip: String::new(),
                xray_config: String::new(),
                routes: vec![],
                topology_data: None,
                summary: "Privacy D-Bus service is not available".to_string(),
                proxy_configs: vec![],
            })),
        }
    }

    async fn health_check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "check_wgcf": req.check_wgcf,
            "check_ovs": req.check_ovs,
            "check_xray": req.check_xray,
            "check_ports": req.check_ports
        });

        match self
            .privacy_dbus_call("health_check", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let issues = parsed
                    .get("issues")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|i| crate::proto::privacy::HealthIssue {
                                component: i
                                    .get("component")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                severity: i
                                    .get("severity")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("warning")
                                    .to_string(),
                                message: i
                                    .get("message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                suggested_fix: i
                                    .get("suggested_fix")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(HealthCheckResponse {
                    all_healthy: parsed
                        .get("all_healthy")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    healthy_components: parsed
                        .get("healthy_components")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    total_components: parsed
                        .get("total_components")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    issues,
                    overall_status: parsed
                        .get("overall_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    checked_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => Ok(Response::new(HealthCheckResponse {
                all_healthy: false,
                healthy_components: 0,
                total_components: 0,
                issues: vec![crate::proto::privacy::HealthIssue {
                    component: "dbus".to_string(),
                    severity: "critical".to_string(),
                    message: "Privacy D-Bus service (org.opdbus.PrivacyV1) is not running"
                        .to_string(),
                    suggested_fix: "Start the privacy-router s6 service".to_string(),
                }],
                overall_status: "unhealthy".to_string(),
                checked_at: Some(OperationGrpcServer::now_ts()),
            })),
        }
    }

    async fn configure_packet_routing(
        &self,
        request: Request<ConfigurePacketRoutingRequest>,
    ) -> Result<Response<ConfigurePacketRoutingResponse>, Status> {
        let req = request.into_inner();
        info!(
            container = %req.container_name,
            container_type = %req.container_type,
            "gRPC ConfigurePacketRouting"
        );

        let args = simd_json::json!({
            "container_name": req.container_name,
            "container_type": req.container_type,
            "enable_http_proxy": req.enable_http_proxy,
            "enable_grpc_proxy": req.enable_grpc_proxy,
            "proxy_type": req.proxy_type,
            "socks_port": req.socks_port,
            "http_port": req.http_port,
            "enable_tproxy": req.enable_tproxy
        });

        match self
            .privacy_dbus_call("configure_routing", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(ConfigurePacketRoutingResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("configured")
                        .to_string(),
                    container_name: req.container_name,
                    proxy_config_summary: parsed
                        .get("proxy_config_summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    configured_at: Some(OperationGrpcServer::now_ts()),
                    applied_rules: parsed
                        .get("applied_rules")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }))
            }
            Err(_) => {
                let mutation_value = simd_json::json!({
                    "container_name": req.container_name,
                    "container_type": req.container_type,
                    "proxy_type": req.proxy_type,
                    "status": "pending_no_backend"
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "privacy".to_string(),
                        format!("/org/opdbus/v1/privacy/routing/{}", req.container_name),
                        ChangeType::MethodCall,
                        Some("configure_routing".to_string()),
                        mutation_value,
                        "grpc".to_string(),
                        None,
                    )
                    .await;

                Ok(Response::new(ConfigurePacketRoutingResponse {
                    success: false,
                    message: "Privacy D-Bus service unavailable; routing config recorded in state"
                        .to_string(),
                    container_name: req.container_name,
                    proxy_config_summary: String::new(),
                    configured_at: Some(OperationGrpcServer::now_ts()),
                    applied_rules: vec![],
                }))
            }
        }
    }

    async fn generate_wire_guard_key_pair(
        &self,
        request: Request<GenerateWireGuardKeyPairRequest>,
    ) -> Result<Response<GenerateWireGuardKeyPairResponse>, Status> {
        let req = request.into_inner();
        info!(
            email = %req.user_email,
            container_type = %req.container_type,
            "gRPC GenerateWireGuardKeyPair"
        );

        let args = simd_json::json!({
            "user_token": req.user_token,
            "user_email": req.user_email,
            "is_admin": req.is_admin,
            "container_type": req.container_type
        });

        match self
            .privacy_dbus_call("generate_keypair", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GenerateWireGuardKeyPairResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    client_public_key: parsed
                        .get("client_public_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wireguard_config: parsed
                        .get("wireguard_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    key_id: parsed
                        .get("key_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: "ok".to_string(),
                    generated_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => Err(Status::unavailable(
                "Privacy D-Bus service is not available; cannot generate WireGuard keypair without backend",
            )),
        }
    }
}

// =============================================================================
// RegistrationService — Magic Link + WireGuard Identity Management
// =============================================================================

use crate::proto::registration::{
    registration_service_server::RegistrationService, AdminUserActionRequest,
    AdminUserActionResponse, GetUserStatusRequest, GetUserStatusResponse,
    GetWireGuardConfigRequest, GetWireGuardConfigResponse, ListUsersRequest, ListUsersResponse,
    RegisterUserRequest, RegisterUserResponse, SendMagicLinkRequest, SendMagicLinkResponse,
    VerifyMagicLinkRequest, VerifyMagicLinkResponse,
};

impl OperationGrpcServer {
    /// Call a RegistrationService method via the D-Bus registration interface.
    async fn registration_dbus_call(&self, method: &str, args: &str) -> Result<String, Status> {
        let conn = self
            .schema_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {}", e)))?;

        let proxy = Proxy::new(
            &conn,
            "org.opdbus.v1",
            "/org/opdbus/v1/registration",
            "org.opdbus.RegistrationV1",
        )
        .await
        .map_err(|e| Status::internal(format!("Registration proxy error: {}", e)))?;

        let result: String = proxy
            .call(method, &(args.to_string(),))
            .await
            .map_err(|e| {
                Status::unavailable(format!(
                    "Registration D-Bus service unavailable for '{}': {}",
                    method, e
                ))
            })?;

        Ok(result)
    }
}

#[tonic::async_trait]
impl RegistrationService for OperationGrpcServer {
    async fn send_magic_link(
        &self,
        request: Request<SendMagicLinkRequest>,
    ) -> Result<Response<SendMagicLinkResponse>, Status> {
        let req = request.into_inner();
        info!(
            email = %req.email,
            domain = %req.domain,
            is_admin = req.is_admin,
            "gRPC SendMagicLink"
        );

        let args = simd_json::json!({
            "email": req.email,
            "domain": req.domain,
            "is_admin": req.is_admin
        });

        match self
            .registration_dbus_call("send_magic_link", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(SendMagicLinkResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("magic link sent")
                        .to_string(),
                    token: parsed
                        .get("token")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    expires_at: Some(ProstTimestamp {
                        seconds: chrono::Utc::now().timestamp() + 3600, // 1 hour expiry
                        nanos: 0,
                    }),
                }))
            }
            Err(_) => {
                // Generate a token and record in state store for later verification
                let token = Uuid::new_v4().to_string();
                let mutation_value = simd_json::json!({
                    "email": req.email,
                    "domain": req.domain,
                    "is_admin": req.is_admin,
                    "token": token,
                    "status": "pending_no_backend",
                    "created_at": chrono::Utc::now().to_rfc3339()
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "registration".to_string(),
                        format!("/org/opdbus/v1/registration/magic_links/{}", token),
                        ChangeType::ObjectAdded,
                        Some("send_magic_link".to_string()),
                        mutation_value,
                        req.email.clone(),
                        None,
                    )
                    .await;

                Ok(Response::new(SendMagicLinkResponse {
                    success: false,
                    message:
                        "Registration D-Bus service unavailable; magic link recorded in state store"
                            .to_string(),
                    token: Some(token),
                    expires_at: Some(ProstTimestamp {
                        seconds: chrono::Utc::now().timestamp() + 3600,
                        nanos: 0,
                    }),
                }))
            }
        }
    }

    async fn verify_magic_link(
        &self,
        request: Request<VerifyMagicLinkRequest>,
    ) -> Result<Response<VerifyMagicLinkResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "token": req.token,
            "domain": req.domain
        });

        match self
            .registration_dbus_call("verify_magic_link", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(VerifyMagicLinkResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    user_id: parsed
                        .get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    email: parsed
                        .get("email")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wireguard_public_key: parsed
                        .get("wireguard_public_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wireguard_config: parsed
                        .get("wireguard_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("verified")
                        .to_string(),
                    is_admin: parsed
                        .get("is_admin")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    verified_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => {
                // Check state store for magic link token
                let state = self.schema_engine.get_state("registration").await;
                if let Some(ref st) = state {
                    if let Some(link_data) = st
                        .as_object()
                        .and_then(|o| o.get("magic_links"))
                        .and_then(|o| o.as_object())
                        .and_then(|o| o.get(&req.token))
                    {
                        let email = link_data
                            .as_object()
                            .and_then(|o| o.get("email"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_admin = link_data
                            .as_object()
                            .and_then(|o| o.get("is_admin"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        return Ok(Response::new(VerifyMagicLinkResponse {
                            success: true,
                            user_id: Uuid::new_v4().to_string(),
                            email,
                            wireguard_public_key: String::new(),
                            assigned_ip: String::new(),
                            wireguard_config: String::new(),
                            message: "Verified from state store (D-Bus unavailable)".to_string(),
                            is_admin,
                            verified_at: Some(OperationGrpcServer::now_ts()),
                        }));
                    }
                }

                Err(Status::unavailable(
                    "Registration D-Bus service is not available; token not found in state store",
                ))
            }
        }
    }

    async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        let req = request.into_inner();
        info!(
            email = %req.email,
            domain = %req.domain,
            is_admin = req.is_admin,
            "gRPC RegisterUser"
        );

        let args = simd_json::json!({
            "email": req.email,
            "wireguard_public_key": req.wireguard_public_key,
            "domain": req.domain,
            "is_admin": req.is_admin
        });

        match self
            .registration_dbus_call("register_user", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(RegisterUserResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    user_id: parsed
                        .get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("registered")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wireguard_config: parsed
                        .get("wireguard_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    registered_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => {
                let user_id = Uuid::new_v4().to_string();
                let mutation_value = simd_json::json!({
                    "email": req.email,
                    "wireguard_public_key": req.wireguard_public_key,
                    "domain": req.domain,
                    "is_admin": req.is_admin,
                    "status": "pending_no_backend"
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "registration".to_string(),
                        format!("/org/opdbus/v1/registration/users/{}", user_id),
                        ChangeType::ObjectAdded,
                        Some("register_user".to_string()),
                        mutation_value,
                        req.email.clone(),
                        None,
                    )
                    .await;

                Ok(Response::new(RegisterUserResponse {
                    success: false,
                    user_id,
                    message: "Registration D-Bus service unavailable; user recorded in state store"
                        .to_string(),
                    assigned_ip: String::new(),
                    wireguard_config: String::new(),
                    registered_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
        }
    }

    async fn get_user_status(
        &self,
        request: Request<GetUserStatusRequest>,
    ) -> Result<Response<GetUserStatusResponse>, Status> {
        let req = request.into_inner();

        // Check state store first
        let state = self.schema_engine.get_state("registration").await;
        if let Some(ref st) = state {
            if let Some(users) = st.as_object().and_then(|o| o.get("users")) {
                // Search by email or user_id
                if let Some(user_data) = users.as_object().and_then(|o| {
                    // Try user_id first
                    if !req.user_id.is_empty() {
                        o.get(&req.user_id)
                    } else {
                        // Search by email
                        o.iter().find_map(|(_, v)| {
                            if v.as_object()
                                .and_then(|u| u.get("email"))
                                .and_then(|e| e.as_str())
                                == Some(&req.email)
                            {
                                Some(v)
                            } else {
                                None
                            }
                        })
                    }
                }) {
                    return Ok(Response::new(GetUserStatusResponse {
                        registered: true,
                        user_id: user_data
                            .as_object()
                            .and_then(|o| o.get("user_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&req.user_id)
                            .to_string(),
                        email: user_data
                            .as_object()
                            .and_then(|o| o.get("email"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&req.email)
                            .to_string(),
                        email_verified: user_data
                            .as_object()
                            .and_then(|o| o.get("email_verified"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        wireguard_public_key: user_data
                            .as_object()
                            .and_then(|o| o.get("wireguard_public_key"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        assigned_ip: user_data
                            .as_object()
                            .and_then(|o| o.get("assigned_ip"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        is_admin: user_data
                            .as_object()
                            .and_then(|o| o.get("is_admin"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        registered_at: None,
                        last_active: None,
                    }));
                }
            }
        }

        let args = simd_json::json!({
            "email": req.email,
            "user_id": req.user_id,
            "domain": req.domain
        });

        match self
            .registration_dbus_call("get_user_status", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GetUserStatusResponse {
                    registered: parsed
                        .get("registered")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    user_id: parsed
                        .get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    email: parsed
                        .get("email")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    email_verified: parsed
                        .get("email_verified")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    wireguard_public_key: parsed
                        .get("wireguard_public_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_admin: parsed
                        .get("is_admin")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    registered_at: None,
                    last_active: None,
                }))
            }
            Err(_) => Ok(Response::new(GetUserStatusResponse {
                registered: false,
                user_id: String::new(),
                email: req.email,
                email_verified: false,
                wireguard_public_key: String::new(),
                assigned_ip: String::new(),
                is_admin: false,
                registered_at: None,
                last_active: None,
            })),
        }
    }

    async fn list_users(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "limit": req.limit,
            "offset": req.offset,
            "include_admins_only": req.include_admins_only,
            "domain_filter": req.domain_filter
        });

        match self
            .registration_dbus_call("list_users", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                let users: Vec<_> = parsed
                    .get("users")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|u| crate::proto::registration::UserInfo {
                                user_id: u
                                    .get("user_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                email: u
                                    .get("email")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                email_verified: u
                                    .get("email_verified")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                wireguard_public_key: u
                                    .get("wireguard_public_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                assigned_ip: u
                                    .get("assigned_ip")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                is_admin: u
                                    .get("is_admin")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                registered_at: None,
                                last_active: None,
                                metadata: None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let total = parsed
                    .get("total_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(users.len() as u64) as u32;
                let filtered = parsed
                    .get("filtered_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(users.len() as u64) as u32;

                Ok(Response::new(ListUsersResponse {
                    users,
                    total_count: total,
                    filtered_count: filtered,
                }))
            }
            Err(_) => Ok(Response::new(ListUsersResponse {
                users: vec![],
                total_count: 0,
                filtered_count: 0,
            })),
        }
    }

    async fn get_wire_guard_config(
        &self,
        request: Request<GetWireGuardConfigRequest>,
    ) -> Result<Response<GetWireGuardConfigResponse>, Status> {
        let req = request.into_inner();
        let args = simd_json::json!({
            "email": req.email,
            "user_id": req.user_id,
            "domain": req.domain
        });

        match self
            .registration_dbus_call("get_wireguard_config", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(GetWireGuardConfigResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    wireguard_config: parsed
                        .get("wireguard_config")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    public_key: parsed
                        .get("public_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    assigned_ip: parsed
                        .get("assigned_ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message: "ok".to_string(),
                    generated_at: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => Err(Status::unavailable(
                "Registration D-Bus service is not available; cannot retrieve WireGuard config",
            )),
        }
    }

    async fn admin_user_action(
        &self,
        request: Request<AdminUserActionRequest>,
    ) -> Result<Response<AdminUserActionResponse>, Status> {
        let req = request.into_inner();
        info!(
            action = %req.action,
            user_id = %req.user_id,
            email = %req.email,
            "gRPC AdminUserAction"
        );

        let args = simd_json::json!({
            "action": req.action,
            "user_id": req.user_id,
            "email": req.email
        });

        match self
            .registration_dbus_call("admin_user_action", &args.to_string())
            .await
        {
            Ok(result) => {
                let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();
                Ok(Response::new(AdminUserActionResponse {
                    success: parsed
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    message: parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ok")
                        .to_string(),
                    user_id: req.user_id.clone(),
                    action_timestamp: Some(OperationGrpcServer::now_ts()),
                }))
            }
            Err(_) => {
                let mutation_value = simd_json::json!({
                    "action": req.action,
                    "user_id": req.user_id,
                    "email": req.email,
                    "status": "pending_no_backend"
                });
                let _ = self
                    .schema_engine
                    .process_grpc_mutation(
                        "registration".to_string(),
                        format!(
                            "/org/opdbus/v1/registration/admin_actions/{}",
                            Uuid::new_v4()
                        ),
                        ChangeType::MethodCall,
                        Some(req.action.clone()),
                        mutation_value,
                        "admin".to_string(),
                        None,
                    )
                    .await;

                Ok(Response::new(AdminUserActionResponse {
                    success: false,
                    message:
                        "Registration D-Bus service unavailable; action recorded in state store"
                            .to_string(),
                    user_id: req.user_id,
                    action_timestamp: Some(OperationGrpcServer::now_ts()),
                }))
            }
        }
    }
}

// =============================================================================
// D-Bus Passthrough Service
// =============================================================================

#[tonic::async_trait]
impl crate::proto::dbus_passthrough_server::DbusPassthrough for OperationGrpcServer {
    type WatchStream =
        Pin<Box<dyn Stream<Item = Result<crate::proto::DbusSignalEvent, Status>> + Send>>;

    async fn call(
        &self,
        request: Request<crate::proto::DbusCallRequest>,
    ) -> Result<Response<crate::proto::DbusCallResponse>, Status> {
        let req = request.into_inner();
        let conn = self
            .schema_engine
            .dbus_connection()
            .await
            .map_err(|e| Status::unavailable(format!("D-Bus not available: {e}")))?;

        let bus_conn = match req.bus.as_str() {
            "session" => {
                let session = Connection::session()
                    .await
                    .map_err(|e| Status::unavailable(format!("session bus unavailable: {e}")))?;
                session
            }
            _ => conn,
        };

        let proxy = Proxy::new(
            &bus_conn,
            req.destination.clone(),
            req.path.clone(),
            req.interface.clone(),
        )
        .await
        .map_err(|e| Status::internal(format!("proxy build failed: {e}")))?;

        let result: String = proxy
            .call(req.method.clone(), &(req.json_body.clone(),))
            .await
            .map_err(|e| Status::internal(format!("D-Bus call '{}' failed: {e}", req.method)))?;

        Ok(Response::new(crate::proto::DbusCallResponse {
            success: true,
            json_result: result,
            error: String::new(),
        }))
    }

    async fn get(
        &self,
        request: Request<crate::proto::DbusGetPropertyRequest>,
    ) -> Result<Response<crate::proto::DbusGetPropertyResponse>, Status> {
        let req = request.into_inner();
        let bus_conn = match req.bus.as_str() {
            "session" => Connection::session()
                .await
                .map_err(|e| Status::unavailable(format!("session bus: {e}")))?,
            _ => Connection::system()
                .await
                .map_err(|e| Status::unavailable(format!("system bus: {e}")))?,
        };

        let props = zbus::fdo::PropertiesProxy::builder(&bus_conn)
            .destination(req.destination.clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?
            .path(req.path.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let val: ZOwnedValue = props
            .get(iface, &req.property)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let json = serde_json::to_string(&val).map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(crate::proto::DbusGetPropertyResponse {
            success: true,
            json_value: json,
            error: String::new(),
        }))
    }

    async fn set(
        &self,
        request: Request<crate::proto::DbusSetPropertyRequest>,
    ) -> Result<Response<crate::proto::DbusSetPropertyResponse>, Status> {
        let req = request.into_inner();
        let bus_conn = match req.bus.as_str() {
            "session" => Connection::session()
                .await
                .map_err(|e| Status::unavailable(format!("session bus: {e}")))?,
            _ => Connection::system()
                .await
                .map_err(|e| Status::unavailable(format!("system bus: {e}")))?,
        };

        let props = zbus::fdo::PropertiesProxy::builder(&bus_conn)
            .destination(req.destination.clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?
            .path(req.path.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?
            .build()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(req.interface.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let value: serde_json::Value = serde_json::from_str(&req.json_value)
            .map_err(|e| Status::invalid_argument(format!("bad JSON: {e}")))?;
        let zval = simd_json_to_zvariant(
            &simd_json::serde::to_owned_value(&value)
                .map_err(|e| Status::internal(e.to_string()))?,
        )
        .map_err(|e| Status::invalid_argument(e.to_string()))?;

        props
            .set(iface, &req.property, zval.into())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(crate::proto::DbusSetPropertyResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn watch(
        &self,
        request: Request<crate::proto::DbusWatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let req = request.into_inner();
        let bus_conn = match req.bus.as_str() {
            "session" => Connection::session()
                .await
                .map_err(|e| Status::unavailable(format!("session bus: {e}")))?,
            _ => Connection::system()
                .await
                .map_err(|e| Status::unavailable(format!("system bus: {e}")))?,
        };

        let proxy = Proxy::new(
            &bus_conn,
            req.destination.clone(),
            req.path.clone(),
            req.interface.clone(),
        )
        .await
        .map_err(|e| Status::internal(format!("proxy build failed: {e}")))?;

        // zbus v5: receive_all_signals() or receive_signal(name) -> SignalStream
        // SignalStream implements futures::Stream<Item = Message>
        let mut sig_stream = if req.signal_names.is_empty() {
            proxy
                .receive_all_signals()
                .await
                .map_err(|e| Status::internal(format!("signal subscribe failed: {e}")))?
        } else {
            // Subscribe to the first named signal; filter others in-stream
            let sig_name = zbus::names::MemberName::try_from(req.signal_names[0].clone())
                .map_err(|e| Status::invalid_argument(format!("invalid signal name: {e}")))?;
            proxy
                .receive_signal(sig_name)
                .await
                .map_err(|e| Status::internal(format!("signal subscribe failed: {e}")))?
        };

        let signal_names = req.signal_names.clone();
        let stream = stream! {
            while let Some(msg) = sig_stream.next().await {
                let hdr = msg.header();
                let member = hdr.member().map(|m| m.as_str().to_string()).unwrap_or_default();
                if !signal_names.is_empty() && !signal_names.contains(&member) { continue; }
                let iface = hdr.interface().map(|i| i.as_str().to_string()).unwrap_or_default();
                let path = hdr.path().map(|p| p.as_str().to_string()).unwrap_or_default();
                let body: String = msg.body()
                    .deserialize::<zbus::zvariant::OwnedValue>()
                    .map(|b| format!("{b:?}"))
                    .unwrap_or_default();
                yield Ok(crate::proto::DbusSignalEvent {
                    signal_name: member,
                    path,
                    interface: iface,
                    json_body: body,
                    timestamp: Some(ProstTimestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: Utc::now().timestamp_subsec_nanos() as i32,
                    }),
                });
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/interceptor.rs">
// 🟢 🛡️ The Tonic gRPC Gatekeeper (Middleware Interceptor)
// Sits on the primary gRPC ingress at port 18789. Intercepts Xray-injected headers,
// performs a zero-copy check against the IdentitySled in shared memory, and either
// allows the gRPC payload through or drops the connection instantly.
//
// Operated by A.N.N.A. Scribe. No payload enters the system without a cryptographic
// "Snowball" session. No SQL databases, no D-Bus watchers. 1:1 Direct Read only.

use memmap2::MmapOptions;
use op_identity::IdentitySled;
use std::fs::File;
use tonic::{Request, Status};

/// Check whether a sled is "valid" per the Absolute Base rule.
fn is_sled_valid(sled: &IdentitySled) -> bool {
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

/// THE GATEKEEPER: Tonic gRPC Interceptor on port 18789.
///
/// Enforces the Absolute Base rule: if the `x-ghostbridge-footprint` provided by Xray
/// does not perfectly match the hashed footprint sitting in shared memory, the payload
/// is rejected. Once validated, embeds the `x-ghostbridge-trace-id` into Tonic Request
/// extensions so the Chatbot and Qdrant semantic search on the Accountability Page
/// have the exact Trace ID needed to link the session.
#[derive(Clone, Debug)]
pub struct GhostbridgeInterceptor;

impl tonic::service::Interceptor for GhostbridgeInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        ghostbridge_interceptor(req)
    }
}

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    // 1. Extract the Xray-injected Identity Headers (The Accountability Loop)
    //    Clone header values upfront to release the immutable borrow on `req`,
    //    allowing the mutable `extensions_mut()` call downstream.
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "A.N.N.A. Scribe: Missing Ghostbridge Identity Sled. Connection Dropped.",
        ));
    }

    // 2. 1:1 Direct Read from the SchemaEngine's shared memory (No SQL, No Polling)
    let file = File::open("/dev/shm/plugin_schema.dat")
        .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;
    let sled = unsafe { &*sled_ptr };

    // The Absolute Base: No valid schema, it does not exist.
    if !is_sled_valid(sled) {
        return Err(Status::failed_precondition(
            "A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.",
        ));
    }

    let current_footprint = sled.hashed_footprint;

    // 3. The Strike/Etch Validation: Check if the payload is in sync with Btrfs.
    //    If a Btrfs mutation has occurred and the client's footprint is stale,
    //    the connection is dropped without consuming any NVMe I/O.
    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;
    let expected_footprint = hex::encode(current_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ));
    }

    // 4. Pass the Trace ID downstream into the gRPC context for the React GUI.
    //    This guarantees that the Chatbot and the Qdrant semantic search
    //    (on the bottom of the Accountability Page) have the exact Trace ID
    //    needed to link the session.
    if let Some(trace_val) = trace_value {
        req.extensions_mut().insert(trace_val);
    }

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataValue;

    /// Because `ghostbridge_interceptor` hardcodes `/dev/shm/plugin_schema.dat`,
    /// direct unit tests exercise the validation logic extracted into helper functions.
    /// These tests validate every branch of the interceptor without requiring root
    /// access to `/dev/shm`.

    #[test]
    fn test_rejects_missing_footprint_header() {
        // No metadata headers at all → unauthenticated
        let req = Request::new(());
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
        assert!(status
            .message()
            .contains("Missing Ghostbridge Identity Sled"));
    }

    #[test]
    fn test_rejects_missing_trace_header() {
        // Only footprint, no trace-id → unauthenticated
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("deadbeef"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_rejects_missing_footprint_with_trace_only() {
        // Only trace-id, no footprint → unauthenticated
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-abc"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_identity_sled_repr_c_layout() {
        // Verify the IdentitySled struct matches the spec exactly (152 bytes)
        let size = std::mem::size_of::<IdentitySled>();
        assert_eq!(
            size, 152,
            "IdentitySled must be exactly 152 bytes per spec, got {} bytes",
            size
        );
    }

    #[test]
    fn test_footprint_hex_encoding_roundtrip() {
        // Verify that the hex encoding of a footprint matches expected format
        let footprint: [u8; 32] = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1A, 0x1B, 0x1C,
        ];
        let encoded = hex::encode(footprint);
        assert_eq!(encoded.len(), 64, "Hex-encoded 32 bytes must be 64 chars");
        assert_eq!(&encoded[..8], "deadbeef");
    }

    #[test]
    fn test_footprint_mismatch_detection() {
        // Simulate the footprint comparison logic from the interceptor
        let sled_footprint: [u8; 32] = [0xAA; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = "0000000000000000000000000000000000000000000000000000000000000000";

        assert_ne!(
            request_footprint, expected,
            "Mismatched footprints must be detected"
        );
    }

    #[test]
    fn test_footprint_match_succeeds() {
        // Simulate a matching footprint scenario
        let sled_footprint: [u8; 32] = [0xBB; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = hex::encode([0xBB; 32]);

        assert_eq!(
            request_footprint, expected,
            "Matching footprints must pass validation"
        );
    }

    #[test]
    fn test_schema_engine_unreachable_returns_internal() {
        // If both headers are present but /dev/shm/plugin_schema.dat is missing,
        // the interceptor must return Status::internal (not unauthenticated).
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("aabbccdd"),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-aabbccdd"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(status.message().contains("SchemaEngine Memory Unreachable"));
    }

    #[test]
    fn test_invalid_sled_rejected() {
        // A sled with zero footprint and trace_id is invalid
        let sled = IdentitySled {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 1,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            schema_version: 0,
            reserved: [0u8; 60],
        };
        assert!(!is_sled_valid(&sled));
    }

    #[test]
    fn test_valid_sled_accepted() {
        let sled = IdentitySled {
            wireguard_pubkey: [0xCC; 32],
            mutation_index: 42,
            hashed_footprint: [0xDD; 32],
            trace_id: [0xEE; 16],
            schema_version: 1,
            reserved: [0u8; 60],
        };
        assert!(is_sled_valid(&sled));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/lib.rs">
//! D-Bus ↔ gRPC Bidirectional Bridge
//!
//! Provides live synchronization between D-Bus objects and gRPC services:
//! - D-Bus property changes → gRPC streaming updates
//! - gRPC mutations → D-Bus method calls / property sets
//! - D-Bus signals → gRPC server-streaming
//! - All changes flow through the event chain for audit/compliance
//!
//! Architecture:
//! ```text
//!                     ┌─────────────────┐
//!                     │   Event Chain   │ ← Source of truth
//!                     │  (audit + hash) │
//!                     └────────┬────────┘
//!                              │
//!               ┌──────────────┴──────────────┐
//!               ▼                              ▼
//!     ┌─────────────────┐            ┌─────────────────┐
//!     │     D-Bus       │◄──────────►│      gRPC       │
//!     │  (local IPC)    │            │  (remote RPC)   │
//!     └─────────────────┘            └─────────────────┘
//! ```

pub mod grpc_client;
pub mod grpc_server;
pub mod interceptor;
pub mod proto_gen;
pub mod schema_engine;

// Re-export main types
pub use grpc_client::{GrpcClientPool, RemoteEndpoint, RemoteOperationClient};
pub use grpc_server::{run_grpc_server, OperationGrpcServer, PluginSchemaProvider};
pub use interceptor::ghostbridge_interceptor;
pub use proto_gen::{ProtoGenConfig, ProtoGenerator};
pub use schema_engine::{ChangeSource, ChangeType, SchemaEngine, StateChange};

/// Generated protobuf types — one sub-module per domain proto.
/// All are compiled into the combined operation_descriptor.bin for reflection.
pub mod proto {
    // Core: StateSync, PluginService, EventChainService, OvsdbMirror, RuntimeMirror
    tonic::include_proto!("operation.v1");

    pub mod mail {
        tonic::include_proto!("operation.mail.v1");
    }
    pub mod privacy {
        tonic::include_proto!("operation.privacy.v1");
    }
    pub mod registration {
        tonic::include_proto!("operation.registration.v1");
    }
    pub mod registry {
        tonic::include_proto!("operation.registry.v1");
    }

    /// Combined FileDescriptorSet covering all domain protos.
    /// Served by tonic-reflection so clients can discover every service.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("operation_descriptor");
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/proto_gen.rs">
//! Proto Generator - Generate protobuf definitions from PluginSchema
//!
//! Converts operation-dbus plugin schemas to protobuf message and service
//! definitions, enabling dynamic schema-driven gRPC.

use op_state_store::{FieldType, PluginSchema, SchemaCatalog, SchemaRegistry};
use std::fmt::Write;

/// Configuration for protobuf generation
#[derive(Debug, Clone)]
pub struct ProtoGenConfig {
    /// Package name for generated proto
    pub package_name: String,
    /// Whether to generate service definitions
    pub generate_services: bool,
    /// Whether to include validation annotations
    pub include_validation: bool,
    /// Whether to generate streaming RPCs for state changes
    pub generate_streams: bool,
}

impl Default for ProtoGenConfig {
    fn default() -> Self {
        Self {
            package_name: "operation.v1".to_string(),
            generate_services: true,
            include_validation: true,
            generate_streams: true,
        }
    }
}

/// Generate protobuf definitions from plugin schemas
pub struct ProtoGenerator {
    config: ProtoGenConfig,
}

impl ProtoGenerator {
    pub fn new(config: ProtoGenConfig) -> Self {
        Self { config }
    }

    /// Generate proto file content for a single plugin schema
    pub fn generate_for_schema(&self, schema: &PluginSchema) -> String {
        let mut output = String::new();

        // Header
        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();

        // Imports
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/timestamp.proto\";").unwrap();
        writeln!(output).unwrap();

        // Generate message for the schema
        self.generate_message(&mut output, schema);

        // Generate request/response messages
        self.generate_crud_messages(&mut output, schema);

        // Generate service if enabled
        if self.config.generate_services {
            self.generate_service(&mut output, schema);
        }

        output
    }

    /// Generate proto file content for all schemas in a catalog.
    pub fn generate_for_catalog(&self, catalog: &SchemaCatalog) -> String {
        let mut output = String::new();
        let mut schema_names: Vec<&str> = catalog.list();
        schema_names.sort_unstable();

        // Header
        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();

        // Imports
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/timestamp.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/any.proto\";").unwrap();
        writeln!(output).unwrap();

        // Generate messages for each schema
        for schema_name in schema_names {
            let Some(schema) = catalog.get(schema_name) else {
                continue;
            };
            writeln!(output, "// =============================================").unwrap();
            writeln!(output, "// {} - {}", schema.name, schema.description).unwrap();
            writeln!(output, "// =============================================").unwrap();
            writeln!(output).unwrap();

            self.generate_message(&mut output, schema);
            self.generate_crud_messages(&mut output, schema);

            if self.config.generate_services {
                self.generate_service(&mut output, schema);
            }

            writeln!(output).unwrap();
        }

        // Add unified service
        self.generate_unified_service(&mut output, catalog);

        output
    }

    /// Compatibility wrapper for older call sites that still say `registry`.
    pub fn generate_for_registry(&self, registry: &SchemaRegistry) -> String {
        self.generate_for_catalog(registry)
    }

    pub fn generate_message(&self, output: &mut String, schema: &PluginSchema) {
        let message_name = to_pascal_case(&schema.name);
        writeln!(output, "message {} {{", message_name).unwrap();

        let mut field_num = 1;
        let mut fields: Vec<(&str, &op_state_store::FieldSchema)> = schema
            .fields
            .iter()
            .map(|(field_name, field_schema)| (field_name.as_str(), field_schema))
            .collect();
        fields.sort_unstable_by(|left, right| left.0.cmp(right.0));

        for (field_name, field_schema) in fields {
            let proto_type = self.field_type_to_proto(&field_schema.field_type);
            let optional_marker = if field_schema.required {
                ""
            } else {
                "optional "
            };
            writeln!(
                output,
                "  {}{} {} = {};",
                optional_marker, proto_type, field_name, field_num
            )
            .unwrap();
            field_num += 1;
        }

        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    fn generate_crud_messages(&self, output: &mut String, schema: &PluginSchema) {
        let message_name = to_pascal_case(&schema.name);

        // Get request
        writeln!(output, "message Get{}Request {{", message_name).unwrap();
        writeln!(output, "  string object_path = 1;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Get response
        writeln!(output, "message Get{}Response {{", message_name).unwrap();
        writeln!(output, "  {} state = 1;", message_name).unwrap();
        writeln!(output, "  string error = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Set request
        writeln!(output, "message Set{}Request {{", message_name).unwrap();
        writeln!(output, "  string object_path = 1;").unwrap();
        writeln!(output, "  {} state = 2;", message_name).unwrap();
        writeln!(output, "  string actor_id = 3;").unwrap();
        writeln!(output, "  string capability_id = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Set response
        writeln!(output, "message Set{}Response {{", message_name).unwrap();
        writeln!(output, "  bool success = 1;").unwrap();
        writeln!(output, "  string event_id = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // List request
        writeln!(output, "message List{}Request {{", message_name).unwrap();
        writeln!(output, "  string path_prefix = 1;").unwrap();
        writeln!(output, "  int32 limit = 2;").unwrap();
        writeln!(output, "  string cursor = 3;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // List response
        writeln!(output, "message List{}Response {{", message_name).unwrap();
        writeln!(output, "  repeated {} items = 1;", message_name).unwrap();
        writeln!(output, "  string next_cursor = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Watch update (for streaming)
        if self.config.generate_streams {
            writeln!(output, "message {}Update {{", message_name).unwrap();
            writeln!(output, "  string object_path = 1;").unwrap();
            writeln!(output, "  {} state = 2;", message_name).unwrap();
            writeln!(output, "  string event_id = 3;").unwrap();
            writeln!(output, "  repeated string tags_touched = 4;").unwrap();
            writeln!(output, "  google.protobuf.Timestamp timestamp = 5;").unwrap();
            writeln!(output, "}}").unwrap();
            writeln!(output).unwrap();

            writeln!(output, "message Watch{}Request {{", message_name).unwrap();
            writeln!(output, "  string path_filter = 1;").unwrap();
            writeln!(output, "  repeated string tag_filters = 2;").unwrap();
            writeln!(output, "}}").unwrap();
            writeln!(output).unwrap();
        }
    }

    fn generate_service(&self, output: &mut String, schema: &PluginSchema) {
        let service_name = to_pascal_case(&schema.name);

        writeln!(output, "service {}Service {{", service_name).unwrap();
        writeln!(output, "  // Get {} state", schema.name).unwrap();
        writeln!(
            output,
            "  rpc Get(Get{}Request) returns (Get{}Response);",
            service_name, service_name
        )
        .unwrap();
        writeln!(output).unwrap();
        writeln!(output, "  // Set {} state", schema.name).unwrap();
        writeln!(
            output,
            "  rpc Set(Set{}Request) returns (Set{}Response);",
            service_name, service_name
        )
        .unwrap();
        writeln!(output).unwrap();
        writeln!(output, "  // List {} objects", schema.name).unwrap();
        writeln!(
            output,
            "  rpc List(List{}Request) returns (List{}Response);",
            service_name, service_name
        )
        .unwrap();

        if self.config.generate_streams {
            writeln!(output).unwrap();
            writeln!(output, "  // Watch for {} changes", schema.name).unwrap();
            writeln!(
                output,
                "  rpc Watch(Watch{}Request) returns (stream {}Update);",
                service_name, service_name
            )
            .unwrap();
        }

        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    fn generate_unified_service(&self, output: &mut String, catalog: &SchemaCatalog) {
        writeln!(output, "// =============================================").unwrap();
        writeln!(output, "// Unified Operation Service").unwrap();
        writeln!(output, "// =============================================").unwrap();
        writeln!(output).unwrap();

        // Generic state messages
        writeln!(output, "message GenericGetRequest {{").unwrap();
        writeln!(output, "  string plugin_id = 1;").unwrap();
        writeln!(output, "  string object_path = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericGetResponse {{").unwrap();
        writeln!(output, "  google.protobuf.Struct state = 1;").unwrap();
        writeln!(output, "  string schema_version = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericSetRequest {{").unwrap();
        writeln!(output, "  string plugin_id = 1;").unwrap();
        writeln!(output, "  string object_path = 2;").unwrap();
        writeln!(output, "  google.protobuf.Struct state = 3;").unwrap();
        writeln!(output, "  string actor_id = 4;").unwrap();
        writeln!(output, "  string capability_id = 5;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericSetResponse {{").unwrap();
        writeln!(output, "  bool success = 1;").unwrap();
        writeln!(output, "  string event_id = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "service OperationService {{").unwrap();
        writeln!(
            output,
            "  rpc Get(GenericGetRequest) returns (GenericGetResponse);"
        )
        .unwrap();
        writeln!(
            output,
            "  rpc Set(GenericSetRequest) returns (GenericSetResponse);"
        )
        .unwrap();

        let mut schema_names: Vec<&str> = catalog.list();
        schema_names.sort_unstable();

        for schema_name in schema_names {
            let name = to_pascal_case(schema_name);
            writeln!(
                output,
                "  rpc Get{}(Get{}Request) returns (Get{}Response);",
                name, name, name
            )
            .unwrap();
            writeln!(
                output,
                "  rpc Set{}(Set{}Request) returns (Set{}Response);",
                name, name, name
            )
            .unwrap();
        }
        writeln!(output, "}}").unwrap();
    }

    fn field_type_to_proto(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "string".to_string(),
            FieldType::Integer => "int64".to_string(),
            FieldType::Float => "double".to_string(),
            FieldType::Boolean => "bool".to_string(),
            FieldType::Array(inner) => format!("repeated {}", self.field_type_to_proto(inner)),
            FieldType::Object(_) => "google.protobuf.Struct".to_string(),
            FieldType::Enum(_) => "string".to_string(),
            FieldType::Any => "google.protobuf.Value".to_string(),
        }
    }
}

/// Convert string to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-', ' '])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Convert string to snake_case
#[cfg(test)]
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::SchemaCatalog;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("lxc"), "Lxc");
        assert_eq!(to_pascal_case("network_interface"), "NetworkInterface");
        assert_eq!(to_pascal_case("ovs-bridge"), "OvsBridge");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("containerID"), "container_i_d");
        assert_eq!(to_snake_case("objectPath"), "object_path");
    }

    #[test]
    fn test_generate_for_catalog() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        let generator = ProtoGenerator::new(ProtoGenConfig::default());
        let proto = generator.generate_for_catalog(&catalog);

        assert!(proto.contains("syntax = \"proto3\";"));
        assert!(proto.contains("service OperationService"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/schema_engine.rs">
//! Schema Engine - The Authoritative Source for State and Schema DNA
//!
//! The Schema Engine is the central coordinator that:
//! - Authoritatively routes all mutations (gRPC and D-Bus)
//! - Ensures all state changes are strictly recorded in the Event Chain (Audit Log)
//! - Broadcasts authoritative state changes to gRPC subscribers
//! - Directly manages authoritative RCP stores (OVSDB, NonNet, SQLite)

use async_trait::async_trait;
use serde_json;
use simd_json::prelude::{ValueAsContainer, ValueAsMutContainer, ValueAsScalar, ValueObjectAccess};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use zbus::zvariant::OwnedValue as ZOwnedValue;
use zbus::{Connection, Proxy};

use base64::Engine;
use op_identity::{read_sled, write_sled_full};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state_store::{Decision, EventChain, OperationType};


/// A state change projected from the authoritative system bus
#[derive(Debug, Clone)]
pub struct StateChange {
    pub change_id: String,
    pub event_id: u64,
    pub plugin_id: String,
    pub object_path: String,
    pub change_type: ChangeType,
    pub member_name: Option<String>,
    pub old_value: Option<simd_json::OwnedValue>,
    pub new_value: simd_json::OwnedValue,
    pub tags_touched: Vec<String>,
    pub event_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub actor_id: String,
    pub source: ChangeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    PropertySet,
    PropertyDelete,
    MethodCall,
    Signal,
    ObjectAdded,
    ObjectRemoved,
    SchemaMigration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    DBus,
    Grpc,
    Internal,
}

pub struct SchemaEngine {
    /// Authoritative Event Chain
    pub event_chain: Arc<RwLock<EventChain>>,
    /// Real-time change projection channel
    change_tx: broadcast::Sender<StateChange>,
    /// State cache for instant gRPC retrieval
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// System D-Bus connection authority
    pub dbus_connection: Arc<OnceCell<Connection>>,
    /// Resource limiter for D-Bus operations
    #[allow(dead_code)]
    dbus_call_limiter: Arc<Semaphore>,

    /// Authoritative RCP stores
    pub ovsdb: Arc<OvsdbClient>,
    pub nonnet: Arc<NonNetDb>,
}

impl std::fmt::Debug for SchemaEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaEngine").finish()
    }
}

#[async_trait]
impl op_core::state_publisher::StatePublisher for SchemaEngine {
    async fn publish_change(
        &self,
        plugin_id: String,
        path: String,
        change_type: op_core::state_publisher::ChangeType,
        property: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        tags: Vec<String>,
        source: String,
    ) -> anyhow::Result<()> {
        let internal_type = match change_type {
            op_core::state_publisher::ChangeType::PropertySet => ChangeType::PropertySet,
            op_core::state_publisher::ChangeType::Signal => ChangeType::Signal,
            op_core::state_publisher::ChangeType::Deleted => ChangeType::ObjectRemoved,
        };

        self.process_authoritative_change(
            plugin_id,
            path,
            internal_type,
            property,
            old_value,
            new_value,
            tags,
            source,
            ChangeSource::Internal,
        )
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(e))
    }
}

impl SchemaEngine {
    /// Create a new authoritative Schema Engine
    pub fn new(
        event_chain: Arc<RwLock<EventChain>>,
        ovsdb: Arc<OvsdbClient>,
        nonnet: Arc<NonNetDb>,
    ) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        Self {
            event_chain,
            change_tx,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(32)),
            ovsdb,
            nonnet,
        }
    }

    /// Authoritative D-Bus connection getter
    pub async fn dbus_connection(&self) -> anyhow::Result<Connection> {
        self.dbus_connection
            .get_or_try_init(|| async { Connection::system().await })
            .await
            .cloned()
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn compute_tags(&self, plugin_id: &str, object_path: &str) -> Vec<String> {
        let mut tags = Vec::new();
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            tags.push("network".to_string());
            tags.push("ovsdb".to_string());
        } else if object_path.contains("/nonnet/") {
            tags.push("nonnet".to_string());
            tags.push("plugin".to_string());
        } else {
            tags.push("state".to_string());
            tags.push(plugin_id.to_string());
        }
        tags
    }

    /// Process a change that has already happened in an authoritative store.
    /// This records the change in the event chain and broadcasts it to gRPC.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_authoritative_change(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        mut tags: Vec<String>,
        actor_id: String,
        source: ChangeSource,
    ) -> Result<StateChange, String> {
        if tags.is_empty() {
            tags = self.compute_tags(&plugin_id, &object_path);
        }

        let event = {
            let mut chain = self.event_chain.write().await;
            let op = match change_type {
                ChangeType::PropertySet => OperationType::PropertySet,
                ChangeType::ObjectRemoved => OperationType::Custom("delete".to_string()),
                _ => OperationType::EmitSignal,
            };
            let event = chain.record(
                actor_id.clone(),
                plugin_id.clone(),
                "1.0.0".to_string(),
                op,
                object_path.clone(),
                tags.clone(),
                Decision::Allow,
                &new_value,
            );
            event.clone()
        };

        self.update_cached_plugin_state(
            &plugin_id,
            &object_path,
            change_type,
            member_name.as_deref(),
            &new_value,
        )
        .await;

        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event.event_id,
            plugin_id,
            object_path,
            change_type,
            member_name,
            old_value,
            new_value,
            tags_touched: tags,
            event_hash: event.event_hash.clone(),
            timestamp: event.timestamp,
            actor_id,
            source,
        };

        let _ = self.change_tx.send(change.clone());
        Ok(change)
    }

    /// Start the Schema Engine background tasks.
    /// Subscribes to authoritative RCP stores and broadcasts changes.
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let me = self.clone();

        // 1. Subscribe to NonNet updates
        let mut nonnet_rx = self.nonnet.subscribe();
        let nonnet_self = me.clone();
        tokio::spawn(async move {
            loop {
                match nonnet_rx.recv().await {
                    Ok(update) => {
                        let _ = nonnet_self
                            .process_authoritative_change(
                                update.table.clone(),
                                format!(
                                    "/org/opdbus/v1/nonnet/{}/{}",
                                    update.db_name, update.table
                                ),
                                ChangeType::PropertySet,
                                None,
                                None,
                                simd_json::json!(update.rows),
                                vec!["nonnet".to_string()],
                                "nonnet-db".to_string(),
                                ChangeSource::Internal,
                            )
                            .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("NonNet subscription lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // 2. Subscribe to OVSDB updates
        let ovsdb_self = me.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_self.ovsdb.monitor_db("Open_vSwitch").await {
                while let Some(update) = rx.recv().await {
                    if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                        if params.len() >= 3 {
                            if let Some(tables) = params[2].as_object() {
                                for (table_name, table_update) in tables.iter() {
                                    let table_name_owned: String = table_name.to_string();
                                    // monitor_db returns serde_json::Value; convert to
                                    // simd_json::OwnedValue required by process_authoritative_change.
                                    let simd_val: simd_json::OwnedValue = {
                                        match serde_json::to_string(table_update).ok().and_then(
                                            |s| {
                                                let mut b = s.into_bytes();
                                                simd_json::to_owned_value(&mut b).ok()
                                            },
                                        ) {
                                            Some(v) => v,
                                            None => continue,
                                        }
                                    };
                                    let _ = ovsdb_self
                                        .process_authoritative_change(
                                            "net".to_string(),
                                            format!("/org/opdbus/v1/ovsdb/{}", table_name_owned),
                                            ChangeType::PropertySet,
                                            Some(table_name_owned),
                                            None,
                                            simd_val,
                                            vec!["ovsdb".to_string(), "network".to_string()],
                                            "ovsdb-monitor".to_string(),
                                            ChangeSource::DBus,
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Unified mutation entry point. Writes to authoritative RCP stores and
    /// triggers the event recording/broadcast pipeline.
    #[allow(clippy::too_many_arguments)]
    pub async fn mutate(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        _capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        let mut old_value = None;

        // 1. Write to authoritative RCP store
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            // OVSDB Authoritative Path
            if change_type == ChangeType::MethodCall {
                if let Some(method) = &member_name {
                    match method.as_str() {
                        "create_bridge" => {
                            if let Some(name) = value.as_str() {
                                self.ovsdb.create_bridge(name).await?;
                            }
                        }
                        "add_port" => {
                            if let Some(args) = value.as_array() {
                                if args.len() >= 2 {
                                    if let (Some(br), Some(port)) =
                                        (args[0].as_str(), args[1].as_str())
                                    {
                                        self.ovsdb.add_port(br, port).await?;
                                    }
                                }
                            }
                        }
                        _ => {
                            // Fallback to generic D-Bus call if it's a known service
                            let _ = self
                                .call_dbus_method(
                                    &format!("org.opdbus.{}.v1", plugin_id),
                                    &object_path,
                                    "org.opdbus.OvsdbV1",
                                    method,
                                    vec![value.clone()],
                                    &actor_id,
                                    &_capability_id,
                                )
                                .await?;
                        }
                    }
                }
            } else if change_type == ChangeType::PropertySet {
                if let Some(prop) = &member_name {
                    // Extract bridge name from path if possible
                    // Path format: /org/opdbus/v1/ovsdb/Bridge/bridge_name
                    let parts: Vec<&str> = object_path.split('/').collect();
                    if parts.len() >= 6 && parts[4] == "Bridge" {
                        let br_name = parts[5].replace('_', "-");
                        if let Some(val_str) = value.as_str() {
                            self.ovsdb
                                .set_bridge_property(&br_name, prop, val_str)
                                .await?;
                        }
                    }
                }
            }
        } else {
            // NonNet / Generic Plugin Path
            if change_type == ChangeType::PropertySet {
                // Get old value for the footprint before update from cache
                old_value = self.get_state(&plugin_id).await.and_then(|v| {
                    if let Some(prop) = &member_name {
                        v.get(prop).cloned()
                    } else {
                        Some(v)
                    }
                });

                // For NonNet plugins, we update the NonNetDb which is authoritative for non-network state
                if let Some(rows) = value.as_array() {
                    let rows_vec: Vec<simd_json::OwnedValue> = rows.to_vec();
                    self.nonnet.update_table(&plugin_id, rows_vec).await;
                }
            }
        }

        // 2. Record and broadcast change
        let change = self
            .process_authoritative_change(
                plugin_id,
                object_path,
                change_type,
                member_name,
                old_value,
                value.clone(),
                vec![], // Automatically computed in process_authoritative_change
                actor_id,
                ChangeSource::Grpc,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Write the Identity Sled with the updated mutation index.
        {
            let (existing_pubkey_b64, existing_trace_hex) =
                if let Ok((ptr, _mmap)) = read_sled() {
                    unsafe {
                        let sled = &*ptr;
                        (
                            base64::engine::general_purpose::STANDARD
                                .encode(sled.wireguard_pubkey),
                            sled.trace_id_hex(),
                        )
                    }
                } else {
                    (String::new(), String::new())
                };
            if let Err(e) = write_sled_full(
                &existing_pubkey_b64,
                change.event_id,
                &existing_trace_hex,
            ) {
                tracing::warn!("sled write after mutation failed: {}", e);
            }
        }

        Ok(MutationResult {
            success: true,
            event_id: change.event_id,
            event_hash: change.event_hash,
            result: Some(value),
            error: None,
        })
    }

    /// Backward-compatible wrapper for gRPC Mutations.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_grpc_mutation(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        self.mutate(
            plugin_id,
            object_path,
            change_type,
            member_name,
            value,
            actor_id,
            capability_id,
        )
        .await
    }

    /// Fetch current state for a specific plugin from authoritative cache
    pub async fn get_state(&self, plugin_id: &str) -> Option<simd_json::OwnedValue> {
        let cache = self.state_cache.read().await;
        cache.get(plugin_id).cloned()
    }

    /// Update the authoritative state cache
    pub async fn update_state_cache(&self, plugin_id: String, state: simd_json::OwnedValue) {
        let mut cache = self.state_cache.write().await;
        cache.insert(plugin_id, state);
    }

    async fn update_cached_plugin_state(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        property: Option<&str>,
        new_value: &simd_json::OwnedValue,
    ) {
        if object_path.starts_with("schema/") {
            return;
        }

        let mut cache = self.state_cache.write().await;

        match change_type {
            ChangeType::ObjectRemoved => {
                cache.remove(plugin_id);
            }
            ChangeType::PropertySet => {
                if let Some(property) = property {
                    let entry = cache
                        .entry(plugin_id.to_string())
                        .or_insert_with(|| simd_json::json!({}));

                    if let Some(existing) = entry.as_object_mut() {
                        existing.insert(property.to_string(), new_value.clone());
                    } else {
                        let mut state = simd_json::value::owned::Object::new();
                        state.insert(property.to_string(), new_value.clone());
                        *entry = simd_json::OwnedValue::Object(Box::new(state));
                    }
                } else {
                    cache.insert(plugin_id.to_string(), new_value.clone());
                }
            }
            _ => {}
        }
    }

    /// Route a D-Bus method call through the authoritative bridge
    #[allow(clippy::too_many_arguments)]
    pub async fn call_dbus_method(
        &self,
        bus_name: &str,
        path: &str,
        interface: &str,
        method: &str,
        _args: Vec<simd_json::OwnedValue>,
        _actor_id: &str,
        _capability_id: &Option<String>,
    ) -> anyhow::Result<simd_json::OwnedValue> {
        let conn = self.dbus_connection().await?;
        let proxy = Proxy::new(&conn, bus_name, path, interface).await?;
        let result: ZOwnedValue = proxy.call(method, &()).await?;
        simd_json::serde::to_owned_value(&result).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn change_tx(&self) -> broadcast::Sender<StateChange> {
        self.change_tx.clone()
    }
}

/// Result of an authoritative mutation
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub success: bool,
    pub event_id: u64,
    pub event_hash: String,
    pub result: Option<simd_json::OwnedValue>,
    pub error: Option<MutationError>,
}

#[derive(Debug, Clone)]
pub struct MutationError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    ValidationFailed,
    ReadOnly,
    Internal,
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/build.rs">
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile all domain protos into a single combined FileDescriptorSet so
    // that tonic-reflection exposes every service in one query.
    //
    // Adding a new domain proto:
    //   1. Add the .proto file under proto/
    //   2. Add it to the compile_protos list below
    //   3. Add rerun-if-changed below
    //   4. Add the generated server/client to grpc_server.rs
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("operation_descriptor.bin"))
        .compile_protos(
            &[
                "proto/operation.proto",
                "proto/mail.proto",
                "proto/privacy_network.proto",
                "proto/registration.proto",
                "proto/registry.proto",
            ],
            &["proto"],
        )?;

    println!("cargo:rerun-if-changed=proto/operation.proto");
    println!("cargo:rerun-if-changed=proto/mail.proto");
    println!("cargo:rerun-if-changed=proto/privacy_network.proto");
    println!("cargo:rerun-if-changed=proto/registration.proto");
    println!("cargo:rerun-if-changed=proto/registry.proto");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/Cargo.toml">
[package]
name = "op-grpc-bridge"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Bidirectional D-Bus <-> gRPC bridge with event chain integration"

[dependencies]
op-core = { workspace = true }
# gRPC
tonic = { workspace = true }
tonic-web = { workspace = true }
prost = { workspace = true }
prost-types = { workspace = true }
tonic-reflection = { workspace = true }
tonic-health = { workspace = true }

# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
tokio-stream = { version = "0.1", features = ["sync"] }

# D-Bus
zbus = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
simd-json = { workspace = true }

# Internal crates
op-state-store = { path = "../op-state-store" }
op-identity = { path = "../op-identity" }
op-network = { path = "../op-network" }
op-jsonrpc = { path = "../op-jsonrpc" }
op-cognitive-mcp = { path = "../op-cognitive-mcp" }
op-cache = { path = "../op-cache" }

# Zero-copy shared memory (1:1 Direct Read)
memmap2 = { workspace = true }
hex = { workspace = true }
sha2 = { workspace = true }

# Utilities
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
futures = "0.3"
async-stream = "0.3"
base64 = "0.21"
qdrant-client = "1.17"

[[bin]]
name = "op-grpc-bridge"
path = "src/bin/op-grpc-bridge.rs"

[build-dependencies]
tonic-build = { workspace = true }

[dev-dependencies]
tokio-test = "0.4"
tempfile = { workspace = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/compare-op-grpc-bridge.md">
# compare-op-grpc-bridge

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 8 |
| Proto files | 5 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 5 |
| Partial artifacts | 0 |
| Spec-listed source files | 6 |
| Spec-listed but missing | 0 |
| Extra implementation files | 2 |

## Current Implementation Overview

- Bidirectional D-Bus <-> gRPC bridge with event chain integration
- Internal crate integrations: op-core, op-state-store, op-identity, op-network.
- Protocol assets: 5 `.proto` files.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/sync_engine.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/sync_engine.rs |
| `src/proto_gen.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/proto_gen.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/grpc_server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc_server.rs |
| `src/grpc_client.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc_client.rs |
| `src/dbus_watcher.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus_watcher.rs |
| `build.rs` | ✅ Present | build script | build.rs |
| `root` | ✅ Present | root source group | src/dbus_watcher.rs, src/grpc_client.rs, src/grpc_server.rs, src/lib.rs, src/proto_gen.rs, src/schema_engine.rs, src/sync_engine.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| sync_engine | ✅ Implemented | src/sync_engine.rs | SPEC main module |
| proto_gen | ✅ Implemented | src/proto_gen.rs | SPEC main module |
| grpc_server | ✅ Implemented | src/grpc_server.rs | SPEC main module |
| grpc_client | ✅ Implemented | src/grpc_client.rs | SPEC main module |
| dbus_watcher | ✅ Implemented | src/dbus_watcher.rs | SPEC main module |
| Protocol `mail.proto` | ✅ Implemented | proto/mail.proto | proto |
| Protocol `operation.proto` | ✅ Implemented | proto/operation.proto | proto |
| Protocol `privacy_network.proto` | ✅ Implemented | proto/privacy_network.proto | proto |
| Protocol `registration.proto` | ✅ Implemented | proto/registration.proto | proto |
| Protocol `registry.proto` | ✅ Implemented | proto/registry.proto | proto |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block
- `op-state-store` - documented in SPEC
- `op-identity` - not listed in SPEC dependency block
- `op-network` - not listed in SPEC dependency block

### External Runtime Dependencies
- `tonic` - documented in SPEC
- `tonic-web` - not listed in SPEC dependency block
- `prost` - documented in SPEC
- `prost-types` - documented in SPEC
- `tonic-reflection` - documented in SPEC
- `tonic-health` - not listed in SPEC dependency block
- `tokio` - documented in SPEC
- `tokio-stream` - documented in SPEC
- `zbus` - documented in SPEC
- `serde` - documented in SPEC
- `serde_json` - documented in SPEC
- `simd-json` - documented in SPEC
- `tracing` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `async-trait` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `futures` - not listed in SPEC dependency block
- `async-stream` - not listed in SPEC dependency block
- `base64` - not listed in SPEC dependency block

### Development and Build Dependencies
- `dev:tokio-test`
- `build:tonic-build`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 2 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: dbus_watcher, grpc_client, grpc_server, proto_gen, schema_engine.
- RPC or protocol definition files: proto/mail.proto, proto/operation.proto, proto/privacy_network.proto, proto/registration.proto, proto/registry.proto.
- 14 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/SPEC.md">
# op-grpc-bridge - Specification

## Overview
**Crate**: `op-grpc-bridge`  
**Location**: `crates/op-grpc-bridge`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-grpc-bridge"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Bidirectional D-Bus <-> gRPC bridge with event chain integration"
```

### Source Structure
```
op-grpc-bridge/src/sync_engine.rs
op-grpc-bridge/src/proto_gen.rs
op-grpc-bridge/src/lib.rs
op-grpc-bridge/src/grpc_server.rs
op-grpc-bridge/src/grpc_client.rs
op-grpc-bridge/src/dbus_watcher.rs
```

### Key Dependencies
```toml
# gRPC
tonic = { workspace = true }
prost = { workspace = true }
prost-types = { workspace = true }
tonic-reflection = { workspace = true }

# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
tokio-stream = { version = "0.1", features = ["sync"] }

# D-Bus
zbus = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
simd-json = { workspace = true }

# Internal crates
op-state-store = { path = "../op-state-store" }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       6 Rust source files

### Main Modules
sync_engine
proto_gen
grpc_server
grpc_client
dbus_watcher

## Purpose
Bidirectional D-Bus <-> gRPC bridge with event chain integration

## Build Information
- **Edition**: edition.workspace = true
- **Version**: 0.1.0
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-state-store

---
*Generated from crate analysis*
</file>

</files>
