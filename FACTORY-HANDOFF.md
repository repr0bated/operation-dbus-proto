# Factory Handoff — GhostBridge Live! consolidation + implementation

Pick up from here. Companion: `IMPLEMENTATION-LOG.md` (design detail + running progress —
update it as you go so a rate-limit cut resumes cleanly). Mode: **don't claim done without a
live test + pasted output.** Conserve tokens.

## Hard architecture rules (do NOT violate)
1. **Only `/org/opdbus/v1/plugins/<plugin>` is projectable.** NEVER register a rogue D-Bus
   name like `opdbus.v1.Xray` / `/opdbus/v1/xray`. Control goes through state plugins.
2. **shm-reactive, no watch/poll/query/index.** `/dev/shm` is authoritative present-state;
   components READ it; xray greets connections (event), no polling loops. Sled =
   `/dev/shm/plugin_schema.dat`; xray dyn config = `/dev/shm/xray-ghostbridge.json`.
3. **Host = s6 (NOT systemd).** Inside containers = Debian/Alpine systemd. Moving a service
   to host means an s6 service def in `/etc/s6/sv/<name>/`.
4. **No unfounded generalizations.** Scope claims to what's verified.
5. **Secrets** never in unit files / never echoed — `/etc/ghostbridge/*.env` + bash_secrets.

## Verified-live baseline (do NOT redo)
- netmaker up (REST :8081, no-NIC, unix sockets, btrfs vols); cognitive-mcp up (queries answer);
  qdrant 839k vectors; cozo backend.
- op-grpc-adapters live (Netmaker/Mq/Mail + tonic-web + reflection + ghostbridge identity gate),
  serving `/run/op-grpc-adapters.sock`.
- op-mcp-shim built. Storage: array btrfs pool exists. /run/netmaker via tmpfiles.d.
- OpenFlow controller already host-native (op-openvswitch-daemon + s6 ovs* services).
- Wedge cleared earlier by SIGKILL of incusd (fresh respawn) — `/run/netmaker` tmpfiles fix
  prevents recurrence.

## DONE this session (verify, don't redo)
- **T2 code written** (architecture fix — retire rogue `opdbus.v1.Xray`):
  - `crates/op-identity/src/schema_bridge.rs`: replaced `start_xray_via_dbus()` (rogue D-Bus)
    with `reload_xray()` (host SIGHUP); removed `use zbus::{Connection, Proxy}`.
    **`cargo check -p op-identity` = PASS (28s).**
  - `crates/op-plugins/src/state_plugins/xray.rs`: `apply_state` now reloads/stops xray
    (host pkill -HUP); added `running: bool` + `xray_running()`; `query_current_state` reports it.
    **`cargo check -p op-plugins` = NOT YET RUN (interrupted) — RUN IT FIRST.**

## CONSOLIDATION runbook (move wg-xray container → host; ops = low effort)
Container `wg-xray` only existed for the old external/gateway addr (gone). Collapse to host.
1. **Copy from wg-xray → host** (preserve, no btrfs send/receive needed):
   - `incus file pull wg-xray/usr/local/etc/xray/config.json /usr/local/etc/xray/config.json`
   - `/etc/ssl/xray/{3tched.com,ghostbridge.tech}.{pem,key}` → host `/etc/ssl/xray/`
2. **zeroclaw → host**: binary `wg-xray:/root/.cargo/bin/zeroclaw` (check glibc compat
   Debian→Artix; if not, `cargo install` on host). s6 service `zeroclaw`: `zeroclaw gateway start`,
   env `ZEROCLAW_GATEWAY_HOST=0.0.0.0 ZEROCLAW_GATEWAY_PORT=8090`, `FACTORY_API_KEY` →
   `/etc/ghostbridge/zeroclaw.env` (rotate the key — it was plaintext in the container unit).
3. **xray → host s6**: fix `gbr-xray` to run `xray run -config /dev/shm/xray-ghostbridge.json`
   (dynamic) — falls back to `/usr/local/etc/xray/config.json` (static) until T3 cutover.
4. **Network**: alias `10.200.0.1` on host (grpc-uplink). Host already has 10.0.0.2 (opdbus) +
   10.200.0.2 (ovsbr0). Do this carefully — don't drop the chrome-remote-desktop session.
5. **Retire openclaw**: disable assistant `openclaw.service` (Node `/usr/bin/openclaw gateway run`).
   Rewire xray config outbound `assistant-openclaw-out` → host zeroclaw `127.0.0.1:8090`.
6. **Kill duplicates + decommission**: container xray(181)+zeroclaw, stray host xray(3599);
   then `incus stop wg-xray` and remove. Sled stays host /dev/shm (no container mount).

## Remaining tasks (ordered)
- **T1** Extract xray REALITY secrets from the working static config (the REALITY private key,
  vless uuid, short id, nextdns profile id are inside `/usr/local/etc/xray/config.json`) →
  `/etc/ghostbridge/xray.env`. The shuttle `run_schema_shuttle` needs
  `XRAY_UUID/XRAY_PRIVATE_KEY/XRAY_SHORT_ID/NEXTDNS_PROFILE_ID`.
- **T3** Dynamic xray cutover: run `run_schema_shuttle` → writes `/dev/shm/xray-ghostbridge.json`
  → diff vs static → host xray reads shm config → `pkill -HUP xray`. Rollback = point gbr-xray
  back at the static config. **This is the "subdomain forwarding" gap — it was built+staged but
  NEVER cut over.** Verify a real authenticated WG connection routes correctly.
- **T4** DNS split-horizon: fix the `nextdns-srv` s6 service (it writes resolv.conf=1.1.1.1) to
  point at the local NextDNS listener; internal `*.ghostbridge.tech` resolve local, decoy stays
  decoy. NextDNS for everything we don't own.
- **T5** Gemma (NOT deployed anywhere) — fresh host service: routing brain = subid classification
  (`src/prj/sch/mut/obs/evt/exp`, see `oscal-subids-report.md`) + tag routing + subdomain map,
  feeding the shuttle's generated config.
- **T6** Key-derivation/registration: `session_id` is currently a random `Uuid::new_v4()` in
  `crates/op-identity/src/session.rs:99` — make it the DERIVED key (per
  `project_key_derivation` memory: design = derived-key stored user-machine-only, real key in DB,
  blake3(psk), magic-link signup, MAC provision-time only). Identity vault + user store.
- **T7** zeroclaw fully replaces openclaw (T-runbook step 5); chatbot/UI plugin; accountability loop.
- **T8** Voyage vectorization correctness — re-chunk repomix (current chunks are line-windows over
  a giant `content-repomix.md`, semantically weak).

## Verification gates (paste output, don't assert)
- `cargo check -p op-plugins` clean before touching anything else.
- After cutover: real WG client → cognitive-mcp/netmaker through xray returns 200 (the
  authenticated path, not a forged footprint).
- xray `running: true` via the xray plugin at `/org/opdbus/v1/plugins/xray`.
