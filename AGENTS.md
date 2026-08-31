# Mandatory skill preload — every agent and model

At the start of every session in this repository, before analysis, diagnostics,
planning, edits, or delegated/subagent work, every LLM/agent must read these four
files completely:

- `.agents/skills/grpc-expert/SKILL.md`
- `.agents/skills/grpc-protocol-expert/SKILL.md`
- `.agents/skills/json-render/SKILL.md`
- `.agents/skills/ovs-db-analysis/SKILL.md`

This preload is mandatory even when the immediate task does not mention these
domains. Every newly spawned subagent and every model handoff must repeat it for
its own session; a parent agent's preload does not count for a child. Do not
claim a skill is loaded unless its `SKILL.md` was read completely. Load all four,
then apply only the guidance relevant to the task. For OP-DBUS PluginSchema,
generated plugin services, reflection, and seal/freeze/hot work,
`grpc-expert` takes precedence over the generic `grpc-protocol-expert`; use the
generic skill for protocol, channel, TLS, streaming, and observability concerns.

# Agent host-service policy

This host runs **runit** as PID 1. Agents must manage host services through
`sudo sv <command> <service>` — for example `sudo sv restart op-grpc-bridge`,
`sudo sv status op-web`. (`sv` needs root to read `supervise/ok`; an unprivileged
`sv status` fails with "access denied".)

Runit has no compiled service database — definitions are live:

- `/etc/runit/sv/<service>/run` is the service definition.
- A service is enabled by a symlink in `/etc/runit/runsvdir/default`
  (`current -> default`).
- `runsvdir -P /run/runit/service` supervises the enabled set.

So the whole update path is: edit the `run` script (or install a new binary),
then `sudo sv restart <service>`.

Do not edit `/run/runit/service` directly — that is the supervisor's runtime
view, not the source of truth. Do not invoke `runsv` or `runsvdir` by hand;
those belong to boot and to explicit human console recovery.

Deployment is **btrfs send/receive** of subvolume snapshots, not a copy script:
the layout in `deploy/btrfs-layout.sh` defines base/modules/snapshots/staging
subvolumes, and a release is a snapshot that is sent to the target. Do not
hand-copy binaries onto a running host as a deployment step.

One script publishes a release both ways from a single build:

```sh
CXXFLAGS="-include cstdint" cargo build --workspace --release
sudo deploy/runit/build-golden.sh          # golden subvolume + live install
sudo deploy/runit/build-golden.sh --dry-run   # review first
```

`--golden-only` skips the running host; `--live-only` skips the subvolume.
Network-critical services (OVS, uplink, DHCP, session bus) are never
auto-restarted — the script reports them for deliberate console action.

s6 is legacy on this host. `service6`, `s6-rc`, `s6-*` and the `/run/service`
tree no longer apply; treat any doc or script that still assumes them as stale.
Do not use `systemctl` or other foreign service-manager CLIs.

Container and application deployment must use D-Bus through `busctl` for
service-manager operations. Do not deploy service lifecycle calls through
`systemctl` or other service-manager CLIs. Host services remain governed by
the `sudo sv ...` rule above.

# Xray configuration policy — mandatory

**XRAY'S LIVE CONFIGURATION MUST EXIST ONLY AT
`/etc/xray/xray_config.json` inside the container.** Never point Xray at `/dev/shm/xray_config.json`,
`/usr/local/etc/xray/config.json`, or another disk-backed live path.

Until model-generated dynamic tag routing is implemented, the static bootstrap
configuration is correct and must be materialized into the container path during
boot. Later, the validated model/control-plane generator replaces that same file
atomically and reloads Xray through D-Bus. Models must not write or reload
Xray directly.
