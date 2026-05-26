# XDP/AF_XDP Conversation Notes - 2026-05-23

This note preserves the actionable contents of the Codex conversation around
AF_XDP, `xdp-loader`, `ovsbr0`, and the `wg-xray` XDP path.

## Initial Request

The requested source material was:

- `afxdp.md`
- `xdp-dispatch.md`
- `claudes-xdp-trial.txt`

The goal was to read those files carefully, infer why AF_XDP on `eth0` was
breaking connectivity, and fix the code path rather than disabling AF_XDP.

## Key Findings

- `afxdp.md` describes the OVS AF_XDP model: physical NIC ports are added to
  an OVS userspace datapath with `type="afxdp"`, and host IP/routing must live
  on the OVS internal bridge interface rather than the physical NIC.
- `xdp-dispatch.md` says custom XDP programs and OVS AF_XDP must coexist through
  the libxdp dispatcher. Loading or unloading independent XDP programs blindly
  on the same interface can overwrite or remove OVS's AF_XDP redirect path.
- `claudes-xdp-trial.txt` showed a prior failure where `op-ovsbr0-afxdp` tried
  to add the default route through `ovsbr0` without `onlink`, causing
  `Network is unreachable` with a `/32` public address.

## Runtime State Observed

At the time of inspection:

- `xdp-loader status eth0` showed `xdp_dispatcher` with `xdp_steer`.
- `ovs-vsctl show` showed `ovsbr0` and `grpc-uplink`, but did not show `eth0`
  attached as an AF_XDP OVS port.
- `eth0` still had `148.113.204.83/32` and the default route.
- `ovsbr0` had private routing state, not the public management `/32`.

That meant the live host was not actually running the intended AF_XDP-on-eth0
through OVS path yet.

## Bugs Identified

1. `op-xdp-wg hostside` said it used the libxdp dispatcher, but it ran:

   ```sh
   xdp-loader unload eth0 --all
   ```

   That can remove OVS's own AF_XDP XDP program and contradicts
   `xdp-dispatch.md`.

2. `op-xdp-wg detach` had the same `--all` unload behavior.

3. `op-ovsbr0-afxdp.rs` existed in `crates/op-network/src/bin/`, but it was not
   declared in `crates/op-network/Cargo.toml`.

4. `deploy/op-xdp-wg/up` built and installed only `op-xdp-wg`; it did not build
   or install `op-ovsbr0-afxdp`.

5. The prior route failure was valid: moving a `/32` public default route to
   an OVS internal port requires `onlink`.

## Changes Made

### `op-xdp-wg`

File:

- `crates/op-network/src/bin/op-xdp-wg.rs`

Changes:

- Added explicit XDP program metadata:
  - `XDP_PROG_NAME = "xdp_steer"`
  - `XDP_PRIO = "50"`
- Replaced `xdp-loader unload eth0 --all` in `hostside` with targeted removal
  of only the existing `xdp_steer` program.
- Loaded the program with:

  ```sh
  xdp-loader load eth0 /tmp/op-xdp-wg.o --mode native --prio 50 --actions XDP_PASS --prog-name xdp_steer
  ```

- Replaced `detach` cleanup with the same targeted removal logic, so detach no
  longer removes all XDP programs from `eth0`.
- Removed unused constants `CT_GW_IPV6` and `CT_IPV4`.

Important helper added:

```rust
fn unload_own_xdp_program(ifname: &str) -> Result<()>
```

This helper parses `xdp-loader status <ifname>`, finds the `xdp_steer` entry,
extracts its program ID, and unloads only that program with:

```sh
xdp-loader unload eth0 --id <id>
```

### `op-network` Cargo Wiring

File:

- `crates/op-network/Cargo.toml`

Change:

```toml
[[bin]]
name = "op-ovsbr0-afxdp"
path = "src/bin/op-ovsbr0-afxdp.rs"
```

### Deploy Script

File:

- `deploy/op-xdp-wg/up`

Changes:

- Added release build for `op-ovsbr0-afxdp`.
- Installed `/usr/local/sbin/op-ovsbr0-afxdp`.

Relevant build/install steps now include:

```sh
cargo build --release -p op-network --bin op-xdp-wg --manifest-path "$REPO_DIR/Cargo.toml"
cargo build --release -p op-network --bin op-ovsbr0-afxdp --manifest-path "$REPO_DIR/Cargo.toml"

install -m 755 "$REPO_DIR/target/release/op-xdp-wg" /usr/local/sbin/op-xdp-wg
install -m 755 "$REPO_DIR/target/release/op-ovsbr0-afxdp" /usr/local/sbin/op-ovsbr0-afxdp
```

## Verification Run

Commands run:

```sh
cargo check -p op-network --bin op-xdp-wg --bin op-ovsbr0-afxdp --manifest-path Cargo.toml
bash -n deploy/op-xdp-wg/up
cargo check -p op-network --bin op-xdp-wg --manifest-path Cargo.toml
```

Results:

- Both Rust binaries checked successfully.
- `deploy/op-xdp-wg/up` passed shell syntax validation.
- After removing unused constants, `op-xdp-wg` checked cleanly with no warnings.

## Reboot Guidance Given

The answer to "reboot?" was:

- Do not reboot yet if the patched binaries have not been installed.
- First run:

  ```sh
  deploy/op-xdp-wg/up
  ```

- After deploy installs the updated binaries and s6/LXC hook wiring, reboot is
  reasonable.

## Current Practical Next Step

Run the deploy script, then reboot:

```sh
deploy/op-xdp-wg/up
reboot
```

After boot, verify:

```sh
xdp-loader status eth0
ovs-vsctl show
ip -4 addr show dev eth0
ip -4 addr show dev ovsbr0
ip route show
```

Expected direction:

- The custom `xdp_steer` program should coexist under the libxdp dispatcher.
- `eth0` should be attached to `ovsbr0` as an OVS AF_XDP port when
  `op-ovsbr0-afxdp` is actually invoked by the service path.
- Public host L3 should move off `eth0` and onto `ovsbr0` with an `onlink`
  default route when the AF_XDP bridge cutover is active.

