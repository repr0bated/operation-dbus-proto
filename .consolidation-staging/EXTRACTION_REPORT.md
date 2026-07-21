# Section-Level Extraction Report

**Generated:** 2026-07-20
**Active repo:** `/home/admin/git/odbus`
**Inspect copy:** `/mnt/opt-inspect/home/git/operation-dbus-proto/`
**Staging root:** `/home/admin/git/odbus/.consolidation-staging/`

## Summary

- 6 mixed documents extracted into cleaned section-level outputs
- 50 new docs copied whole to staging
- 17 stale docs archived whole
- 91 deploy files auto-archived for architecture violations
- 358 deploy files placed in manual review bucket
- Redundant analysis outputs removed

## Section-level extractions (real merge content)

| Source | Output | Status |
|--------|--------|--------|
| `docs/s6-boot-recovery-gemma-ollama-handoff.md` | `docs-merge/operations/artix-s6-bootdb-recovery.md` | Ready to promote |
| `docs/OVS-NATIVE-SETUP.md` | `docs-merge/guides/ovs-native-jsonrpc.md` | Ready to promote |
| `docs/operations/xdp-afxdp-conversation-2026-05-23.md` | `docs-merge/operations/xdp-debugging-patterns.md` | Ready to promote |
| `docs/BLOB_ARCHITECTURE_SYNTHESIS.md` | `docs-merge/schema-coupled-plugin-blob-reflection-whitepaper-appendix.md` | Ready to promote |
| `docs/WG-SESSION-ID.md` | `docs-merge/architecture/wireguard-identity-principles.md` | Ready to promote |
| `deploy/README.md` | `docs-merge/historical/deploy-readme-evolution.md` | Ready to promote |

## Redundant / already-current content removed

The following inspect sources were found to already match the active repo (port 8090, current architecture), so their merge outputs are redundant:

- `docs/subid-taxonomy.md` — identical to active `docs/subid-taxonomy.md`
- `docs/zenflow-byom-profile.md` — identical to active `docs/zenflow-byom-profile.md`
- `docs/ghostbridge-identity-sled-snowball-pipeline.md` — active copy (2026-07-03) is more recent than inspect copy (2026-07-18)

Redundant analysis documents were removed from `docs-merge/`.

## New docs copied whole (`docs-new/`)

- `FACTORY_MCP_SETUP.md`
- `inception-narrative-plan.md`
- `kiro-spec-workflow.md`
- `architecture/privacy-network-architecture.md`
- `operations/code-assist-escalation-2026-02-11.md`
- `op-services/README.md` and `op-services/cli-notebook-log-sync.md` (note: these describe a dinit/systemd-like model and need review)
- `planning/` (3 files)
- `prompts/dbus-mirror-event-session-refactor.md`
- `feature-review/` (34 files)
- `collected-code-reviews/` (6 files)
- `codeassist-review.txt`

## Stale docs archived whole (`docs-stale-archive/`)

- `AUDIT_PLUGIN_ARCHITECTURE_2024.md`
- `PLUGIN_ARCHITECTURE_CLEANUP_SUMMARY.md`
- `PLUGIN_SCHEMA_MIGRATION_COMPLETE.md`
- `BLOB_ARCHITECTURE_SYNTHESIS.md` (original)
- `GENERATION_STATUS.md`
- `README_GENERATION.md`
- `CONVERSATION_EXTRACTION_20260212.md`
- `FULL_CONVERSATION_HISTORY_20260212.md`
- `CODEX_RAW_DATA_20260212.tar.gz`
- `RAW_CONVERSATION_HISTORY_20260212.jsonl`
- `op-dbus-dinit.md`
- `schema-coupled-plugin-blob-reflection-handoff.md` (pre-existing in archive)
- `reference/proto/op-openvswitch-daemon/` (3 proto docs)
- `recycled-bin/2026-05-22-old-docs/` contents

## Deploy scan results

- 91 files archived for violations (systemd, systemctl, raw s6, op-openvswitch-daemon, host AF_XDP/WireGuard, netplan, disk-backed xray config)
- 358 files placed in unknown review (no automatic violations detected, but may be outdated or container-scoped)
- 47 files skipped as identical to active `deploy/`

Notable archived violations:
- entire `systemd/` directory
- `install.sh`, `base-install.sh`, `upgrade.sh`, `uninstall.sh` (old systemd/apt installers)
- `s6/` directory (legacy host WireGuard/AF_XDP services)
- `op-xdp-wg/` and `netplan/` directories
- `lib/systemd.sh`, `lib/common.sh`, `lib/nginx.sh`, `lib/tls.sh`, `lib/install-binaries.sh`, `lib/agent-integration.sh`, `lib/build.sh`
- `ghostbridge/`, `ide-tunnel/`, `nginx/` configs, `caddy/` configs, `hooks/`, `archive/`, `services/`, `registration/`, `webmail/`, `xray/client.json`

Notable files in unknown review requiring manual decision:
- `setup-qdrant-incus.sh`, `setup-registration-domain.sh`, `setup-mail-domain.sh`, `setup-assistant-domain.sh`
- `netmaker/incus-compose.yaml` (new Incus composition?)
- `incus/privacy-xray-ingress/` (container-scoped systemd configs — may be valid inside containers)
- `config/subid-registry.json` (has 18789 entries?)
- `op-xdp-wg/bin/op-xdp-hostside.sh`, `op-xdp-wg/bin/op-xdp-watch.sh`, `op-xdp-wg/bin/op-xdp-detach.sh` (AF_XDP host binaries, but no obvious violation strings?)
- `s6/` directory contents (many need review; some may be container-socket services rather than host WireGuard)
- `lib/` non-violating files

## Items still needing user clarification

1. `rust-network-mgr/` in inspect deploy — is this a new component or obsolete?
2. `netmaker/incus-compose.yaml` — should this replace or supplement current `deploy/netmaker/` configs?
3. Container-scoped systemd files in `incus/privacy-xray-ingress/` — preserve as container configs or archive?
4. Historical conversation archives (3.2MB) — keep full or extract digest only?
5. `op-services/` docs in `docs-new/` — they describe a dinit/systemd-like `op-services` model; keep or archive?

## Recommended next steps

1. Review the 6 section-level outputs in `docs-merge/` and promote them to the active repo.
2. Review the 50 files in `docs-new/` and promote the useful ones, archive the rest.
3. Manually review the 358 files in `deploy-unknown-review/`.
4. Update `docs/SPEC_AND_DESIGN_INDEX.md` with promoted docs.
5. Run `docs/check-links.sh` after promotion.
6. Stage and commit.
