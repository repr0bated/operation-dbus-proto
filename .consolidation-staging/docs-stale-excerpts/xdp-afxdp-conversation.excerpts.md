# Dropped Excerpts from xdp-afxdp-conversation-2026-05-23.md

These sections were dropped because they contain reboot guidance or host AF_XDP cutover recommendations that are forbidden per current architecture (CLAUDE.md).

---

## Reboot Guidance Given

The answer to "reboot?" was:

- Do not reboot yet if the patched binaries have not been installed.
- First run:

  ```sh
  deploy/op-xdp-wg/up
  ```

- After deploy installs the updated binaries and s6/LXC hook wiring, reboot is
  reasonable.

---

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
- **Public host L3 should move off `eth0` and onto `ovsbr0` with an `onlink`
  default route when the AF_XDP bridge cutover is active.** ← FORBIDDEN per CLAUDE.md

---

## Key Findings - Host AF_XDP Cutover Context

From the "Key Findings" section:

- `afxdp.md` describes the OVS AF_XDP model: physical NIC ports are added to
  an OVS userspace datapath with `type="afxdp"`, and **host IP/routing must live
  on the OVS internal bridge interface rather than the physical NIC.** ← FORBIDDEN per CLAUDE.md

---

## Runtime State Observed - AF_XDP Context

At the time of inspection:

- `xdp-loader status eth0` showed `xdp_dispatcher` with `xdp_steer`.
- `ovs-vsctl show` showed `ovsbr0` and `grpc-uplink`, but did not show `eth0`
  attached as an AF_XDP OVS port.
- `eth0` still had `148.113.204.83/32` and the default route.
- `ovsbr0` had private routing state, not the public management `/32`.

**That meant the live host was not actually running the intended AF_XDP-on-eth0
through OVS path yet.** ← Context for forbidden host AF_XDP architecture

---

## Bugs Identified - Item 5

5. The prior route failure was valid: moving a `/32` public default route to
   an OVS internal port requires `onlink`. ← Context for forbidden host AF_XDP architecture

---

<!-- Extracted stale content from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/operations/xdp-afxdp-conversation-2026-05-23.md on 2026-07-20 -->
