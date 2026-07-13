---
name: op-dbus-local-s6-deploy
description: Use when the user wants a local deploy/restart in operation-dbus-proto, especially for named binaries/services like op-grpc-bridge-zeroclaw, op-projection, op-dbus-mirror, or op-openvswitch-daemon.
argument-hint: "[crate/bin/service targets]"
disable-model-invocation: true
user-invocable: false
allowed-tools:
  - Read
  - Grep
  - Bash
---

# op-dbus local s6 deploy

## When to use

Use this when the task is "deploy locally", "restart the service", "deploy plugins/grpc-bridge/projection", or similar in `/home/jeremy/git/operation-dbus-proto` or a closely related worktree.

Do not use this for:
- branch push / PR workflows
- broad host package conversion
- pure spec/design work with no local build/install/restart

## Inputs / context to gather

1. Confirm the real working tree and branch.
2. Confirm whether the user wants a deploy-script install into `/usr/local/bin` or an ad hoc local convenience install such as `~/bin`, and plan to report the exact destination.
3. Identify the real deployable targets:
   - inspect `Cargo.toml` / crate bins
   - inspect `deploy/s6/<service>/run`
   - remember `op-plugins` is a library crate, not a deployable binary
4. Check whether the user wants a broad deploy or a targeted local restart.
5. Check for host-control constraints:
   - `sudo -n` availability
   - `/run/service/<service>` permissions
   - `/etc/s6/sv/<service>` symlinks back into the repo

## Procedure

1. Map the request to actual `crate:binary:service` targets.
   - If the name is ambiguous, inspect the repo rather than guessing.
   - Example: `grpc-bridge` may mean `op-grpc-bridge-zeroclaw`.
2. Prefer targeted local deploy over broad `deploy/deploy.sh` unless the user explicitly wants the broad path.
3. Run narrow validation checks first.
   - `cargo check -p <crate>`
   - If memory pressure or workspace contention appears, retry with:
   - `CARGO_BUILD_JOBS=1 cargo check -p <crate> --bin <bin>`
4. Build the exact release binaries you need.
   - Example:
   - `cargo build --release -p op-dbus-mirror --bin ovs-dbus-init -p op-openvswitch-daemon --bin op-openvswitch-daemon`
5. Install only the target binaries, and keep the destination explicit in your report.
   - Deploy path example: `sudo install -m 0755 target/release/<bin> /usr/local/bin/<bin>`
   - Local convenience path example: `install -m 0755 target/release/<bin> "$HOME/bin/<bin>"`
   - If installing many local executables from `target/release`, filter executable files and exclude `.d`, `.rlib`, and `.rmeta`.
6. Restart only the matching live services when the request is a service deploy.
   - `sudo s6-svc -r /run/service/<service>`
   - If the task is a persistent host service enable/start, use `sudo s6-rc -u change <service>` when appropriate.
7. Verify with machine-local evidence.
   - `sudo s6-svstat /run/service/<service>`
   - `busctl --system list`
   - `busctl --system introspect <bus-name> <object-path>` when the service should own D-Bus objects
   - optional: `sha256sum /usr/local/bin/<bin> target/release/<bin>`

## Efficiency plan

1. Read only the specific crate and `deploy/s6/<service>/run` files for the requested targets.
2. Serialize `cargo check` calls when lock contention is likely.
3. Use `CARGO_BUILD_JOBS=1` quickly if a full check gets SIGKILLed or shared dependency compilation is too heavy.
4. Skip broad deploy scripts if a targeted build/install/restart will satisfy the request.
5. Stop once install/restart verification succeeds; do not hold the run open on a flaky auxiliary health probe.

## Pitfalls and fixes

- Symptom: `cp -a ... are the same file` from `deploy/deploy.sh`
  - Likely cause: `/etc/s6/sv/<service>` is a symlink back into the repo checkout.
  - Fix: detect with `readlink -f` and bypass the broad script; deploy only the requested binaries/services.

- Symptom: `s6-svstat` or `s6-svc` returns `Permission denied`
  - Likely cause: host service dirs require privilege.
  - Fix: switch to `sudo -n` immediately.

- Symptom: `cargo check` gets SIGKILLed or hangs in heavy shared deps
  - Likely cause: memory pressure or too much parallelism.
  - Fix: use `CARGO_BUILD_JOBS=1` and narrow to the specific `--bin` target.

- Symptom: the user pushes back on where binaries were installed
  - Likely cause: the run mixed a local `~/bin` convenience install with the repo deploy-script destinations.
  - Fix: state the exact destination for each install and distinguish ad hoc local installs from `/usr/local/bin`, `/usr/local/sbin`, or `/opt/op-dbus/bin` deploy-script paths.

- Symptom: health HTTP probe hangs after restart
  - Likely cause: the probe is not the best readiness gate during service churn.
  - Fix: rely on `s6-svstat`, D-Bus ownership, and targeted introspection first.

- Symptom: shorthand service request like `crd`
  - Likely cause: the user expects you to resolve the real service name.
  - Fix: search `deploy/s6`, `/etc/s6/sv`, and `/run/service` for likely names, then operate on the discovered service.

## Verification checklist

- The requested binary or binaries built successfully.
- The installed target paths were updated, whether that was `/usr/local/bin/<bin>` or a local convenience path such as `~/bin/<bin>`.
- The intended `/run/service/<service>` entries report `up` with `sudo s6-svstat`.
- If the service owns D-Bus objects, `busctl --system list` / `introspect` shows them after restart.
- No broad deploy-script failure remains in the path actually used.

## Minimal example

For a local OVS deploy:

```bash
cargo build --release -p op-dbus-mirror --bin ovs-dbus-init -p op-openvswitch-daemon --bin op-openvswitch-daemon
sudo install -m 0755 target/release/ovs-dbus-init /usr/local/bin/ovs-dbus-init
sudo install -m 0755 target/release/op-openvswitch-daemon /usr/local/bin/op-openvswitch-daemon
sudo s6-svc -r /run/service/op-dbus-mirror
sudo s6-svc -r /run/service/op-openvswitch-daemon
sudo s6-svstat /run/service/op-dbus-mirror /run/service/op-openvswitch-daemon
```
