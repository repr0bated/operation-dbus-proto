thread_id: 019f125d-9aab-70f3-8f52-fff59eb6c061
updated_at: 2026-06-29T08:32:11+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/06/29/rollout-2026-06-29T03-52-32-019f125d-9aab-70f3-8f52-fff59eb6c061.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: feat/sled-source-port-salt

# Deployment target mapping for plugins, gRPC bridge, and projection in `operation-dbus-proto`

Rollout context: The user asked to "deploy plugins,l grpc-bridge and projection" in `/home/jeremy/git/operation-dbus-proto`. The assistant first inspected repo layout and deploy conventions, then mapped the request to actual crates and s6 services. The user later clarified that the bridge target might be `op-grpc-bridge-zeroclaw`.

## Task 1: Identify deployable targets and run checks

Outcome: partial

Preference signals:
- The user asked for "deploy plugins,l grpc-bridge and projection" -> future agents should treat this as a request to locate the repo’s actual deployable crates/services rather than assume the names map 1:1.
- When the assistant noticed the ambiguity, the user replied "it might be op-grpc-bridge-zeroclaw?" -> future agents should actively check for variant binaries/service names when a service label is ambiguous, especially when there are multiple bridge binaries.

Key steps:
- Inspected workspace and deploy layout with `rg --files` and `sed` to find `crates/op-plugins`, `crates/op-grpc-bridge`, `crates/op-projection`, and matching `deploy/s6/*` service directories.
- Verified `crates/op-plugins` is a library crate, not a standalone service binary.
- Confirmed `crates/op-grpc-bridge/Cargo.toml` defines two binaries: `op-grpc-bridge` and `op-grpc-bridge-zeroclaw`.
- Confirmed `deploy/s6/op-grpc-bridge-zeroclaw/run` exists and launches `/usr/local/bin/op-grpc-bridge-zeroclaw`, while `deploy/s6/op-grpc-bridge/run` launches `/usr/local/bin/op-grpc-bridge`.
- Ran `cargo check -p op-plugins`, `cargo check -p op-grpc-bridge`, and `cargo check -p op-projection`; the initial parallel checks hit workspace lock contention, and the full `cargo check -p op-grpc-bridge` was later killed by SIGKILL while checking shared `op-plugins` dependencies.
- Reran the bridge check narrowly with `CARGO_BUILD_JOBS=1 cargo check -p op-grpc-bridge --bin op-grpc-bridge-zeroclaw`, which completed successfully.

Failures and how to do differently:
- The initial full bridge check was killed by SIGKILL during `op-plugins` compilation, likely due to memory pressure. The narrower single-job check avoided that.
- Running multiple `cargo check` jobs in parallel caused lock contention on the package cache/build directory; future similar validation should be serialized or run with a constrained job count.

Reusable knowledge:
- `op-plugins` is a library crate; deploying it means rebuilding dependent services, not installing a plugin binary.
- `op-grpc-bridge` exposes two binaries and the Zeroclaw variant has its own s6 service directory and depends on `op-plugins`.
- For this repo, `CARGO_BUILD_JOBS=1` is a useful fallback when a workspace check gets SIGKILLed during large dependency compilation.

References:
- [1] `crates/op-grpc-bridge/Cargo.toml` contains `[[bin]] name = "op-grpc-bridge"` and `[[bin]] name = "op-grpc-bridge-zeroclaw"`.
- [2] `deploy/s6/op-grpc-bridge-zeroclaw/run` ends with `exec /usr/local/bin/op-grpc-bridge-zeroclaw`.
- [3] Successful narrow check: `CARGO_BUILD_JOBS=1 cargo check -p op-grpc-bridge --bin op-grpc-bridge-zeroclaw` -> `Finished 'dev' profile ... target(s) in 1m 18s`.

## Task 2: Patch deployment script for Zeroclaw bridge target

Outcome: success

Preference signals:
- After the bridge ambiguity, the user effectively steered the target toward the Zeroclaw variant by asking about `op-grpc-bridge-zeroclaw` -> future agents should not force the generic bridge if the Zeroclaw service exists and matches the asked-for slice.

Key steps:
- Updated `deploy/deploy.sh` service list comment and `SERVICES` array to include `op-grpc-bridge:op-grpc-bridge-zeroclaw:op-grpc-bridge-zeroclaw`.
- Verified the edit with `bash -n deploy/deploy.sh` and `git diff -- deploy/deploy.sh`.

Failures and how to do differently:
- No syntax issues were introduced; the patch was intentionally narrow and only added the Zeroclaw bridge as a first-class deploy target.

Reusable knowledge:
- The deploy script’s service tuple format is `crate:binary:service`.
- `deploy/deploy.sh` already had `op-projection:projection_server:op-projection`; the bridge variant needed an explicit tuple to be deployable through the same path.

References:
- [1] Patch added `"op-grpc-bridge:op-grpc-bridge-zeroclaw:op-grpc-bridge-zeroclaw"` to `deploy/deploy.sh`.
- [2] `bash -n deploy/deploy.sh` succeeded.

## Task 3: Attempt live deployment of projection and Zeroclaw bridge

Outcome: partial

Preference signals:
- The user wanted deployment, not just inspection, so the workflow should proceed to install/restart only after validation passes.

Key steps:
- Attempted `CARGO_BUILD_JOBS=1 sudo -E ./deploy/deploy.sh --skip-network op-projection`.
- The script began installing system files and several s6 services, but failed on `cp -a` because `/etc/s6/sv/gbr-warp` resolved back into the repo checkout: `cp: '/home/jeremy/git/operation-dbus-proto/deploy/s6/gbr-warp//.' and '/etc/s6/sv/gbr-warp/.' are the same file`.
- Inspected links: `/etc/s6/sv/gbr-warp -> /home/jeremy/git/operation-dbus-proto/deploy/s6/gbr-warp`, while `/etc/s6/sv/op-projection` and `/etc/s6/sv/op-grpc-bridge-zeroclaw` were real directories.
- Confirmed `/usr/bin/s6-svc` exists and `/run/service/op-projection` plus `/run/service/op-grpc-bridge-zeroclaw` are present as symlinks into s6-rc servicedirs.
- Began a manual `CARGO_BUILD_JOBS=1 cargo build --release -p op-projection --bin projection_server` to work around the deploy-script self-copy issue and to reuse release artifacts for the bridge.
- The projection release build was still running when the user aborted the turn; no install/restart of either service was completed in this rollout.

Failures and how to do differently:
- The repo’s deploy script is not safe on this machine when an s6 service path in `/etc/s6/sv` points back into the repo checkout. Future agents should detect symlinked `/etc/s6/sv/*` paths before using the broad `cp -a` service installation path.
- Because the broad deploy path failed before the requested binaries were installed or restarted, a safer fallback is to build/install only the named binaries and restart only the matching services.

Reusable knowledge:
- `deploy/deploy.sh` currently copies every `deploy/s6/*` service into `/etc/s6/sv`, which can fail if `/etc/s6/sv/<service>` is itself a symlink into the repo.
- The live deployment path was not completed; projection and Zeroclaw bridge were still build-gated when the turn was aborted.

References:
- [1] Deploy failure: `cp: '/home/jeremy/git/operation-dbus-proto/deploy/s6/gbr-warp//.' and '/etc/s6/sv/gbr-warp/.' are the same file`.
- [2] `readlink -f /etc/s6/sv/gbr-warp /home/jeremy/git/operation-dbus-proto/deploy/s6/gbr-warp /etc/s6/sv/op-projection /etc/s6/sv/op-grpc-bridge-zeroclaw` showed `gbr-warp` resolves to the repo path, while the other two resolve to `/etc/s6/sv/...` directories.
- [3] Manual build command started: `CARGO_BUILD_JOBS=1 cargo build --release -p op-projection --bin projection_server`.

