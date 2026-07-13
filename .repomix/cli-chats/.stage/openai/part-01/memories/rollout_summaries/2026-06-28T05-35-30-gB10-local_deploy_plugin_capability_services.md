thread_id: 019f0cb9-cc11-7f63-9f39-80e612bb34a2
updated_at: 2026-06-28T09:56:55+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/06/28/rollout-2026-06-28T01-35-30-019f0cb9-cc11-7f63-9f39-80e612bb34a2.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: feat/sled-source-port-salt

# Local deploy of mission changes for the plugin-capability worktree

Rollout context: The user first said "deploy misson changes", then corrected it to "i meant deploy localloy", so the work shifted from pushing the branch to building/installing the changes locally on the machine and restarting the local services. The work was done in `/home/jeremy/git/operation-dbus-proto-wt-plugin-capability` while the branch was `plugin-capability`.

## Task 1: Clarify deploy target and identify the local service path
Outcome: success

Preference signals:
- The user corrected the request from "deploy misson changes" to "i meant deploy localloy" -> in similar cases, default to local install/restart unless the user explicitly asks to push a branch or create a PR.

Key steps:
- Checked repo deploy scripts and service layout rather than improvising.
- Identified that `deploy/deploy.sh` is broad and would touch unrelated services/network bootstrap, so it was not used for the local deploy.
- Read `deploy/install-op-openvswitch-daemon.sh`, `deploy/s6/op-dbus-mirror/run`, `deploy/s6/op-openvswitch-daemon/run`, and related service definitions to understand the right local restart path.
- Confirmed the live s6 service directories existed for `op-dbus-mirror` and `op-openvswitch-daemon`.

Failures and how to do differently:
- The initial interpretation as a GitHub deploy/branch push was wrong for the user’s actual intent; the user had to correct it.
- The broad deploy script was avoided because it included network bootstrap and stale service mappings.

Reusable knowledge:
- For this repo, local deployment of the hypervisor/mirror services is better handled by building release binaries and restarting the specific s6 services, not by running the full `deploy/deploy.sh` flow.
- `deploy/install-op-openvswitch-daemon.sh` is the targeted install script for the OVS daemon; `deploy/deploy.sh` is a broader system deployment script with a stale service list.

References:
- `deploy/install-op-openvswitch-daemon.sh` installs `/usr/local/bin/op-openvswitch-daemon`, writes D-Bus policy/service files, and restarts the daemon.
- `deploy/s6/op-dbus-mirror/run` runs `/usr/local/bin/ovs-dbus-init`.
- `deploy/s6/op-openvswitch-daemon/run` runs `/usr/local/bin/op-openvswitch-daemon --grpc 127.0.0.1:50051`.
- Existing live s6 service dirs: `/run/service/op-dbus-mirror`, `/run/service/op-openvswitch-daemon`.

## Task 2: Build, install, and restart the local services
Outcome: success

Key steps:
- Ran `cargo build --release -p op-dbus-mirror --bin ovs-dbus-init -p op-openvswitch-daemon --bin op-openvswitch-daemon`.
- The release build took about 13 minutes and completed successfully.
- Installed the built binaries with `sudo install -m 0755` to `/usr/local/bin/ovs-dbus-init` and `/usr/local/bin/op-openvswitch-daemon`.
- Restarted the live services with `sudo s6-svc -r /run/service/op-dbus-mirror` and `sudo s6-svc -r /run/service/op-openvswitch-daemon`.
- Verified the binaries on disk matched the release artifacts by SHA-256.
- Verified service presence and D-Bus ownership with `busctl` and `s6-svstat`.

Reusable knowledge:
- Release builds for these binaries are dependency-heavy and can take several minutes; this is normal and not a hang.
- The local deploy can be validated by comparing SHA-256 of `/usr/local/bin/*` against `target/release/*`.
- `busctl --system list` showed `org.opdbus.v1.mirror` and `org.opdbus.v1.plugins.ovsdb` after restart.
- `busctl --system introspect` on `org.opdbus.v1.mirror /org/opdbus/v1/mirror` returned the expected mirror-management interface and subpaths.
- `busctl --system introspect` on `org.opdbus.v1.plugins.ovsdb /org/opdbus/v1/plugins/ovsdb` showed methods like `Transact`, `SendMessage`, `RecvMessage`, `NewStream`, `NextId`, `Notify`, and status/queue methods.

Failures and how to do differently:
- `cargo build` emitted existing dead-code warnings for `op-openvswitch-daemon`, but no build error; they were already present in the binary’s code surface.
- `s6-svstat /run/service/op-dbus-mirror /run/service/op-openvswitch-daemon` without sudo hit a permission error, so service status checks should use `sudo`.
- A `curl -sf http://100.90.37.254:3003/health` probe hung and was cancelled; it did not block the local deploy, but it is not a reliable gate in this environment during the service restart window.

References:
- Build command: `cargo build --release -p op-dbus-mirror --bin ovs-dbus-init -p op-openvswitch-daemon --bin op-openvswitch-daemon`
- Install/restart command: `sudo install -m 0755 target/release/ovs-dbus-init /usr/local/bin/ovs-dbus-init && sudo install -m 0755 target/release/op-openvswitch-daemon /usr/local/bin/op-openvswitch-daemon && sudo s6-svc -r /run/service/op-dbus-mirror && sudo s6-svc -r /run/service/op-openvswitch-daemon`
- SHA-256 verification confirmed matching release artifacts.
- `s6-svstat` with sudo reported both services up: `op-dbus-mirror` pid `14128`, `op-openvswitch-daemon` pid `14129`.
- D-Bus names observed: `org.opdbus.v1.mirror`, `org.opdbus.v1.plugins.ovsdb`.

## Task 3: Final deploy verification and smoke test
Outcome: partial

Key steps:
- Confirmed the services were up under s6.
- Confirmed D-Bus introspection worked for both the mirror and OVSDB plugin paths.
- Tried a lightweight health smoke test against cognitive-MCP (`curl -sf http://100.90.37.254:3003/health`), but it hung and was cancelled.

Failures and how to do differently:
- The cognitive-MCP health check did not return within the short timeout used here; if this check is needed, it may require a longer timeout or should be run after service churn settles.
- For this local-deploy workflow, the D-Bus/s6 checks were the successful validation path; the health probe was not necessary to confirm the deploy.

Reusable knowledge:
- Local deploy success signal in this repo: release binaries installed, s6 services restarted, `busctl` introspection succeeds, and the service names appear on the system bus.
- If a health endpoint hangs during deploy verification, it is better to stop the probe than to hold the deploy open waiting on it.

References:
- `sudo s6-svstat /run/service/op-dbus-mirror /run/service/op-openvswitch-daemon` returned `up (pid 14128 ...)` and `up (pid 14129 ...)`.
- `busctl --system introspect org.opdbus.v1.mirror /org/opdbus/v1/mirror` showed `org.opdbus.MirrorV1` with `GetStats`, `ListPaths`, `PublishSnapshot`, and `Reconcile`.
- `busctl --system introspect org.opdbus.v1.plugins.ovsdb /org/opdbus/v1/plugins/ovsdb` showed the OVSDB plugin interface and methods.
- The cancelled smoke probe was `curl -sf http://100.90.37.254:3003/health`.

