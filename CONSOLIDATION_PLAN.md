# Deploy/Docs Consolidation Plan
**Generated:** 2026-07-20  
**Active Repo:** `/home/admin/git/odbus` (branch: claude/artix-s6-install-script-ao9c5x)  
**Inspect Copy:** `/mnt/opt-inspect/home/git/operation-dbus-proto/`

## Executive Summary

Inspected 307+ files/directories across two legacy directories against the current active codebase. Key findings:

**Deploy Directory:**
- **58 STALE** files violating current architecture (systemd units, systemctl scripts, removed daemons, forbidden patterns)
- **32 MATCH** files identical to active repo
- **36+ scripts** using `systemctl` instead of mandatory `service6` wrapper
- **Critical violations:** entire `systemd/` directory, AF_XDP/netplan/host-WG scripts, disk-backed xray configs

**Docs Directory:**
- **82 MATCH** files identical to active repo
- **56 NEW** files with valuable content not in active repo
- **7 STALE** docs with pre-blob-catalog architecture or deprecated daemons
- **Critical finds:** s6 boot recovery handoff, 73KB WireGuard auth guide, complete feature review matrix (34 crates)

## Approach

1. **Conservative merge strategy:** preserve all NEW/MERGE content in separate staging area before any deletions
2. **Architecture compliance filter:** reject any content violating CLAUDE.md/AGENTS.md invariants (D-Bus control plane, s6-only host, SHM-reactive, sealed blob catalog)
3. **Documentation value extraction:** mine historical handoffs and incident recovery docs for operational knowledge
4. **Deduplication priority:** active repo wins where content matches; inspection copy provides additive value only

---

## Deploy Directory Classification

### STALE (Delete After Review) — 58 files

**Host systemd units (entire directory):**
```
systemd/op-web.service
systemd/op-dbus.service
systemd/op-mcp.service
systemd/op-mcp-compact.service
systemd/op-mcp-agents.service
systemd/op-agents.service
systemd/op-cli-notebook-log-sync@.service
systemd/op-dbus.target
systemd/mcp-smart-gateway.service
systemd/streaming-logs.service.root
systemd/op-web.service.journal
op-dbus-services.service
op-dbus-services.conf
```
**Reason:** Host services must use s6 supervision exclusively per AGENTS.md

**Scripts using systemctl (36+ files):**
```
start-services.sh
install-services.sh
deploy-network.sh
fix-all.sh
fix-nginx-ghostbridge.sh
fix-steering.sh
icus-ctl.sh
release-server-install.sh
setup-assistant-incus.sh
setup-chimera-icus.sh
setup-cloudflare-tls.sh
setup-cozo.sh
setup-ghostbridge-public.sh
setup-letsencrypt-cloudflare.sh
setup-public-domain.sh
setup-with-existing-proxmox.sh
setup-zeroclaw-wayland.sh
target-setup.sh
verify-cloudflare-tls.sh
verify-tls.sh
lib/systemd.sh
```
**Reason:** Violates mandatory `sudo service6 ...` wrapper policy

**Deprecated installers:**
```
install.sh
base-install.sh
upgrade.sh
uninstall.sh
setup-complete.sh (identical to base-install.sh)
```
**Reason:** Canonical installer is `install/3tched-artix-s6-install.sh`; these use apt/systemctl

**Removed daemon references:**
```
install-op-openvswitch-daemon.sh
s6/recompile-and-update.sh (uses raw s6-rc)
deploy.sh (uses raw s6-svc)
```
**Reason:** `op-openvswitch-daemon` removed (replaced by rovs plugins); raw s6 commands bypass service6 wrapper

**Architecture violations:**
```
setup-hypervisor-xray.sh
apply-afxdp-boot.sh
cutover-afxdp.sh
netplan/ (directory)
op-xdp-wg/ (directory)
```
**Reason:** Writes to `/etc/xray/config.json` (must use `/dev/shm/xray_config.json`); AF_XDP/netplan/host-WG explicitly forbidden per active README

**Old structure artifacts:**
```
README.md (old install.sh structure)
novnc-cutover.sh (superseded by novnc-ovs-cutover.sh)
services/ (directory)
services.conf
services.log
archive/ (directory)
chat.log
dinit.log
```
**Reason:** Superseded by current structure or historical logs

### MATCH (Keep, Already in Active Repo) — 32 files

```
AGENT-INTEGRATION.md
PKGBUILD-sdbusplus-s6
base-components.md
build-sdbusplus-s6.sh
btrfs-layout.sh
deploy-blob-gemma4.sh
environment.default
email-config.env
qdrant-container-config.yaml
qdrant-repomix-local.yaml
novnc-ovs-cutover.sh
dbus/org.opdbus.conf
dbus/org.dbusmcp.conf
config/ (directory + contents)
incus-hooks/ (directory + contents)
mcp/ (directory + contents)
monitoring/ (directory + contents)
nextdns/ (directory + contents)
scripts/btrfs-chroot.sh
scripts/incus-debian-mount.sh
scripts/provision-workspace-subscriber.sh
xray/client.json
```
**Action:** No action needed — content identical

### MERGE (Needs Detailed Comparison) — 3 directories

```
netmaker/ (active has 4 files, inspect has more including incus-compose.yaml)
scripts/ (both have same core files but may differ in details)
s6/ (large directory, many services in both repos)
```
**Action:** File-by-file diff to extract any unique useful content from inspect copy

### NEW (Not in Active Repo) — 4 items

**Actually already in active (false positive):**
```
99-agent-s6-guard.hook
agent-s6-guard.sh
agent-service6.sudoers
```
**Verified in active:** `/home/admin/git/odbus/deploy/` contains all three agent policy files

**Truly new:**
```
netmaker/incus-compose.yaml
rust-network-mgr/ (appears to be new component)
```

### UNKNOWN (Needs Manual Inspection) — 63 files

These require checking for systemctl usage patterns or determining if content is container-scoped vs host-scoped:

```
systemd/networkd/ (may be container-specific)
check-tls-status.sh
cleanup-local-3tched-interfaces.sh
cloudflare-dns-setup.sh
deploy-local-tls.sh
diagnose-connection.sh
dnsmasq/ (directory)
find-media-certs.sh
fix-mail-3tched-network.sh
ghostbridge/ (directory)
hooks/ (directory)
ide-tunnel/ (directory)
incus/ (directory with container configs)
install-dashboard-nginx.sh
install-makepkg-hook.sh
install-ovsbr0-netdev.sh
lib/common.sh
lib/build.sh
lib/nginx.sh
lib/tls.sh
lib/install-binaries.sh
lib/agent-integration.sh
netclient-s6/ (directory)
nginx/ (directory)
op-assistant-grpc-deploy.sh
oracle-decoy-ingress/ (directory)
quick-setup-chimera.sh
registration/ (directory)
run_foreground.sh
setup-assistant-domain.sh
setup-bridge.sh
setup-hypervisor-controller.sh
setup-hypervisor-netclient.sh
setup-mail-domain.sh
setup-qdrant-incus.sh
setup-registration-domain.sh
webmail/ (directory)
wireguard-add-client.sh
CHIMERA-ICUS-README.md
DOMAIN-SETUP.md
PROXMOX-SETUP.md
caddy/ (directory)
```

---

## Docs Directory Classification

### MATCH (Keep, Already in Active Repo) — 82 files

All proto reference docs, core guides, and architecture docs that exist identically in both repos:

```
COGNITIVE_MCP_CLIENT_GUIDE.md
CONTEXT_AWARENESS.md
DBUS_INDEXER_IMPLEMENTATION_GUIDE.md (stub)
HIERARCHICAL_DBUS_DESIGN.md (stub)
LOVABLE_PROMPT.md
PLUGIN-DEVELOPMENT-GUIDE.md (stub)
README.md (active has cleaner 2026-07-20 version)
SNAPSHOT_AUTOMATION.md (stub)
SPEC_AND_DESIGN_INDEX.md
architecture-flow.md
check-links.sh
d_bus_introspection_with_zbus.md (stub)
ghostbridge-identity-sled-snowball-pipeline.md
mail-server-setup.md
mcp-vscode-bridge.md
network-address-table.md
nvidia-inception-brainstorm.md
plugin-schema-refactor-guidance.md
pyproject.toml
schema-as-code.md
schema-coupled-plugin-blob-reflection-whitepaper.md
schema-from-structs.md
subid-taxonomy.md
vectors.md
zenflow-byom-profile.md
architecture/completion-status.md
architecture/state-flow.md
guides/user-guide.md
op-gateway/README.md
operations/ghostbridge-incus-ovs-architecture.md
operations/microsoft-agent-auth.md
operations/mirror-projection.md
operations/mutation-paths.md
operations/op-dbus-dinit.md
operations/op-web-ui-build.md
overview/architecture.md
plugins/create-and-register.md
plugins/plugin-catalog.md
plugins/system-overview.md
reference/api-reference.md
reference/proto/README.md
(+ 44 proto docs under reference/proto/)
schema/plugin-contracts.md
schema/registry-coverage.md
specs/ctl-plane-chatbot-reasoning-vectorization.md
specs/op-core.md
ui/API.md
ui/COMPONENTS.md
ui/README.md
```

### NEW (Valuable Content Not in Active Repo) — 56 files

**Critical operational docs:**
```
s6-boot-recovery-gemma-ollama-handoff.md (2026-07-02 incident recovery)
WG-SESSION-ID.md (73KB complete WireGuard auth implementation)
OVS-NATIVE-SETUP.md (native OVSDB JSON-RPC guide)
```

**Feature/build verification:**
```
feature-review/ (34 files: comprehensive per-crate build status matrix)
feature-review/README.md (summary for 31 Rust crates + 2 frontends)
feature-review/crates/*.md (32 per-crate reviews)
feature-review/frontends/*.md (2 frontend reviews)
```

**Code reviews:**
```
collected-code-reviews/ (6 files)
collected-code-reviews/operation-dbus/*.md
collected-code-reviews/operation-dbus-standalone-flat/*.md
```

**Historical/planning docs:**
```
CODEX_RAW_DATA_20260212.tar.gz (compressed archive)
CONVERSATION_EXTRACTION_20260212.md
FACTORY_MCP_SETUP.md
FULL_CONVERSATION_HISTORY_20260212.md (1.6MB)
GENERATION_STATUS.md (2026-02-16 doc generation status)
RAW_CONVERSATION_HISTORY_20260212.jsonl (1.6MB)
README_GENERATION.md
codeassist-review.txt
inception-narrative-plan.md (NVIDIA Inception narrative with TODOs)
kiro-spec-workflow.md
orphan-opdbus-binary.md
```

**Architecture additions:**
```
architecture/privacy-network-architecture.md (wgcf/WARP, privacy routing)
```

**Operations additions:**
```
operations/code-assist-escalation-2026-02-11.md
operations/xdp-afxdp-conversation-2026-05-23.md (XDP/AF_XDP bug fixes)
```

**Service docs:**
```
op-services/README.md
op-services/cli-notebook-log-sync.md
```

**Planning:**
```
planning/ (3 files)
planning/op-cache-review.md
planning/op-chat-review.md
planning/openclaw-chatbot-and-code-indexing.md
```

**Prompts/refactors:**
```
prompts/dbus-mirror-event-session-refactor.md
```

**Recycled bin (historical):**
```
recycled-bin/2026-05-22-old-docs/*.gz
recycled-bin/2026-05-22-old-docs/*.jsonl
```

### STALE (Remove or Archive) — 7 files

```
AUDIT_PLUGIN_ARCHITECTURE_2024.md
PLUGIN_ARCHITECTURE_CLEANUP_SUMMARY.md
PLUGIN_SCHEMA_MIGRATION_COMPLETE.md
schema-coupled-plugin-blob-reflection-handoff.md
reference/proto/op-openvswitch-daemon/ (directory + 3 proto docs)
```
**Reason:** Pre-blob-catalog architecture, legacy D-Bus paths, references to `op-openvswitch-daemon` (deprecated per CLAUDE.md)

### MERGE (Reconcile Differences) — 4 items

```
README.md (active has cleaner 2026-07-20 version; inspect has historical content)
BLOB_ARCHITECTURE_SYNTHESIS.md (historical context could augment whitepaper)
architecture/ (inspect has privacy doc, active has socket readiness ADR)
operations/ (inspect has 4 additional docs including valuable XDP conversation)
```

---

## Prioritized Next Steps

### Phase 1: Safe Extraction (Do First)
1. **Copy NEW docs to staging:** `mkdir -p /home/admin/git/odbus/.consolidation-staging/docs-new/` and copy all 56 NEW files
2. **Copy MERGE candidates:** Copy 4 MERGE items to `.consolidation-staging/docs-merge/`
3. **Extract s6 boot recovery:** Move `s6-boot-recovery-gemma-ollama-handoff.md` to `docs/operations/` (critical operational knowledge)
4. **Extract WG auth guide:** Move `WG-SESSION-ID.md` to `docs/architecture/` (implementation reference)
5. **Extract feature review matrix:** Move `feature-review/` to `docs/` (build verification baseline)
6. **Preserve historical archives:** Move `CODEX_RAW_DATA_20260212.tar.gz` and conversation histories to `docs/historical/`

### Phase 2: Deploy Directory Cleanup (After Extraction)
1. **Archive STALE deploy files:** Move 58 STALE files to `.consolidation-staging/deploy-stale-archive/`
2. **Document violations:** Create `STALE_VIOLATIONS.md` listing all systemd/systemctl/deprecated patterns for reference
3. **Extract UNKNOWN utilities:** Manually review 63 UNKNOWN files for container-scoped configs or useful utilities
4. **Merge netmaker/scripts/s6:** File-by-file diff of 3 MERGE directories, extract any inspect-only useful content

### Phase 3: Docs Directory Integration (After Phase 1)
1. **Merge README.md:** Extract historical handoff content from inspect README, preserve as `docs/historical/README-handoffs.md`
2. **Merge architecture/:** Copy `privacy-network-architecture.md` to active `docs/architecture/`
3. **Merge operations/:** Copy 4 additional operation docs (including XDP conversation) to active `docs/operations/`
4. **Archive STALE docs:** Move 7 STALE docs to `.consolidation-staging/docs-stale-archive/`
5. **Integrate BLOB_ARCHITECTURE_SYNTHESIS.md:** Extract historical context as footnote/appendix to current whitepaper

### Phase 4: Verification & Cleanup (Final)
1. **Update SPEC_AND_DESIGN_INDEX.md:** Add new docs to master index
2. **Run link checker:** `bash docs/check-links.sh` on updated docs/
3. **Verify no broken references:** Grep for references to removed/moved files
4. **Update WISHLIST.md:** Add tasks for processing NEW content (e.g., OD-## for feature review integration)
5. **Commit consolidation:** Stage and commit with message: "docs: consolidate inspect copy — extract NEW/MERGE content, archive STALE violations"

---

## Items Needing User Clarification

### High Priority
1. **rust-network-mgr/ in inspect deploy:** Is this a new component under development or obsolete? (No equivalent in active repo)
2. **netmaker/incus-compose.yaml:** Is this Incus container composition preferred over current netmaker/ configs?
3. **Container systemd units:** Should `incus/privacy-xray-ingress/` and similar container-scoped systemd files be preserved? (Policy allows systemd *inside* containers)

### Medium Priority
4. **Historical conversation data (3.2MB):** Preserve full CODEX/conversation archives or extract digest only?
5. **Feature review integration:** Should feature-review matrix be CI-integrated or remain as static docs?
6. **Inception narrative TODOs:** Should `inception-narrative-plan.md` with founder story TODOs be assigned to an agent (per WISHLIST pattern)?

### Low Priority
7. **Caddy vs nginx:** Inspect has `caddy/` directory — is Caddy in use or was it replaced by nginx?
8. **UNKNOWN utilities (63 files):** Bulk manual review tedious — prioritize subset or assign to script that checks for systemctl/service violations?
9. **Recycled bin archives:** Keep in docs/historical/ or move to separate artifact repo?

---

## Safety Constraints

**Never delete without backup:**
- All STALE files moved to `.consolidation-staging/` first
- Git history preserves all deletions (can restore from inspect mount or git)

**Architecture compliance gate:**
- No content violating CLAUDE.md/AGENTS.md invariants enters active repo
- All systemd host service units stay archived
- All systemctl scripts stay archived unless verified container-scoped

**Active repo precedence:**
- Where MATCH exists, active repo version wins
- Inspect copy provides additive value only via NEW/MERGE

**Preserve operational knowledge:**
- All incident recovery handoffs, critical guides preserved
- Historical context extractable from archives
- No blind deletion of NEW content

---

## Estimated Effort

- **Phase 1 (Safe Extraction):** 2-3 hours (scripted copying + manual review of critical docs)
- **Phase 2 (Deploy Cleanup):** 3-4 hours (63 UNKNOWN files need manual systemctl check + 3 MERGE dirs diff)
- **Phase 3 (Docs Integration):** 1-2 hours (straightforward copying + index update)
- **Phase 4 (Verification):** 1 hour (link checker + grep + commit)

**Total:** 7-10 hours of focused work

---

---

## Section-Level Extraction Addendum (User Requested)

The user requested section-level consolidation rather than whole-file copying. This means mixed documents (partly current, partly obsolete) must be split into useful excerpts and stale excerpts, with only the useful excerpts promoted into the active repo.

### Section-level targets

| Source | Keep Sections | Drop Sections | Output |
|--------|---------------|---------------|--------|
| `docs/s6-boot-recovery-gemma-ollama-handoff.md` | Bootdb recovery procedure, `s6-apply` safety notes, `op-web-srv` notification-fd wedge, gemma failure-masking bug | References to `op-openvswitch-daemon`, raw `s6` commands, PR numbers, caddy deletion | `docs/operations/artix-s6-bootdb-recovery.md` |
| `docs/WG-SESSION-ID.md` | Cryptographic principles (Argon2 PSK rotation, session lifecycle, zero-trust identity), key rotation rationale | `wg-auth-service` implementation, JSON-RPC methods, NetworkManager plugin, deployment commands, systemd integration | `docs/architecture/wireguard-identity-principles.md` |
| `docs/OVS-NATIVE-SETUP.md` | Native OVSDB JSON-RPC protocol, transaction examples, Rust `OvsdbClient` usage | `systemctl` troubleshooting commands, shell-script setup that may invoke CLI | `docs/guides/ovs-native-jsonrpc.md` |
| `docs/BLOB_ARCHITECTURE_SYNTHESIS.md` | Blob definition, lifecycle, current state analysis, why-it-matters for zeroclaw/gemma | BTRFS packaging details if they conflict with AGENTS.md "zero-btrfs-overhead" identity rule | Appendix to `docs/schema-coupled-plugin-blob-reflection-whitepaper.md` |
| `docs/operations/xdp-afxdp-conversation-2026-05-23.md` | XDP program coexistence debugging, targeted `xdp-loader unload` pattern, `op-ovsbr0-afxdp` wiring | Reboot guidance and any host AF_XDP cutover recommendations | `docs/operations/xdp-debugging-patterns.md` with a warning that host AF_XDP is now forbidden |
| `docs/operations/op-dbus-dinit.md` | None — dinit is not the current runtime | Entire document | Archive as `.consolidation-staging/docs-stale-archive/op-dbus-dinit.md` |
| `deploy/README.md` | Historical evolution note about deploy structure | All systemd-specific commands and `systemctl` status checks | `docs/historical/deploy-readme-evolution.md` |
| `docs/ghostbridge-identity-sled-snowball-pipeline.md` (inspect) | Identity/sled/shuttle concepts, SHM paths | Port `18789`, old `wg-xray` datapath | Merge into current top-level `opblob-shm-handoff.md` or `docs/operations/ghostbridge-incus-ovs-architecture.md` |
| `docs/subid-taxonomy.md` (inspect) | Taxonomy rules and examples | Port `18789` references | Merge into current `docs/subid-taxonomy.md` |
| `docs/zenflow-byom-profile.md` (inspect) | BYOM profile content | Port `18789` references | Merge into current top-level `zenflow-byom-profile.md` |

### Section-level rules

1. Active repo wins: if a document already exists in `/home/admin/git/odbus`, produce a diff-style merge instead of overwriting. Extract only the missing/updating sections.
2. No systemd content enters active repo except inside explicit container contexts (e.g., `incus/privacy-xray-ingress/`). If a section contains `systemctl`, archive it.
3. Port `18789` is retired — replace with `8090` when extracting into current docs, or drop the obsolete section.
4. `op-openvswitch-daemon` is removed — drop sections that reference it or replace with "rovs plugins" where possible.
5. Host AF_XDP/WireGuard are forbidden — extract only debugging principles, not cutover instructions.
6. Every extracted section gets a source attribution footer: `<!-- Extracted from <source> on 2026-07-20 -->`.
7. Stale excerpts are placed in `.consolidation-staging/docs-stale-excerpts/` for reference, not deleted from the inspect mount.

### Updated Phase 1a: Section-Level Extraction

1. Create staging dirs:
   ```bash
   mkdir -p /home/admin/git/odbus/.consolidation-staging/{docs-new,docs-merge,docs-stale-excerpts,docs-stale-archive,deploy-stale-archive}
   ```
2. For each section-level target above, spawn a `worker` subagent to read the source, extract the keep sections, and write the cleaned output to the staging target path.
3. For MATCH files that are identical, do nothing.
4. For fully NEW files that are invariant-clean, copy whole files to `docs-new/` (e.g., `feature-review/`, `collected-code-reviews/`, `FACTORY_MCP_SETUP.md`).
5. For fully STALE files, copy whole files to `docs-stale-archive/` (e.g., `AUDIT_PLUGIN_ARCHITECTURE_2024.md`, `PLUGIN_SCHEMA_MIGRATION_COMPLETE.md`).
6. For deploy files, run a script that checks for `systemctl`/`s6-svc`/`s6-rc`/`s6 live install`/`s6 set commit` usage; any file containing these goes to `deploy-stale-archive/`. Files without these go to a manual review bucket.

## Success Criteria (Section-Level)

- [ ] Section-level extraction plan above executed for all 10 mixed documents
- [ ] Stale excerpts preserved in `.consolidation-staging/docs-stale-excerpts/` for audit
- [ ] Active repo docs updated only via merge/diff, never overwritten
- [ ] No `systemctl` or `op-openvswitch-daemon` references in newly-integrated active-repo docs
- [ ] Port `18789` replaced with `8090` in all extracted sections
- [ ] `docs/check-links.sh` passes after integration
- [ ] All changes staged and committed with a clear consolidation message
