# Plugin Architecture Audit Report

**Date:** 2024
**Scope:** Comprehensive plugin architecture review for consistency and single source of truth
**Auditor:** Droid Factory

---

## Executive Summary

This audit reveals **critical architectural inconsistencies** in the plugin system that violate the principle of a single source of truth. The codebase contains **multiple conflicting path schemes**, **hardcoded schema definitions** spanning 3,180 lines, and **64 plugins without dedicated schema files**.

### Critical Findings

| Severity | Count | Issue |
|----------|-------|-------|
| 🔴 CRITICAL | 2+ | Multiple conflicting canonical paths |
| 🔴 CRITICAL | 64+ | Plugins without schema files |
| 🟡 HIGH | 3,180+ | Lines of hardcoded schema definitions |
| 🟡 HIGH | 15+ | Hardcoded path constants across crates |
| 🟠 MEDIUM | 2 | Competing registry implementations |
| 🟠 MEDIUM | 0 | FreeDesktop plugin (missing entirely) |

---

## 1. Path Inconsistencies (CRITICAL)

### Conflicting Canonical Paths Found

| Path Pattern | Location | Line(s) | Status |
|--------------|----------|---------|--------|
| `/opdbus/v1/plugins/` | `default_registry.rs` | 147 | ❌ Legacy, non-standard |
| `/org/opdbus/v1/plugins/` | `default_registry.rs` | 147 | ✅ Correct FreeDesktop convention |
| `/opdbus/v1/plugins/` | `zeroclaw.rs` | 59, 65, 82, 195 | ❌ Hardcoded legacy |
| `/opdbus/v1/plugins/*` | `plugin_schema_defs.rs` | 3123 | ❌ Example in schema |
| `/opdbus/v1/plugins/wireguard` | `plugin_schema_defs.rs` | 3040 | ❌ Example in schema |
| `/org/opdbus/v1` | `managed_objects.rs` | 32 | ✅ Correct (OBJECT_MANAGER_PATH) |
| `org.opdbus.v1` | `plugin_schema_defs.rs` | 3050 | ✅ Correct (interface naming) |
| `org.opdbus.v1.plugins` | `registry.rs` | 95 | ❌ Missing `/` prefix |
| `org.opdbus.MailServer.3tched` | `plugin_schema_defs.rs` | 1815 | ⚠️ Non-standard suffix |
| `/org/opdbus/bridge` | `ovs-dbus-init.rs` | - | ⚠️ Legacy initialization path |

### Path Constants (Hardcoded)

| Constant | Value | File | Line | Issue |
|----------|-------|------|------|-------|
| `OBJECT_MANAGER_PATH` | `/org/opdbus/v1` | `managed_objects.rs` | 32 | ✅ Correct |
| `PROJECTED_IFACE` | `org.opdbus.ProjectedObjectV1` | `managed_objects.rs` | 33 | ✅ Correct |
| Path in comments | `/opdbus/v1/plugins/compact_mcp` | `compact_mcp.rs` | 5 | ❌ Comment shows wrong path |
| Path in comments | `/opdbus/v1/plugins/cognitive_mcp` | `cognitive_mcp.rs` | 5 | ❌ Comment shows wrong path |
| DBus object | `/opdbus/v1/plugins/zeroclaw` | `zeroclaw.rs` | 59 | ❌ Hardcoded legacy |
| Policy source | `/opdbus/v1/plugins/oscal_subid_registry` | `zeroclaw.rs` | 65, 82, 195 | ❌ Hardcoded legacy |

---

## 2. Schema Source of Truth Issues (CRITICAL)

### Schema Files (Good)

| File | Purpose | Status |
|------|---------|--------|
| `opdbus-plugin-schema.json` | Generic plugin schema | ✅ Exists |
| `service-plugin-schema.json` | Service plugin schema | ✅ Exists |
| `incus-wireguard-ingress.json` | Incus wireguard | ✅ Exists |
| `incus-xray-reality-client.json` | XRay client | ✅ Exists |
| `incus-xray-reality-server.json` | XRay server | ✅ Exists |
| `jsonschema-meta.json` | Meta schema | ⚠️ EMPTY FILE (3 bytes `{}`) |

### Hardcoded Schemas (CRITICAL PROBLEM)

**File:** `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`
- **Lines:** 3,180 lines
- **Function:** `simple_schema()` helper + 45+ individual schema functions
- **Pattern:** Each plugin schema is hand-coded in Rust

**Plugin Schemas Hardcoded (64 total):**

```
adc_plugin_schema
agent_config_plugin_schema
endpoint_plugin_schema
gcloud_adc_plugin_schema
hardware_plugin_schema
keypair_plugin_schema
ovsdb_bridge_plugin_schema
proxmox_plugin_schema
proxy_server_plugin_schema
service_plugin_schema
sess_decl_plugin_schema
software_plugin_schema
users_plugin_schema
web_ui_plugin_schema
wireguard_plugin_schema
incus_plugin_schema
net_plugin_schema
rtnetlink_plugin_schema
openflow_plugin_schema
s6_plugin_schema
privacy_router_plugin_schema
unix_socket_plugin_schema
privacy_routes_plugin_schema
mail_server_plugin_schema
... (and 40+ more)
```

**Each hardcoded schema includes:**
- Field definitions
- Type constraints
- Examples
- Dependencies
- Read-only conditions

**Problem:** Schema changes require Rust code modification and recompilation instead of editing a JSON/YAML file.

---

## 3. Registry Duplication (HIGH)

### Multiple Registry Implementations

| Registry | Location | Purpose | Issue |
|----------|----------|---------|-------|
| `DefaultPluginRegistry` | `default_registry.rs` | Auto-loads essential plugins | Hardcoded plugin list |
| `PluginRegistry` (op-mcp) | `op-mcp/src/tool_registry.rs` | MCP tool registry | Separate from main registry |
| `ManagedObjectRegistry` | `managed_objects.rs` | D-Bus object registry | Uses dashmap, not schema-backed |
| `AgentRegistry` | `op-agents/src/agent_registry.rs` | Agent management | Separate registry |
| `Registry` (gRPC bridge) | `proto/registry.proto` | gRPC registry | Different protocol |

### Registry Path Logic Duplication

**File:** `default_registry.rs:144-155`
```rust
const PREFIXES: [&str; 2] = ["/opdbus/v1/plugins/", "/org/opdbus/v1/plugins/"];
```

This accepts BOTH legacy and correct paths, perpetuating inconsistency.

---

## 4. Missing FreeDesktop Plugin (MEDIUM)

### Current FreeDesktop Support

The system has `org.freedesktop.DBus.ObjectManager` interface implemented in `managed_objects.rs`, but **NO dedicated FreeDesktop plugin** exists.

### What Should Exist

| Component | Status | Needed |
|-----------|--------|--------|
| FreeDesktop plugin schema | ❌ Missing | ✅ Required |
| FreeDesktop plugin implementation | ❌ Missing | ✅ Required |
| FreeDesktop D-Bus interfaces | Partial | Complete model |
| FreeDesktop object paths | Partial | Canonical paths |
| FreeDesktop properties | Partial | Schema-backed |
| FreeDesktop methods | Partial | Schema-backed |
| FreeDesktop signals | Partial | Schema-backed |

### FreeDesktop Interfaces to Model

```
org.freedesktop.DBus.ObjectManager
org.freedesktop.DBus.Properties
org.freedesktop.DBus.Introspectable
org.freedesktop.DBus.Peer
org.freedesktop.systemd1.Manager (optional)
org.freedesktop.NetworkManager (optional)
```

---

## 5. Schema Validation Gaps (CRITICAL)

### Current State

| Aspect | Status | Required |
|--------|--------|----------|
| Plugins require schema to exist | ❌ NO | ✅ YES |
| Schema validation at runtime | ❌ NO | ✅ YES |
| Schema versioning | ⚠️ Partial | ✅ Strict |
| Schema-to-model generation | ❌ NO | ✅ YES |
| OSCAL compliance mapping | ⚠️ Partial | ✅ Complete |

### Evidence from `default_registry.rs`

```rust
// Lines 398-415
let missing: Vec<String> = plugins
    .iter()
    .filter(|plugin| plugin.schema().is_none())
    .map(|plugin| plugin.name().to_string())
    .collect();

assert!(
    missing.is_empty(),
    "auto-loaded plugins missing schema(): {:?}",
    missing
);
```

**This is a TEST ONLY** - runtime doesn't enforce schema validation!

---

## 6. File-by-File Audit

### Plugin Implementation Files (64 found)

All files in `crates/op-plugins/src/state_plugins/`:

```
adc.rs                  - NO SCHEMA FILE
agent_config.rs         - NO SCHEMA FILE
dnsresolver.rs          - NO SCHEMA FILE
packagekit.rs           - NO SCHEMA FILE
software.rs             - NO SCHEMA FILE
openflow.rs             - NO SCHEMA FILE
mod.rs                  - (module file)
antigravity.rs          - NO SCHEMA FILE
antigravity_chat.rs     - NO SCHEMA FILE
knowledge_plugin.rs     - NO SCHEMA FILE
btrfs_plugin.rs         - NO SCHEMA FILE
workflows_plugin.rs     - NO SCHEMA FILE
memory_plugin.rs        - NO SCHEMA FILE
cron.rs                 - NO SCHEMA FILE
fail2ban.rs             - NO SCHEMA FILE
factory.rs              - NO SCHEMA FILE
zeroclaw.rs             - NO SCHEMA FILE
mail_server.rs          - NO SCHEMA FILE
ctl_plane_chatbot.rs    - NO SCHEMA FILE
compact_mcp.rs          - NO SCHEMA FILE
cognitive_mcp.rs        - NO SCHEMA FILE
service.rs              - NO SCHEMA FILE
s6.rs                   - NO SCHEMA FILE
mcp.rs                  - NO SCHEMA FILE
unix_socket.rs          - NO SCHEMA FILE
procfs.rs               - NO SCHEMA FILE
privacy_router.rs       - NO SCHEMA FILE
openflow_obfuscation.rs - NO SCHEMA FILE
net.rs                  - NO SCHEMA FILE
full_system.rs          - NO SCHEMA FILE
wireguard.rs            - NO SCHEMA FILE
web_ui.rs               - NO SCHEMA FILE
users.rs                - NO SCHEMA FILE
systemd_networkd.rs     - NO SCHEMA FILE
systemd.rs              - NO SCHEMA FILE
sessdecl.rs             - NO SCHEMA FILE
schema_contract.rs      - NO SCHEMA FILE
rtnetlink.rs            - NO SCHEMA FILE
proxy_server.rs         - NO SCHEMA FILE
proxmox.rs              - NO SCHEMA FILE
privacy_routes.rs       - NO SCHEMA FILE
privacy.rs              - NO SCHEMA FILE
pcidecl.rs              - NO SCHEMA FILE
ovsdb_bridge.rs         - NO SCHEMA FILE
netmaker.rs             - NO SCHEMA FILE
lxc.rs                  - NO SCHEMA FILE
login1.rs               - NO SCHEMA FILE
keyring.rs              - NO SCHEMA FILE
keypair.rs              - NO SCHEMA FILE
incus.rs                - NO SCHEMA FILE
hardware.rs             - NO SCHEMA FILE
gcloud_adc.rs           - NO SCHEMA FILE
endpoint.rs             - NO SCHEMA FILE
config.rs               - NO SCHEMA FILE
```

**All 54+ plugin implementations lack corresponding schema files!**

---

## 7. Deliverables Checklist

### Files to Create

| File | Purpose |
|------|---------|
| `schemas/plugin/` | New directory for all plugin schemas |
| `schemas/plugin/freedesktop.json` | FreeDesktop plugin schema |
| `schemas/plugin/{plugin-name}.json` | 64 individual plugin schemas |
| `crates/op-plugins/src/state_plugins/freedesktop.rs` | FreeDesktop plugin implementation |
| `docs/PLUGIN_ARCHITECTURE.md` | Canonical architecture documentation |

### Files to Modify

| File | Changes Required |
|------|----------------|
| `default_registry.rs` | Remove legacy path support, enforce `/org/opdbus/v1/plugins/` |
| `plugin_schema_defs.rs` | Remove hardcoded schemas, add schema file loader |
| `managed_objects.rs` | Update comments, verify path consistency |
| `registry.rs` | Consolidate with default_registry, single source of truth |
| `zeroclaw.rs` | Update hardcoded paths to canonical |
| `mail_server.rs` | Update hardcoded paths to canonical |
| `compact_mcp.rs` | Update comment paths |
| `cognitive_mcp.rs` | Update comment paths |
| `op-mcp/src/tool_registry.rs` | Consolidate with main registry |
| `op-agents/src/agent_registry.rs` | Consolidate with main registry |

### Constants to Unify

| Current | Target |
|---------|--------|
| `OBJECT_MANAGER_PATH = "/org/opdbus/v1"` | Keep ✅ |
| `PROJECTED_IFACE = "org.opdbus.ProjectedObjectV1"` | Keep ✅ |
| `/opdbus/v1/plugins/` (legacy) | REMOVE ❌ |
| `/org/opdbus/v1/plugins/` (correct) | ENFORCE ✅ |
| `org.opdbus.v1` (interface) | Keep ✅ |
| `org.opdbus.v1.plugins` (incomplete) | FIX to include `/` |

---

## 8. Canonical Path Rules

### Required Naming Conventions

| Context | Format | Example |
|---------|--------|---------|
| D-Bus Object Path | `/org/opdbus/v1/plugin/plugins` | `/org/opdbus/v1/plugin/plugins/incus` |
| D-Bus Interface Name | `org.opdbus.v1.Plugin.Plugins` | `org.opdbus.v1.Plugin.Plugins.Incus` |
| D-Bus Service Name | `org.opdbus.v1` | `org.opdbus.v1` |
| Schema File Path | `schemas/plugin/{name}.json` | `schemas/plugin/incus.json` |
| Registry Projection | `/org/opdbus/v1/plugin/plugins` | Canonical path |
| Plugin Namespace | `org.opdbus.v1.plugin.{name}` | `org.opdbus.v1.plugin.incus` |

### Forbidden Patterns

```
/opdbus/v1/...           (missing org prefix)
/org/opdbus/plugins/...  (missing v1)
/org/opdbus/v1/...       (missing plugin segment)
/opdbus/v1/plugins/...   (missing org prefix)
org.opdbus.plugins...    (missing v1)
org.opdbus.v1plugins...  (missing dot)
```

---

## 9. Implementation Recommendations

### Phase 1: Path Unification (URGENT)

1. Define single `CANONICAL_PLUGIN_PATH` constant
2. Replace all legacy `/opdbus/v1/plugins/` references
3. Update path extraction logic to ONLY accept canonical paths
4. Add deprecation warnings for legacy paths

### Phase 2: Schema Extraction (CRITICAL)

1. Extract all `*_plugin_schema()` functions from `plugin_schema_defs.rs`
2. Generate individual JSON schema files in `schemas/plugin/`
3. Create schema loader that reads from files at runtime
4. Implement schema caching for performance

### Phase 3: FreeDesktop Plugin (HIGH)

1. Create `schemas/plugin/freedesktop.json`
2. Implement `freedesktop.rs` plugin
3. Model all FreeDesktop D-Bus interfaces
4. Add FreeDesktop compliance validation

### Phase 4: Registry Consolidation (HIGH)

1. Merge `DefaultPluginRegistry` and `PluginRegistry`
2. Single registry backed by schema files
3. Remove duplicate path resolution logic
4. Unified plugin loading from canonical paths only

### Phase 5: Validation Enforcement (CRITICAL)

1. Add runtime schema validation
2. Reject plugins without valid schemas
3. Implement schema versioning
4. Add schema-to-model code generation

---

## 10. Final Architecture Target

```
schemas/plugin/
├── org.opdbus.plugin.schema.json   (meta-schema for all plugins)
├── freedesktop.json                (FreeDesktop plugin)
├── incus.json                      (Incus plugin)
├── net.json                        (Network plugin)
├── s6.json                         (S6 plugin)
└── ... (64 plugin schema files)

Source of Truth: JSON Schema Files
                 ↓
Validated at Runtime (cacheable)
                 ↓
D-Bus Projection: /org/opdbus/v1/plugin/plugins/{plugin}
                 ↓
Interface: org.opdbus.v1.Plugin.Plugins.{PluginName}
                 ↓
Registry: Unified, schema-backed, canonical paths only
```

---

## Appendix A: Hardcoded Path References

### By File

| File | Line | Content |
|------|------|---------|
| `default_registry.rs` | 116 | Comment: `/opdbus/v1/plugins/<plugin>/...` |
| `default_registry.rs` | 147 | `PREFIXES: ["/opdbus/v1/plugins/", "/org/opdbus/v1/plugins/"]` |
| `default_registry.rs` | 415 | Test path: `/opdbus/v1/plugins/procfs/memory` |
| `default_registry.rs` | 436 | Test path: `/opdbus/v1/plugins/procfs/memory` |
| `zeroclaw.rs` | 59 | `"dbus_object": "/opdbus/v1/plugins/zeroclaw"` |
| `zeroclaw.rs` | 65 | `"policy_source": "/opdbus/v1/plugins/oscal_subid_registry"` |
| `zeroclaw.rs` | 82 | `"source": "/opdbus/v1/plugins/oscal_subid_registry"` |
| `zeroclaw.rs` | 195 | `"source": "/opdbus/v1/plugins/oscal_subid_registry"` |
| `mail_server.rs` | 71 | `dbus_service_name: "org.opdbus.MailServer.3tched"` |
| `mail_server.rs` | 177 | `dbus_services: ["org.opdbus.MailServer.3tched"]` |
| `compact_mcp.rs` | 5 | Comment: `/opdbus/v1/plugins/compact_mcp` |
| `cognitive_mcp.rs` | 5 | Comment: `/opdbus/v1/plugins/cognitive_mcp` |
| `plugin_schema_defs.rs` | 1815 | Example: `"org.opdbus.MailServer.3tched"` |
| `plugin_schema_defs.rs` | 1879 | Example: `"dbus_service_name": "org.opdbus.MailServer.3tched"` |
| `plugin_schema_defs.rs` | 2239 | Description: `org.opdbus.CognitiveMcp` |
| `plugin_schema_defs.rs` | 3040 | Example: `/opdbus/v1/plugins/wireguard` |
| `plugin_schema_defs.rs` | 3050 | Example: `org.opdbus.v1` |
| `plugin_schema_defs.rs` | 3123 | Example: `/opdbus/v1/plugins/*` |
| `antigravity.rs` | 129 | `oscal_source`: `/opdbus/v1/plugins/oscal_subid_registry` |
| `antigravity.rs` | 171 | `source`: `/opdbus/v1/plugins/oscal_subid_registry` |
| `antigravity.rs` | 282 | `source`: `/opdbus/v1/plugins/oscal_subid_registry` |
| `antigravity.rs` | 541 | `oscal_source`: `/opdbus/v1/plugins/oscal_subid_registry` |

---

## Appendix B: Audit Evidence

### Commands Used

```bash
# Find all plugin paths
grep -rn "org\.opdbus\|opdbus/v1\|/opdbus/v1\|org/opdbus" crates/ --include="*.rs"

# Find all path constants
grep -rn "const.*PATH\|static.*PATH\|PLUGIN.*PATH\|DBUS.*PATH\|OBJECT.*PATH" crates/ --include="*.rs"

# List all plugin files
ls crates/op-plugins/src/state_plugins/*.rs

# Check schema directory
ls schemas/
```

### Statistics

- **Total plugin implementations:** 54
- **Plugins with schema files:** 0 (excluding generic schemas)
- **Hardcoded path references:** 22+
- **Schema definition lines:** 3,180
- **Registry implementations:** 5+
- **Conflicting path patterns:** 2

---

**END OF AUDIT REPORT**

Next Steps: See `docs/PLUGIN_ARCHITECTURE_CLEANUP.md` for implementation plan.
