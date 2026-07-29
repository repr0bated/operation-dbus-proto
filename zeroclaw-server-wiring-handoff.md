# Zeroclaw Server Wiring — Handoff

_Saved 2026-07-29. Scope: wiring zeroclaw as the control plane on the **server (VPS)**._
_Local node/provider integration deliberately excluded (deferred)._

## Goal (unfinished)

Get the server's `zeroclaw` from "installed but idle" to a **running, configured control plane**
(providers set, daemon/gateway up). Scope of "wired" was **not** settled — see Open Questions.

## Access

- **VPS (mail server + zeroclaw):** `ssh mail-vps` → `admin@188.68.58.237` (key `~/.ssh/vps_key`, ED25519 `jeremy@3tched`).
- **Oracle decoy:** `ssh oracle-decoy` → `ubuntu@129.153.134.63` (same key). Mailbox nearly empty; has `~/bash_secrets` trove.
- Both SSH via port 22. IMAP 993/143 up on both; VPS also has SMTP 25/587.
- Aliases + key already installed in `~/.ssh/config` on this machine.

## Current server state (facts verified this session)

- `zeroclaw` **0.8.3** installed at `~/.cargo/bin/zeroclaw` on the VPS. Source repo at `~/zeroclaw`
  (GitHub `zeroclaw-labs/zeroclaw`, branch `master`, VPS pinned `f75b36a27`, origin/master ahead at `04af7b8`,
  working tree clean, submodule `docs/book/po`).
- **No persisted config** — `zeroclaw config list` shows only built-in defaults (`schema_version=3`).
  No `config.toml`; no `~/.config/zeroclaw` content. Config dir is overridable via `--config-dir` / `ZEROCLAW_CONFIG_DIR`.
- **No zeroclaw daemon/gateway running.** The binary is idle.
- **The live control plane is `op-grpc-bridge`** (PID seen: 1324) bound to `0.0.0.0:8090`. Per Jeremy: **"the bridge IS zeroclaw"** — this bridge (formerly *ghostbridge*, references since renamed to **`opdbus`**) is the zeroclaw runtime bridge.
- Control-plane chatbot provider `op-llm/src/openclaw.rs` (`OpenClawProvider`) is a plain HTTP client to
  `http://127.0.0.1:8090/v1/chat/completions` (models `openclaw:main`, `openclaw:gemini3-adc`). Default `OPENCLAW_BASE_URL=http://127.0.0.1:8090`.
- nginx `openclaw.3tched.com` conf: **gateway removed** → `503 "OpenClaw gateway disabled ... until zero-trust gRPC bridge is wired."`
- **Jeremy's conclusion (agreed):** the chatbot↔zeroclaw integration was **never actually wired in** — it's *greenfield*, not broken. `zeroclaw.rs` plugin only *publishes a schema*; no live dispatch path. Matches the old `AUDIT_REPORT` ("declared as router/enforcer, not implemented").

## Provider: Salad (the cloud provider)

- Jeremy: **Salad (SaladCloud, distributed-GPU inference) is already set up as the provider on the server.**
- In zeroclaw's model, Salad = an **OpenAiCompatible** provider (custom `base_url` → Salad container-gateway endpoint).
- **UNRESOLVED:** *where* Salad is actually configured. It is **NOT** in zeroclaw's config (which is empty).
  Likely in the odbus system, an env var / API key in `~/.bash_secrets`, or a config-dir not yet found.
  A recursive `grep salad ~/git/odbus` timed out (repo is 400 MB+) — narrow it next time
  (`grep -ri salad ~/.bash_secrets ~/.config ~/git/odbus/crates/*/src` or check env).
- There is a Salad one-time-login email in `jeremy@3tched.com` mailbox (account exists).

## Schema (why zeroclaw was chosen — it's schema-driven)

Canonical schema lives in the odbus plugin structs (schemars `JsonSchema`, `x-oscal-subid` annotations):
`~/git/odbus/crates/op-plugins/src/state_plugins/`
- `zeroclaw.rs` (97 KB) — `LlmTransport` (`dbus_object`, `grpc_target`, `incus_container`, `browser_surface`, `rest_aliases`, `policy_source`), `ModelAssignments` (`ovs_routing`, `obfuscation`, `vectorization`, `qdrant_retrieval`, `cozo_retrieval`). Plugin = "GB.Zeroclaw", "schema/RPC-native model router for Antigravity UI/CLI".
- `ghostbridge.rs` (→opdbus) — `BridgeIdentity` (wireguard_pubkey, tls_subject/sans/expires), `GhostrunnerSurface` (port default **8091**, bind `127.0.0.1`, env `GHOSTBRIDGE_UI_PORT`), `BridgeEndpoint`, `GhostbridgeState`.
- `ctl_plane_chatbot.rs` (29 KB) — the control-plane chatbot plugin (also references OVS bridges — don't confuse with the zeroclaw gRPC bridge).
- `zeroclaw config schema` dumps the full JSON Schema; `zeroclaw config list` lists live keys.

## Relevant zeroclaw CLI surface (0.8.3)

Subcommands incl: `agent`, `gateway` (webhooks/websockets), `acp` (JSON-RPC/stdio), `daemon`, `service`
(launchd/systemd user service), `status`, `providers`, `models`, `config`, `channel(s)`, `agents`, `plugin` (WASM), `migrate`.
Config groups seen: `gateway.allow_remote_admin`, `nodes.*` (federation — deferred), `observability.*`, `providers.*`.
Providers: 74 supported incl `ollama [local]`, `openrouter`, `anthropic`, `openai`, `gemini`, many OpenAiCompatible.
(Confirm `salad` is in the list / how it's addressed.)

## Incus containers on VPS (context)

`sudo incus list`: `NetMaker`, `cozo`, `mail-3tched`, `qdrant`, `xray` (10.200.0.1, binds `10.200.0.1:8090`), + two uuid-named containers. Storage pool `3tched-storage`. Mail server = Postfix/Dovecot in `mail-3tched` (`/var/mail/vhosts/3tched.com/{jeremy,admin,noreply}/Maildir`).

## Open questions to settle before touching production

1. **What "wired" means:**
   - (a) **Operational** — persist zeroclaw config (providers incl Salad), run it as a daemon/gateway service, verify. No new code.
   - (b) **Integration build** — make the chatbot dispatch *through* zeroclaw (point `:8090` / op-grpc-bridge at zeroclaw's gateway). Real Rust dev.
   - (c) **Config-only** — write config, leave running to Jeremy.
2. **Where Salad is configured** (see above) — determines if zeroclaw can already reach it or if that link is also unbuilt.

## Suggested next steps (server, operational reading)

1. Narrow-grep for Salad config/key (env + `.bash_secrets` + odbus src) — reconcile "already set up."
2. Decide scope (a/b/c) with Jeremy.
3. If (a): `zeroclaw config init` → set `providers.<salad>` (OpenAiCompatible base_url + key) → persist to a chosen `--config-dir`; enable via `zeroclaw service` (systemd user unit); `zeroclaw status`/`doctor` to verify.
4. Only after server is a real running control plane: revisit the (excluded) local node join.

## Cross-session notes / durable changes already made on THIS machine

- oo7-daemon replaced gnome-keyring as `org.freedesktop.secrets` (headless). Removed `gnome-keyring` + `cachyos-mangowc-dms` (settings pkg only; compositor intact). Stale keyrings at `~/.local/share/keyrings/*.gnome-bak`.
- Mailspring installed + logged into `jeremy@3tched.com`; old local inbox unrecoverable (home subvol never sent; nvme1n1 TRIM-wiped).
- Gnoppix AI config duplicated → `~/.config/gnoppix-ai/`, `~/.local/bin/gnoppix*`, desktop entries. `ollama` installed + active (no model pulled yet). Docker services deferred → `~/.config/gnoppix-ai/incus-deferred.md`.
