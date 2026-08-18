# Plugin Curation — Typed Method/Return Foundation

Purpose: curate the full plugin set before we (1) add typed returns to existing
methods and (2) add method surfaces to the method-less plugins.

How to use:
- In the **Keep?** column put `keep`, `deprecate`, or `merge:<other>`.
- Add any notes (correct method ideas, deprecation reason, rename) in **Notes**.
- Leave the rest; I'll read this file back and act on it.

Registration is discovery-based via `inventory::submit!(PluginReg)`.
**65 plugins are registered (active).** 3 files exist but are NOT registered (orphans).

Method counts = number of schema method declarations in this curation snapshot;
verify the source before acting on a row. Method return migration is at mixed
stages: the Antigravity rows already use typed inputs and outputs. The 4 plugins
marked `*` declare methods via a path other than `.methods.insert` (migration
must handle them specially).

---

## A. Registered plugins WITH methods (40) — curate and complete typed returns

| Keep? | Plugin | File | Methods | Notes |
|-------|--------|------|--------:|-------|
|       | login1 | login1.rs | 23 | |
|       | dnsresolver | dnsresolver.rs | 20 | |
|       | ovsdb_bridge | ovsdb_bridge.rs | 14 | OVS datapath; OpenFlow controller = xray. Two flow tables: xray-controlled (routing) + obfuscation. |
|       | incus `*` | incus.rs | 13 | non-insert path |
|       | btrfs | btrfs_plugin.rs | 12 | |
|       | qdrant | qdrant.rs | 10 | |
|       | s6_systemctl | s6_systemctl.rs | 10 | |
|       | mail_server | mail_server.rs | 9 | |
|       | rtnetlink | rtnetlink.rs | 9 | |
|       | s6 | s6.rs | 8 | |
|       | wgcf | wgcf.rs | 8 | |
|       | xray | xray.rs | 8 | |
|       | cozo | cozo.rs | 7 | |
|       | freedesktop `*` | freedesktop.rs | 7 | non-insert path |
|       | oci `*` | oci.rs | 7 | non-insert path |
|       | procfs | procfs.rs | 7 | |
|       | rovs_commands `*` | rovs_commands.rs | 7 | non-insert path; has real domain results; OVS controlled by xray |
|       | netmaker | netmaker.rs | 6 | |
|       | packagekit | packagekit.rs | 6 | |
|       | wireguard | wireguard.rs | 6 | |
|       | fail2ban | fail2ban.rs | 4 | |
|       | software | software.rs | 4 | |
|       | unix_socket | unix_socket.rs | 4 | |
|       | users | users.rs | 4 | |
|       | workflows | workflows_plugin.rs | 4 | |
|       | openflow | openflow.rs | 3 | OVS; controller = xray. Two OpenFlow tables: xray-controlled (routing) + obfuscation. |
|       | persona | persona.rs | 3 | |
| deprecate | privacy_router | privacy_router.rs | 3 | Get rid of. Consumed privacy_routes (also gone). Routing → xray/zeroclaw; OVS obfuscation handled by openflow/openflow_obfuscation/ovsdb_bridge. |
|       | proxy_server | proxy_server.rs | 3 | |
|       | service | service.rs | 3 | |
|       | zeroclaw | zeroclaw.rs | 3 | has real domain results |
|       | antigravity | antigravity.rs | 3 | typed I/O; auth/usage/safety only; model catalog delegated to large_language_model |
|       | antigravity_chat | antigravity_chat.rs | 3 | typed I/O; schema-declared bridge controls; model catalog delegated to large_language_model |
|       | net | net.rs | 2 | |
|       | openflow_obfuscation | openflow_obfuscation.rs | 2 | OVS; the obfuscation OpenFlow table (sibling of the xray-controlled routing table). |
|       | pcidecl | pcidecl.rs | 2 | |
|       | keypair | keypair.rs | 1 | |

Subtotal methods: 269

---

## B. Registered plugins WITH NO methods (25) — need method surfaces

These declare a schema/state but expose zero callable methods. Research agents
will propose optimal methods per plugin AFTER you curate (keep/deprecate).

| Keep? | Plugin | File | Notes |
|-------|--------|------|-------|
|       | adc | adc.rs | |
|       | agent_config | agent_config.rs | |
|       | blockchain | blockchain_plugin.rs | |
| keep  | cognitive_mcp | cognitive_mcp.rs | Universal MCP gateway :3003. Absorbs `mcp` registry + compact toolset. Gating: local agents = full execute access; chatbot = read-only (no execute). |
| keep  | compact_mcp | compact_mcp.rs | Compact toolset surfaced via cognitive_mcp (local-agent gated). compact_mcp server lifecycle retained. |
|       | config | config.rs | |
|       | cron | cron.rs | |
|       | ctl_plane_chatbot | ctl_plane_chatbot.rs | |
| keep  | datastore | datastore.rs | NOT a dup: read-only projection of canonical op-state-store (OD-30). Methods = obs over store index/namespaces/counts. |
|       | endpoint | endpoint.rs | |
|       | factory | factory.rs | |
|       | full_system | full_system.rs | |
|       | gemma_brain | gemma_brain.rs | |
|       | hardware | hardware.rs | |
|       | keyring | keyring.rs | |
| deprecate | knowledge | knowledge_plugin.rs | cognitive_mcp was refactored onto qdrant directly, not knowledge. Superseded. |
|       | large_language_model | large_language_model.rs | |
| merge:cognitive_mcp | mcp | mcp.rs | Fold registry + tool_groups/access-gating into cognitive_mcp. Compact toolset exposed via cognitive_mcp gated to LOCAL AGENTS ONLY (full execute access); chatbot excluded (no execute power). |
|       | memory | memory_plugin.rs | |
|       | notebooklm | notebooklm.rs | |
|       | oscal_subid_registry | oscal_subid_registry.rs | |
| deprecate | privacy_routes | privacy_routes.rs | zeroclaw handles all routing now. Superseded. (privacy_router consumes it — see open question.) |
|       | schema_renderer | schema_renderer.rs | |
| deprecate | sess_decl | sessdecl.rs | Get rid of. Overlaps login1 (live sessions) + persona. |

---

## C. Orphan files — present but NOT registered (likely deprecated)

| Keep? | File | Notes |
|-------|------|-------|
| this is necessary to have independant shared socket not connected to container      | shared_unix_socket.rs | no inventory::submit; superseded by unix_socket? |

---

## Decisions already locked
- Add typed returns to all kept methods (populate typed `result`).
- Add method surfaces to kept method-less plugins (B), researched per plugin.
- Returns derived from real implementations; typed `Ack` only where no domain
  return exists. As robust/complete as possible.
