# Root `src/` Audit

## Goal

Remove the ambiguous top-level `src/` directory from the workspace root so
LLM/code-assist tools do not confuse legacy root-package code with
crate-scoped source trees under `crates/*/src`.

## Audit Method

The audit classified files under the old root `src/` tree by checking:

1. Cargo entrypoints for the root `op-dbus` package
2. The root library module graph from `src/lib.rs`
3. Direct consumers in `src/main.rs`
4. Orphaned files that exist under `src/` but are not declared in the active
   module graph

## Current Build Reality Before Rename

- The workspace root is both a workspace manifest and the manifest for the
  `op-dbus` package.
- The `op-dbus` package was compiling from the top-level `src/` tree.
- The root binary uses `op_web` directly for web serving; the old `src/web/*`
  tree was legacy.

## Post-Rename State

- The top-level `src/` directory has been renamed to `root-package-src/`.
- `Cargo.toml` now points the root package at explicit paths under
  `root-package-src/` for the library and the three root binaries.
- `cargo check` passes against the renamed root package layout.

## Classification

### Active Root Package Entry Points

These are authoritative for the root package and were actively compiled:

- `src/lib.rs`
- `src/main.rs`
- `src/bin/inspector-onboard.rs`
- `src/bin/microsoft-device-login.rs`

### Active Root Library Modules

These are declared by `src/lib.rs` and therefore part of the compiled root
package:

- `src/json_rpc.rs`
- `src/plugin.rs`
- `src/work_stack.rs`
- `src/blockchain.rs`
- `src/cache.rs`
- `src/mcp.rs`
- `src/mcp_live.rs`
- `src/error.rs`
- `src/plugins.rs`
- `src/pre_canned.rs`
- `src/security.rs`
- `src/chatbot/mod.rs`
- `src/chatbot/intent.rs`
- `src/chatbot/planner.rs`
- `src/chatbot/session.rs`
- `src/chatbot/maintenance.rs`
- `src/chatbot/cognitive.rs`
- `src/inspector_gadget/mod.rs`
- `src/inspector_gadget/introspective.rs`
- `src/policy/mod.rs`
- `src/dependency/mod.rs`
- `src/disaster_recovery/mod.rs`
- `src/vectorization/mod.rs`
- `src/numa_cache/mod.rs`

### Inactive Orphaned Files Under Root `src/`

These files existed under the old root `src/` tree but were not wired into the
active root module graph:

- `src/chatbot/live_mcp.rs`
- `src/chatbot/tools.rs`
- `src/chatbot/web.rs`
- `src/chatbot/web_ui.html`
- `src/tool/mod.rs`
- `src/tool/builtin.rs`
- `src/tool/dbus.rs`
- `src/tool/incus.rs`
- `src/tool/ovs.rs`

Reason:

- `src/chatbot/mod.rs` does not declare `live_mcp`, `tools`, or `web`.
- `src/lib.rs` has `pub mod tool;` commented out, and there are no active root
  imports of `crate::tool`.

### Legacy Web Tree

The following files were legacy root-web leftovers and had already been
identified as obsolete because the root binary serves web traffic through the
`op-web` crate:

- `src/web/mod.rs`
- `src/web/routes.rs`
- `src/web/server.rs`
- `src/web/websocket.rs`

## Workspace Overlap

The old root tree is not uniformly obsolete. Some modules clearly overlap with
crate-owned implementations; others are still the only live implementation for
the root package.

### Strong Crate Overlap

These root modules have direct crate-era counterparts by symbol or domain:

- `src/chatbot/*` overlaps with `crates/op-chat/src/*`
- `src/blockchain.rs` overlaps with `crates/op-blockchain/src/blockchain.rs`
  and `crates/op-blockchain/src/streaming_blockchain.rs`
- `src/cache.rs` overlaps with `crates/op-cache/src/btrfs_cache.rs`
- `src/json_rpc.rs` overlaps with `crates/op-jsonrpc/src/protocol.rs`
- `src/security.rs` overlaps with `crates/op-tools/src/security.rs`
- `src/vectorization/mod.rs` overlaps with
  `crates/op-blockchain/src/plugin_footprint.rs`
- `src/plugin.rs` and `src/plugins.rs` overlap with `crates/op-plugins/src/*`
- `src/inspector_gadget/*` overlaps with `crates/op-inspector/src/*` and
  `crates/op-introspection/src/*`

### Root-Only Or Mixed Glue

These modules are still active under the root package, but no crate-local
replacement was confirmed as a drop-in successor during this audit:

- `src/mcp.rs`
- `src/mcp_live.rs`
- `src/work_stack.rs`
- `src/policy/mod.rs`
- `src/dependency/mod.rs`
- `src/disaster_recovery/mod.rs`
- `src/pre_canned.rs`
- `src/error.rs`
- `src/numa_cache/mod.rs`

That means the safe interpretation is:

- some of `root-package-src/` is stale duplicate logic
- some of `root-package-src/` is still authoritative root glue
- deleting the entire tree would have been unsafe

## Git History Compare

Representative `git log --follow` checks support the “pre-refactor leftovers”
hypothesis for some domains, but not all of them.

### Likely Stale Legacy Copies

These root files only show the initial import commit, while their crate-era
counterparts continued to change later:

- `src/chatbot/mod.rs`: 1 commit
  `crates/op-chat/src/lib.rs`: 2 commits
- `src/blockchain.rs`: 1 commit
  `crates/op-blockchain/src/blockchain.rs`: 2 commits
- `src/cache.rs`: 1 commit
  `crates/op-cache/src/btrfs_cache.rs`: 3 commits
- `src/json_rpc.rs`: 1 commit
  `crates/op-jsonrpc/src/protocol.rs`: 3 commits

This is strong evidence that those root copies are legacy holdovers, even
though some are still compiled by the root package today.

### Not Yet Safe To Declare Obsolete

These root domains and their crate counterparts both received later edits:

- `src/inspector_gadget/mod.rs` and `crates/op-inspector/src/lib.rs`
- `src/plugin.rs` and `crates/op-plugins/src/lib.rs`

These need migration work, not blind deletion.

## Root Cause Summary

The top-level `src/` directory was not a clean “old code” bucket. It mixed:

- active root-package source
- compatibility shims
- dead/orphaned files
- already-obsolete legacy web code

That mix is what made the layout dangerous for automated tools and humans.

## Executed Cleanup

The following cleanup has already been applied:

1. Renamed `src/` to `root-package-src/`
2. Repointed `Cargo.toml` to explicit root lib/bin paths
3. Removed the obsolete root `src/web/*` tree
4. Removed inactive orphaned files:
   - `root-package-src/chatbot/live_mcp.rs`
   - `root-package-src/chatbot/tools.rs`
   - `root-package-src/chatbot/web.rs`
   - `root-package-src/chatbot/web_ui.html`
   - `root-package-src/tool/mod.rs`
   - `root-package-src/tool/builtin.rs`
   - `root-package-src/tool/dbus.rs`
   - `root-package-src/tool/incus.rs`
   - `root-package-src/tool/ovs.rs`
5. Verified the workspace still builds with `cargo check`

## Safe Immediate Action

The safe immediate action is to remove the top-level `src/` name itself,
without pretending every file inside it is obsolete.

That action is now complete:

1. Renaming `src/` to `root-package-src/`
2. Repointing the root package manifest to explicit lib/bin paths
3. Preserving active root-package code while removing the ambiguous path name

## Follow-Up Cleanup

After the rename, the next safe reduction steps are:

1. Continue deleting only files proven to be orphaned beyond the ones already
   removed
2. Move active root-package domains into crate-scoped packages under
   `crates/*/src`
3. Eventually remove `root-package-src/` entirely once the root package becomes
   a thin shim or is fully relocated
