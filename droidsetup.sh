I’ll add gRPC/Rust/Tonic as a dedicated communication layer skill group, not just a code-review category. The main point is enforcing schemas, reflection, routing, and auditability across service boundaries.



Thought for 5s



Add this as a dedicated gRPC / Rust Tonic skill layer.



Tonic is the right Rust-native gRPC stack for this. Current docs describe tonic as a Rust gRPC-over-HTTP/2 implementation focused on async/await, performance, interoperability, and production use. tonic-reflection provides gRPC Server Reflection, and tonic-build / related build tooling generate Rust service stubs from protobuf definitions.



gRPC / Tonic skills

Skill	Purpose

proto-schema-author	Write .proto contracts for DBus objects, schema events, model-routing events, compliance findings, and audit records.

tonic-service-generator	Generate Rust server/client code from protobuf using Tonic build tooling.

grpc-reflection-indexer	Use gRPC reflection to discover live services, methods, request types, and response types.

grpc-schema-validator	Validate gRPC requests/responses against canonical schema rules before accepting them.

grpc-dbus-bridge	Expose selected DBus methods/properties/signals as typed gRPC services.

grpc-event-stream	Stream DBus changes, schema mutations, audit events, and compliance evidence over server-streaming or bidirectional gRPC.

grpc-policy-interceptor	Use Tonic interceptors/middleware-style checks for auth, schema tags, routing policy, and control enforcement.

grpc-audit-interceptor	Log every request, method, caller, schema hash, result hash, and control mapping.

grpc-health-check	Standard health service for agents, schema services, DBus bridge, MCP gateway, and model router.

grpc-error-normalizer	Convert internal errors into stable typed gRPC status/error structures.

grpc-contract-test	Test that generated Rust services match .proto, JSON Schema, and OSCAL mappings.

grpc-tonic-review	Audit Rust/Tonic code for bad async boundaries, unwraps, missing timeouts, oversized messages, and missing auth.

Where it fits
￼

￼
￼
Opus 4.8 High
￼
￼


DBus

  ↓

DBus introspection

  ↓

Canonical schema

  ↓

Rust typed models

  ↓

gRPC / Tonic service boundary

  ↓

MCP / model routing / UI / compliance engine

Recommended service layout

proto/

  op_dbus/

    introspection.proto

    state.proto

    mutation.proto

    events.proto



  op_schema/

    registry.proto

    validation.proto

    render.proto



  op_compliance/

    oscal.proto

    evidence.proto

    findings.proto



  op_model/

    routing.proto

    reasoning.proto

    tool_call.proto



crates/

  op-proto/

  op-grpc/

  op-dbus-bridge/

  op-schema-service/

  op-compliance-service/

  op-model-router/

Specific skills to add to your list

skills/grpc/

  proto-schema-author/

  tonic-service-generator/

  grpc-reflection-indexer/

  grpc-schema-validator/

  grpc-dbus-bridge/

  grpc-event-stream/

  grpc-policy-interceptor/

  grpc-audit-interceptor/

  grpc-health-check/

  grpc-contract-test/

  grpc-tonic-review/

Key design rule



Do not let gRPC become “just transport.”



For your system, every gRPC method should carry:



request_id

actor_id

schema_id

schema_version

schema_hash

control_tags

risk_level

mutation_intent

audit_context



That makes gRPC part of the compliance/control plane instead of just RPC plumbing.



Example service categories

Service	Role

DbusIntrospectionService	Lists objects, interfaces, methods, properties, and signals.

DbusStateService	Reads canonical live state.

DbusMutationService	Applies controlled DBus changes.

SchemaRegistryService	Stores schema versions and hashes.

SchemaValidationService	Validates payloads and runtime state.

OscalMappingService	Maps assets/events to controls.

EvidenceService	Records proof for compliance.

ModelRoutingService	Chooses model/tool path based on schema and risk.

McpGatewayService	Controlled bridge into MCP tools.

AuditLedgerService	Writes immutable event records.

Most important Tonic review checks

No unauthenticated mutation methods.

No raw JSON blobs unless wrapped with schema_id + schema_hash.

No unwrap/expect in service handlers.

No unlimited request body size.

No missing timeout/deadline handling.

No service without reflection metadata.

No mutation without audit event.

No DBus call without policy check.

No generated proto drift from committed schema.

No transport-only methods that bypass control tags.



For your operation, the clean architecture is:



DBus is the live system interface.

Schema is the law.

gRPC/Tonic is the typed service boundary.

OSCAL is the compliance mapping.

MCP is the external tool gateway.

Audit ledger is the proof layer.

Model routing is the brainstem.
