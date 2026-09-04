# Mission: Plugin Schema Uniformization

## Objective

Migrate ALL plugins in `crates/op-plugins/src/state_plugins/` to a uniform GB.<PluginName> pattern using a 7-agent pipeline per plugin. Each new plugin is created from scratch by reading the old plugin and transferring ONLY the good parts. Common schema elements are extracted into reusable op-blob schema blobs to keep large schemas manageable.

## Ground Rules

1. **Create NEW plugin files** named `gb_<plugin_name>.rs`. Read the OLD plugin entirely. Cut and paste only the good parts into the new file. Leave behind the bad.
2. **For hand-rolled plugins**, research official sources (e.g., networkmanager.dev for OVS, WireGuard docs, systemd docs) to get structured data for typed structs. If something is missing, fix it. If not sure, ask the user.
3. **Use blobs for common elements.** If multiple plugins share common fields (status, running, oscal_source, network interface, service config, etc.), create a reusable schema blob in op-blob first and reference it when building plugin schemas. This keeps large schemas manageable.
4. **One agent per quality dimension.** Each agent has a specific job. Agents run sequentially per plugin.
5. **Do NOT modify the old plugin file** until Agent 7 retires it (rename to `<plugin_name>.rs.bak` or delete after the new one is registered).

## Plugin Priority Order

### Phase 1: Failing-test plugins (block CI)
1. fail2ban (invalid subid category "security")
2. persona (invalid subid category "agent")
3. snowball (mut.* missing actor_id/capability_id)
4. json_render (src.* missing source_system/source_locator)
5. openflow_obfuscation (mut.* missing actor_id/capability_id)
6. software (stale golden: packages.type any -> array<object>)

### Phase 2: Tier 4 hand-rolled plugins (no schemars, no drift guard)
7. wireguard (research WireGuard docs for typed structs)
8. ovsdb_bridge (research networkmanager.dev / OVS schema for typed structs)
9. openflow (research OpenFlow spec for typed structs)
10. users (research /etc/passwd, /etc/group, login1 D-Bus for typed structs)
11. s6_systemctl (research s6 docs for typed structs)
12. login1 (research systemd login1 D-Bus interface for typed structs)
13. cozo (research CozoDB docs for typed structs)
14. dnsresolver (research systemd-resolved D-Bus for typed structs)
15. keyring (research GNOME Keyring DBus for typed structs)
16. packagekit (research PackageKit D-Bus for typed structs)
17. pcidecl (research lspci/sysfs for typed structs)
18. service (research systemd service D-Bus for typed structs)
19. s6 (research s6 init system for typed structs)
20. systemd (research systemd D-Bus for typed structs)
21. systemd_networkd (research systemd-networkd D-Bus for typed structs)
22. proxy_server (research proxy config schema for typed structs)
23. shared_unix_socket (research Unix socket schema for typed structs)
24. datastore (research data store schema for typed structs)
25. mail_server (research mail server config for typed structs)
26. incus (research Incus API for typed structs)
27. incus_device (research Incus device API for typed structs)
28. rovs_commands (research ovs-vsctl / rovs for typed structs)
29. privacy (coordinating plugin, research privacy config schema)
30. zeroclaw (already partially schemars, needs full migration)

### Phase 3: Tier 3 plugins (schemars but no drift guard, no subid validation)
31. btrfs_plugin
32. factory
33. full_system
34. qdrant
35. embedding_model
36. large_language_model
37. gemma_brain
38. memory_plugin
39. privacy_router
40. freedesktop

### Phase 4: Tier 2 plugins (schemars + subid validation but no drift guard)
41. config
42. endpoint
43. keypair
44. agent_config
45. antigravity
46. antigravity_chat

## Pre-Work: Common Schema Blobs

Before starting the plugin pipeline, identify and create reusable schema blobs in op-blob for common elements that appear across 3+ plugins:

### Likely common blobs:
- **PluginMetadata blob**: `status`, `running`, `healthy`, `version`, `software`, `dependencies`, `oscal_source`, `tools` (appears in xray, cognitive_mcp, compact_mcp, zeroclaw, etc.)
- **NetworkInterface blob**: `interface`, `address`, `mtu`, `bridge_name`, `port` (appears in net, wireguard, ovsdb_bridge, openflow, privacy_router)
- **ServiceConfig blob**: `enabled`, `socket_port`, `config_path`, `pid_file` (appears in xray, s6, systemd, cron, fail2ban)
- **AIModelConfig blob**: `model_id`, `model_name`, `auth_method`, `api_key`, `available` (appears in cognitive_mcp, compact_mcp, antigravity_chat, gemma_brain, embedding_model, large_language_model)
- **SecurityKey blob**: `key`, `certificate`, `algorithm`, `public_key`, `private_key_path` (appears in wireguard, keypair, keyring, wgcf)

Create these as composable schema sections in op-blob that can be referenced when building a plugin's PluginSchema.

## The 7-Agent Pipeline (per plugin)

For each plugin, run these 7 agents sequentially:

### Agent 1: Schema Location & Validation
**Input:** Old plugin file path
**Job:**
1. Read the ENTIRE old plugin file
2. Read the template (plugin_scaffold.rs.template) for target structure
3. Read xray.rs as gold-standard reference
4. Create NEW file `gb_<plugin_name>.rs`
5. Transfer the plugin identity (name, version, category, description)
6. Transfer the state struct fields as typed Rust structs with `#[derive(schemars::JsonSchema)]`
7. Fix any bad subid categories (use only the 7 allowed: src, prj, sch, mut, obs, evt, exp)
8. For hand-rolled plugins: research official sources, create proper typed structs
9. Reference common blobs where applicable instead of redefining common fields
10. Write the PLUGIN ENTRY and PLUGIN BODY sections
**Output:** New file created with typed state structs and StatePlugin impl

### Agent 2: Drift Guard
**Input:** Old plugin file, new plugin file
**Job:**
1. Read the old plugin's schema function (the hand-rolled `PluginSchema` or `schema_golden()`)
2. Create a `#[cfg(test)] fn <plugin>_schema_golden()` in the new file that reproduces the old hand-rolled schema as the golden reference
3. Add `derived_schema_matches_hand_rolled` test using `schema_diffs`
4. If the old plugin had no golden, create one from the old plugin's current schema output
5. Fix any schema drift found (update golden to match correct derived schema)
**Output:** Drift guard test added to new file

### Agent 3: OSCAL Subid Taxonomy Compliance
**Input:** Old plugin file, new plugin file
**Job:**
1. Check all subids in the old plugin against the 7-category taxonomy
2. Verify all subids in the new file use valid categories (src, prj, sch, mut, obs, evt, exp)
3. Verify all subids use valid component-types (software, service, network, hardware, process-procedure, standard, validation, policy, plan, guidance, physical, this-system, system, interconnection)
4. For `mut.*` subids: ensure `actor_id` and `capability_id` fields exist in the schema
5. For `src.*` subids: ensure `source_system` and `source_locator` fields exist in the schema
6. For `evt.*` subids: ensure `event_id` or `event_hash` fields exist
7. Add `all_subids_are_valid` test
8. Add any missing required fields to the typed structs
**Output:** All subids compliant, subid validation test added

### Agent 4: Schemars Adapter & Typed Method Declarations
**Input:** Old plugin file, new plugin file
**Job:**
1. Verify the new file uses `plugin_schema_from_json` (not hand-rolled `PluginSchema { ... }`)
2. Verify `apply_state_defaults` is called
3. Check old plugin for any methods (D-Bus method contracts)
4. For each method in the old plugin, create typed input/output structs with `#[derive(schemars::JsonSchema)]`
5. Add method declarations using `method_decl_from_schemars_with_output::<InputType, OutputType>`
6. If the old plugin had no methods, add at minimum a `get_status` (Read) and `set_status` (Mutation) method
7. Verify all methods are inserted into `schema.methods`
**Output:** All methods typed and declared via schemars adapter

### Agent 5: SideEffect & Idempotency Annotations
**Input:** Old plugin file, new plugin file
**Job:**
1. Check old plugin for SideEffect usage (Read, Mutation)
2. For each method in the new file, verify SideEffect is declared
3. For each method, verify idempotency is set (true for read-only/query methods, false for mutations, true for delete operations)
4. For each method, verify capability string is set
5. Cross-check: Read methods should not modify state; Mutation methods should
6. Add any missing SideEffect annotations
**Output:** All methods have correct SideEffect and idempotency

### Agent 6: x-oscal-subid Annotations
**Input:** Old plugin file, new plugin file
**Job:**
1. Check every field in the old plugin's schema for subid annotations
2. Verify every field in the new file's state struct has `#[schemars(extend("x-oscal-subid" = "..."))]`
3. Verify every method input/output struct has x-oscal-subid on the struct and each field
4. Verify the root state struct has a schema-level x-oscal-subid
5. Copy any valid subids from the old plugin that were missed
6. Generate new subids for any fields that were added (following the taxonomy)
7. Verify subid format: `<category>.<component-type>.<subject>.<verb>[.<facet>][@v1]`
**Output:** Every struct and field has valid x-oscal-subid annotations

### Agent 7: Validation, Registration & Retirement
**Input:** New plugin file
**Job:**
1. Add `pub mod gb_<plugin_name>;` to mod.rs
2. Add `pub use gb_<plugin_name>::Gb<PluginName>Plugin;` to mod.rs re-exports
3. Verify `inventory::submit!` is in the new file for self-registration
4. Run `cargo check -p op-plugins`
5. Run `cargo test -p op-plugins --lib -- <plugin_name>` (run the new plugin's tests)
6. Run `cargo clippy -p op-plugins -- -D warnings` on the new file
7. If all pass: rename old file to `<plugin_name>.rs.bak` (or add `#[cfg(disabled)]` to prevent compilation)
8. Remove old `pub mod <plugin_name>;` from mod.rs if it conflicts
9. Remove old `pub use` re-export from mod.rs if it conflicts
10. Run full `cargo test -p op-plugins --lib` to verify no regressions
**Output:** New plugin registered, old plugin retired, all tests pass

## Acceptance Gates

After ALL plugins are migrated:
- [ ] `cargo check -p op-plugins` passes
- [ ] `cargo test -p op-plugins --lib` passes with 0 failures
- [ ] `cargo clippy -p op-plugins -- -D warnings` passes
- [ ] `cargo test -p op-plugins --lib -- all_plugin_subids_are_valid_and_unique` passes
- [ ] Every plugin has: schemars-derived schema, drift guard test, subid validation test, typed methods with SideEffect, x-oscal-subid annotations
- [ ] Common schema blobs created in op-blob for shared elements
- [ ] All old plugin files retired (renamed to .bak)

## Reference Files
- Template: crates/op-plugins/src/state_plugins/plugin_scaffold.rs.template
- Gold standard: crates/op-plugins/src/state_plugins/xray.rs
- Adapter: crates/op-blob/src/adapter.rs
- Schema types: crates/op-state-store/src/plugin_schema.rs
- OSCAL validation: crates/op-plugins/src/state_plugins/common/oscal.rs
- Module tree: crates/op-plugins/src/state_plugins/mod.rs
