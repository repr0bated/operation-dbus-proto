> ⚠️ **Architecture Warning**
>
> Host AF_XDP is now forbidden per current architecture; host WireGuard and host AF_XDP are architecture violations. The XDP debugging patterns below are preserved only for reference. Do not apply any host AF_XDP cutover or move host L3 onto an OVS bridge.

# XDP Program Coexistence and Targeted Unload Patterns

This note preserves XDP debugging patterns from the conversation around `xdp-loader`, `ovsbr0`, and the `wg-xray` XDP path.

## Initial Request

The requested source material was:

- `afxdp.md`
- `xdp-dispatch.md`
- `claudes-xdp-trial.txt`

The goal was to read those files carefully, infer why AF_XDP on `eth0` was breaking connectivity, and fix the code path rather than disabling AF_XDP.

## Key Findings

- `xdp-dispatch.md` says custom XDP programs and OVS AF_XDP must coexist through the libxdp dispatcher. Loading or unloading independent XDP programs blindly on the same interface can overwrite or remove OVS's AF_XDP redirect path.
- `claudes-xdp-trial.txt` showed a prior route failure where `op-ovsbr0-afxdp` tried to add the default route without `onlink`, causing `Network is unreachable` with a `/32` public address.

## Bugs Identified

1. `op-xdp-wg hostside` said it used the libxdp dispatcher, but it ran:

   ```sh
   xdp-loader unload eth0 --all
   ```

   That can remove OVS's own AF_XDP XDP program and contradicts `xdp-dispatch.md`.

2. `op-xdp-wg detach` had the same `--all` unload behavior.

3. `op-ovsbr0-afxdp.rs` existed in `crates/op-network/src/bin/`, but it was not declared in `crates/op-network/Cargo.toml`.

4. `deploy/op-xdp-wg/up` built and installed only `op-xdp-wg`; it did not build or install `op-ovsbr0-afxdp`.

## Changes Made

### `op-xdp-wg`

File: `crates/op-network/src/bin/op-xdp-wg.rs`

Changes:

- Added explicit XDP program metadata:
  - `XDP_PROG_NAME = "xdp_steer"`
  - `XDP_PRIO = "50"`
- Replaced `xdp-loader unload eth0 --all` in `hostside` with targeted removal of only the existing `xdp_steer` program.
- Loaded the program with:

  ```sh
  xdp-loader load eth0 /tmp/op-xdp-wg.o --mode native --prio 50 --actions XDP_PASS --prog-name xdp_steer
  ```

- Replaced `detach` cleanup with the same targeted removal logic, so detach no longer removes all XDP programs from `eth0`.
- Removed unused constants `CT_GW_IPV6` and `CT_IPV4`.

Important helper added:

```rust
fn unload_own_xdp_program(ifname: &str) -> Result<()>
```

This helper parses `xdp-loader status <ifname>`, finds the `xdp_steer` entry, extracts its program ID, and unloads only that program with:

```sh
xdp-loader unload eth0 --id <id>
```

### `op-network` Cargo Wiring

File: `crates/op-network/Cargo.toml`

Change:

```toml
[[bin]]
name = "op-ovsbr0-afxdp"
path = "src/bin/op-ovsbr0-afxdp.rs"
```

### Deploy Script

File: `deploy/op-xdp-wg/up`

Added release build for `op-ovsbr0-afxdp` and installation of `/usr/local/sbin/op-ovsbr0-afxdp`.

Relevant build/install steps now include:

```sh
cargo build --release -p op-network --bin op-xdp-wg --manifest-path "$REPO_DIR/Cargo.toml"
cargo build --release -p op-network --bin op-ovsbr0-afxdp --manifest-path "$REPO_DIR/Cargo.toml"

install -m 755 "$REPO_DIR/target/release/op-xdp-wg" /usr/local/sbin/op-xdp-wg
install -m 755 "$REPO_DIR/target/release/op-ovsbr0-afxdp" /usr/local/sbin/op-ovsbr0-afxdp
```

## Verification Run

Commands:

```sh
cargo check -p op-network --bin op-xdp-wg --bin op-ovsbr0-afxdp --manifest-path Cargo.toml
bash -n deploy/op-xdp-wg/up
cargo check -p op-network --bin op-xdp-wg --manifest-path Cargo.toml
```

Results:

- Both Rust binaries checked successfully.
- `deploy/op-xdp-wg/up` passed shell syntax validation.
- After removing unused constants, `op-xdp-wg` checked cleanly with no warnings.

<!-- Extracted from xdp-afxdp-conversation-2026-05-23.md on 2026-07-20 -->
