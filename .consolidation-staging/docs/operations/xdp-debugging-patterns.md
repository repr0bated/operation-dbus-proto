# XDP Debugging Patterns

**WARNING: Host AF_XDP is now forbidden per current architecture. These patterns apply only to container-side XDP and debugging XDP program coexistence.**

## XDP Program Coexistence Debugging

### Key Principles

- Custom XDP programs and OVS AF_XDP must coexist through the libxdp dispatcher.
- Loading or unloading independent XDP programs blindly on the same interface can overwrite or remove OVS's AF_XDP redirect path.
- `xdp-dispatch.md` (historical reference) documented that coexistence requires using the libxdp dispatcher and avoiding `--all` unload operations.

### Runtime State Inspection

When debugging XDP state, check:

```sh
xdp-loader status eth0      # Shows dispatcher and loaded programs
ovs-vsctl show               # Shows OVS bridge and port configuration
ip -4 addr show dev eth0     # Verify interface addressing
ip route show                # Check routing state
```

Expected observations for proper coexistence:
- `xdp-loader status` should show `xdp_dispatcher` with multiple programs listed
- Each program should have a distinct priority and program ID
- OVS AF_XDP ports (when used in containers) should appear in `ovs-vsctl show`

## Targeted `xdp-loader unload` Pattern

### The Problem

Early implementations used:

```sh
xdp-loader unload eth0 --all
```

This removes **all** XDP programs from the interface, including any OVS AF_XDP redirect programs, breaking coexistence.

### The Solution

Unload only your specific program by ID:

```rust
fn unload_own_xdp_program(ifname: &str) -> Result<()> {
    // Parse xdp-loader status output
    // Find the specific program by name (e.g., "xdp_steer")
    // Extract its program ID
    // Unload only that ID:
    //   xdp-loader unload eth0 --id <id>
}
```

### Implementation in `op-xdp-wg`

The corrected implementation:

1. **Load with explicit metadata:**

   ```sh
   xdp-loader load eth0 /tmp/op-xdp-wg.o \
     --mode native \
     --prio 50 \
     --actions XDP_PASS \
     --prog-name xdp_steer
   ```

2. **Unload by program name/ID:**
   - Parse `xdp-loader status eth0` output
   - Find the `xdp_steer` entry
   - Extract its program ID
   - Run: `xdp-loader unload eth0 --id <id>`

This ensures the custom XDP program can be cleanly removed without disturbing other programs on the interface.

## `op-ovsbr0-afxdp` Wiring

### Binary Declaration

File: `crates/op-network/Cargo.toml`

Required entry:

```toml
[[bin]]
name = "op-ovsbr0-afxdp"
path = "src/bin/op-ovsbr0-afxdp.rs"
```

**Note:** This binary exists for historical/debugging reference. The current architecture does not use host-side AF_XDP bridging.

### Deploy Script Integration

File: `deploy/op-xdp-wg/up`

Build and install steps:

```sh
cargo build --release -p op-network --bin op-xdp-wg --manifest-path "$REPO_DIR/Cargo.toml"
cargo build --release -p op-network --bin op-ovsbr0-afxdp --manifest-path "$REPO_DIR/Cargo.toml"

install -m 755 "$REPO_DIR/target/release/op-xdp-wg" /usr/local/sbin/op-xdp-wg
install -m 755 "$REPO_DIR/target/release/op-ovsbr0-afxdp" /usr/local/sbin/op-ovsbr0-afxdp
```

## Verification Pattern

After deploying XDP changes, verify with:

```sh
cargo check -p op-network --bin op-xdp-wg --bin op-ovsbr0-afxdp --manifest-path Cargo.toml
bash -n deploy/op-xdp-wg/up
xdp-loader status eth0
```

Expected results:
- Both binaries check successfully
- Deploy script passes shell syntax validation
- `xdp-loader status` shows the custom program coexisting under the dispatcher

## Historical Bug: `xdp-loader unload --all`

**Bug identified (fixed):**

`op-xdp-wg hostside` and `detach` commands originally used:

```sh
xdp-loader unload eth0 --all
```

This violated the libxdp dispatcher coexistence model by removing all XDP programs, including any OVS AF_XDP redirect programs.

**Fix applied:**

Replaced with targeted removal using program ID lookup, preserving other XDP programs on the interface.

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/operations/xdp-afxdp-conversation-2026-05-23.md on 2026-07-20 -->
