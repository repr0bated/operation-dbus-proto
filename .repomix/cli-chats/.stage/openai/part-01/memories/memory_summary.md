v1

## User Profile
The user works mostly in `/home/jeremy/git/operation-dbus-proto`, with adjacent work in `operation-dashboard-ui-07` plus host-local config under `~/.factory` and `~/.config/JetBrains/Air`. Their tasks routinely cross repo code, `/dev/shm` schema/blob state, host s6 services, D-Bus topology, WireGuard/OCI runtime state, and local config files.

They expect direct execution on terse asks, but only with evidence-backed status. In `operation-dbus-proto`, they repeatedly steer agents back to the live authority path: schema/blob/D-Bus/runtime truth from the current checkout and host, not guessed abstractions, docs, or stale prior mental models. When they ask to probe, introspect, or troubleshoot, they want the live object, service, bus, or network state checked first.

They are highly sensitive to overclaiming. If work is only scaffolded, partially wired, or only locally verified for one crate, they want that said plainly. They also care about exact ownership boundaries and terminology matching reality. When time is short, they may want a concrete handoff file written to disk, with the filename stated explicitly. If trust breaks or they say recovery is happening elsewhere, the safe default is to stop touching the repo until they reopen the work.

## User preferences
- When a request is action-oriented (`fix it`, `deploy locally`, `start crd s6 service`, `add grok as byok in factory`), execute directly and carry the obvious operational next step through verification.
- In `operation-dbus-proto`, prefer live schema/blob/D-Bus authority over guessed wrappers: `"schema driven"`, `"if the schema is missing it do0es not exist"`, and `"it is supposed to be writing blobs instead of schema now"`.
- When the user says to probe or introspect (`"why dont you probe"`, `"intospect the object"`), inspect the live host or D-Bus object first instead of staying in repo internals.
- Do not present sketches, docs, partial wiring, or dead-code helpers as complete. The user's explicit boundary is `"no stubs or placeholders"`.
- If progress is ambiguous, report it as designed, partially wired, or actually runnable. Do not smooth that distinction away.
- If the user asks for a handoff and especially says `"write handoff to file"` or `"filename?"`, create the file on disk and report the exact path.
- When the user narrows scope (`in ~/.factory`, `just relevant source`, `just plugin and scema`), pivot immediately and keep the change set tight.
- For `operation-dashboard-ui-07`, treat Lovable as static/layout iteration and Rust as the realtime/runtime path unless the repo proves otherwise.
- If the user says a shell change should be "persistent" or asks for it "in my bash", write the actual Bash startup files and verify in a fresh shell instead of relying on the current session.
- When shell tooling must work "as root tool" or the user says "put in my user and root", wire and verify both startup contexts separately.
- Use existing local key/config sources when the user points to them (`you can get key in opencode`, `you can get api key in ~/.factory/`) instead of asking them to paste secrets.
- For local deploys on this machine, default to targeted build/install/restart of the named binaries/services; switch to the broad deploy script only when the user explicitly chooses that path.
- When building or installing, verify the real repo first and report the exact install destination; do not blur `~/bin` convenience installs with deploy-script paths like `/usr/local/bin`.
- In live incident work such as OCI/WireGuard or D-Bus/plugin breakage, start with operational evidence and keep topology drift or multiple active connections in mind before editing code.
- If the user signals trust loss or says recovery is happening elsewhere (`grok is activly recovering from git`), stand down and stop touching the repo.

## General Tips
- `sudo -n` is often required for host s6 control and some D-Bus/service inspection on this machine.
- In `operation-dbus-proto`, validate local deploys with `sudo s6-svstat`, `busctl --system introspect`, `op-identity-sled`, and sometimes `sha256sum`; short HTTP probes and plain `cargo check` are not sufficient proof of runtime correctness.
- For runtime blob work, `opblob seal-shm`, `opblob catalog /dev/shm/opdbus/plugin-blobs`, and `opblob inspect /dev/shm/opdbus/plugin-blobs/<plugin>.*.blob` are the fast proof path; blob correctness and service bind correctness can fail independently.
- If a broad deploy touches `/etc/s6/sv/*`, compare `readlink -f` paths first to catch repo-to-system self-copy failures.
- `op-web` release builds in `operation-dbus-proto` now depend on `crates/op-web/ui/dist`, not `lovable/dist`; `cargo check -p op-web` plus a real UI asset build is the fast validation path when release embedding breaks.
- For plugin/UI capability audits in `operation-dbus-proto`, start with `zcall` and the live plugin method list before trusting README/UI labels or inferred integration.
- On this host, `~/.bash_profile` sources `~/.bashrc`, and `/usr/share/bash-completion/bash_completion` may be absent; use `skills/bash-startup-persistence/SKILL.md` for repeated Bash persistence or completion work.
- For Factory config, the truth lives in `~/.factory/settings.json`; validate with `jq empty` plus root-bound `customModels` checks, and use `skills/factory-custom-models/SKILL.md` for repeated BYOK/model-family edits.
- For repeated local repo deploys, use `skills/op-dbus-local-s6-deploy/SKILL.md`.
- For Oracle/WireGuard triage in this repo, start with `busctl --system tree org.opdbus.v1.plugins`, `wg show`, `ip -brief addr`, and local `oci` queries before assuming the wrong service/object name or a single-tunnel topology.
- On this Artix host, treat systemd-oriented package/service assumptions as conversion targets for s6/elogind.

## What's in Memory

### /home/jeremy/git/operation-dbus-proto

#### 2026-07-04

- standalone UI checkout and Gemma/zcall audit: operation-dashboard-ui-07, submodule, standalone checkout, lovable, realtime, zcall, gemma_brain, CatalogService/Subscribe
  - desc: Search first when the user wants `operation-dashboard-ui-07` split from the parent repo, asks how Lovable and Rust divide ownership, or wants a concrete audit of Gemma/chat/catalog promotion wiring across the UI and plugin bridge.
  - learnings: the UI is now a standalone repo at `/home/jeremy/git/operation-dashboard-ui-07`; `zcall` and `src/views/gemma.rs` prove real prompt/plugin wiring, but authoritative catalog subscribe/writeback is still partial
- blob-driven `zcall` wrapper and shell completion: zcall, /dev/shm/opdbus/plugin-blobs, complete -F _zcall zcall, COMPREPLY, /etc/bash_completion.d/zcall, /root/.bashrc
  - desc: Search first when the user wants a short plugin-call CLI, tab completion from blob metadata, or automatic `zcall` loading in user/root shells for this repo on this host.
  - learnings: the stable interaction shape is plugin -> method -> args, blob data is the default authority, and `/etc/bash_completion.d` alone was not enough because this host lacks active bash-completion auto-loading
- SHM blobstore materializer and handoff: opblob, seal-shm, /dev/shm/opdbus/plugin-blobs, opblob-shm-handoff.md, MethodDecl.name, Cannot assign requested address
  - desc: Search first when the user wants the blob runtime path made real, asks whether schema authority is actually sealed into blobs, or wants a handoff file written to disk; applies to `cwd=/home/jeremy/git/operation-dbus-proto`.
  - learnings: `opblob seal-shm` sealed 62 tmpfs-backed plugin blobs; method lookup must trust `MethodDecl.name`, and `op-dbus` bind-address failures are a separate issue from blob correctness
- plugin/schema migration review and honest workspace status: op-plugins, op-cognitive-mcp, identity.as_deref(), BlobStore, blob.manifest, current_schema_catalog_hash, cargo check --workspace
  - desc: Use for the active Factory plugin-schema mission, generated plugin-tree breakage, bridge/blob compatibility fixes, and post-reboot compiler/log verification expectations.
  - learnings: validate crate-by-crate (`op-plugins`, `op-cognitive-mcp`, `op-blob`/`op-grpc-bridge`) but do not claim workspace green until `cargo check --workspace` and warning review are rerun after reboot
- Oracle / OCI / WireGuard runtime triage: oci, wg show, org.opdbus.v1.plugins, netmaker DOWN, decoy-wg2, 129.153.134.63:51821, setup-wg-decoy.sh
  - desc: Search here when Oracle decoy ingress, multiple WG tunnels, or a moved `opdbus` interface may be part of the failure; routes to live OCI, bus, and tunnel evidence rather than code speculation.
  - learnings: start with `busctl --system tree org.opdbus.v1.plugins`, `wg show`, and `ip -brief addr`; in this run `opdbus` had a live Oracle handshake while `netmaker` was DOWN

#### 2026-07-03

- blob architecture verification, embedded schema deploy, and trust boundary: /dev/shm/plugin_schema.dat, OPBLOB01, schema_blob_bytes, no stubs or placeholders, LIVE_SCHEMA_PATH, FILE_DESCRIPTOR_SET
  - desc: Search first when work touches SHM blob/schema authority, blob-first readers, or disputed claims that a blob runtime path is already complete; applies to `cwd=/home/jeremy/git/operation-dbus-proto`.
  - learnings: embedded schema in the sled blob is now real and deployable, but older gemma4/zeroclaw "blob architecture complete" claims were not validated end to end; distinguish runnable from scaffolding before claiming success
- plugin-backed state mutation and live object introspection: ApplyContractMutation, org.opdbus.StateManager, /org/opdbus/v1/state, org.opdbus.v1.plugins, privacy_container.rs
  - desc: Search here when the user wants Incus/container or unix-socket changes done through the designed D-Bus/plugin path, or tells you to introspect the live object instead of patching app code.
  - learnings: the writable surface is `StateManager.ApplyContractMutation`; inspect the live plugin object first, and do not patch `op-web` if the user asked for the designed plugin method
- broad deploy script repair for blob-owning binaries: deploy.sh, --skip-network all, op-identity-sled, op-grpc-bridge, op-mcp-server, readlink -f
  - desc: Routes to the July 3 deploy-script fix and live deploy verification when the user explicitly wants the broad local deploy path instead of a narrow service restart.
  - learnings: the broad script only became safe after adding missing binaries and a self-copy guard; still verify live service names like `op-web-srv`
- workspace release builds and `op-web` Rust UI assets: cargo build --workspace --release, ~/bin, /usr/local/bin, ui/dist, lovable/dist, npm ci --legacy-peer-deps
  - desc: Use when a release build in `operation-dbus-proto` fails on missing UI assets or when the user wants binaries built/installed and the install destination needs to be stated precisely.
  - learnings: `op-web` is Rust-UI-first under `crates/op-web/ui`, and local convenience installs in this rollout went to `~/bin`, not the deploy-script destinations

### host shell configuration

#### 2026-07-04

- Bash startup persistence for exported env vars: ~/.bashrc, ~/.bash_profile, XAI_API_KEY, export, bash -lc, child environment
  - desc: Search here when the user says a shell setting was not made persistent, wants something "in my bash", or needs proof that an env var survives fresh login shells on this machine.
  - learnings: `~/.bash_profile` already sources `~/.bashrc`; use one exported assignment, verify in a fresh `bash -lc`, and do not print the secret

### /home/jeremy/git/operation-dashboard-ui-07

#### 2026-07-03

- headless `zeroclaw-wayland` probe on Artix s6: zeroclaw-wayland, /run/s6-rc/servicedirs/zeroclaw-wayland/down, /usr/bin/weston, WAYLAND_DISPLAY, XDG_SESSION_TYPE=tty
  - desc: Search here when the question is whether a Wayland display already exists for ZeroClaw on the headless Artix host, or whether the existing service is simply disabled; applies to `cwd=/home/jeremy/git/operation-dashboard-ui-07`.
  - learnings: the host already had a headless Weston service definition, but no live compositor/socket; the key signal was the `down` file in the s6-rc compiled state, not the presence of `s6-supervise`

### ~/.factory and desktop config

#### 2026-07-03

- Factory xAI BYOK Grok family edits: ~/.factory/settings.json, ~/.local/share/opencode/auth.json, ~/.cache/opencode/models.json, custom:Grok-4.3-[xAI-BYOK]-0, grok-build-0.1
  - desc: Search here when the user wants Grok/xAI added to Factory from local opencode state, or when "all models" really means the whole chat-capable provider family.
  - learnings: use targeted opencode auth/state/model-cache reads, add chat-capable `xai` entries only, and exclude non-chat `grok-imagine-*` endpoints from Factory `customModels`

#### 2026-06-28

- JetBrains Air defaults: .codex/config.toml, wire_api = responses, env_key, /usr/bin/cargo, /usr/bin/rustc
  - desc: Routes to persistent Air AI-provider defaults and system Rust toolchain wiring on this desktop.
  - learnings: this Air/Codex build wants root-level `model_provider` and `wire_api = "responses"`; leaving `env_key` in place broke BYOK token use

### Older Memory Topics

#### /home/jeremy/git/operation-dbus-proto

- schema-first projection, plugin schema migration, and Zeroclaw architecture: op-identity-sled, /dev/shm/plugin_schema.dat, SchemaEngine, MutationEngine, feat/sled-source-port-salt, mission.md
  - desc: Search here for live sled reading, explicit `unix:path=` bus discovery, plugin-schema mission review, generated `op-plugins` breakage, and the settled Zeroclaw ownership model; applies to `cwd=/home/jeremy/git/operation-dbus-proto`
- local deploy and live service control: deploy.sh, op-grpc-bridge-zeroclaw, projection_server, CARGO_BUILD_JOBS=1, cp -a same file, /run/service
  - desc: Use for mapping user-facing deploy requests onto real crate/bin/service tuples, choosing targeted install/restart over the broad script, and keeping install destinations exact; applies to `cwd=/home/jeremy/git/operation-dbus-proto`
- bridge, socket ownership, and Qdrant reachability: createunixsocket, /run/ghostbridge/container.sock, SearchSemanticTrace, FailedPrecondition
  - desc: Covers shared unix-socket ownership, projected `unix_socket` state, and separating bridge transport success from an unconfigured semantic shuttle; applies to `cwd=/home/jeremy/git/operation-dbus-proto`
- repo deployment on Artix s6 / headless Wayland for zeroclaw-gui: weston, wayvnc, org.opdbus.v1.S6.Systemctl, USER unbound variable
  - desc: Covers isolated GUI forwarding on this Artix host without disturbing CRD/X11, including the D-Bus-backed s6 control path; applies to `cwd=/home/jeremy/git/operation-dbus-proto`
- review zip creation and shorthand service requests: meta-ai-review-conversations-plugin-schema-bridge-20260625.zip, chrome-remote-desktop, just plugin and scema
  - desc: Use for small handoff/archive bundles and terse host-service requests like `crd`; applies to `cwd=/home/jeremy/git/operation-dbus-proto`

#### ~/.factory and desktop config

- Factory Missions OpenRouter routing and narrow registry edits: missionOrchestratorModel, missionModelSettings, custom:* IDs, openrouter/owl-alpha, minimax/minimax-m2.5
  - desc: Covers mission routing keys plus smaller `customModels` edits in `~/.factory/settings.json`; use when the task is Factory config, not repo code

#### Artix host package workflows

- s6/elogind package conversion and AUR fixes: paru -Suyy, /usr/local/bin/makepkg, autopatch-systemd-to-elogind.sh, incus-s6
  - desc: Covers the host-specific `makepkg` wrapper/hook issue, Incus s6 conversion, and the proven fix for `can't find package name in packagelist`; applies to this Artix machine
- microsoft-edge-canary s6 packaging: microsoft-edge-canary-updater-srv, microsoft-edge-canary-updater-log, s6-log -d3
  - desc: Use for package-local s6 conversion patterns where the payload ships updater helpers and the log half must match `notification-fd` semantics; applies to this Artix machine
- XanMod trust path on Artix: chaotic-keyring, pacman-key --populate chaotic, torvalds@kernel.org
  - desc: Covers distro-specific signing/keyring setup for XanMod binaries and AUR source verification on this Artix host
