# Stale Excerpts from xdp-afxdp-conversation-2026-05-23.md

**Reason for exclusion:** These sections contained reboot guidance and host AF_XDP cutover recommendations that contradict the current architecture (host AF_XDP is now forbidden).

---

## Excerpt 1: Reboot Guidance Given

**Original section:**

```markdown
## Reboot Guidance Given

The answer to "reboot?" was:

- Do not reboot yet if the patched binaries have not been installed.
- First run:

  ```sh
  deploy/op-xdp-wg/up
  ```

- After deploy installs the updated binaries and s6/LXC hook wiring, reboot is
  reasonable.
```

**Note:** Generic reboot guidance removed; reboot decisions should follow standard service deployment patterns.

---

## Excerpt 2: Current Practical Next Step (Host AF_XDP Cutover)

**Original section:**

```markdown
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
```

**Note:** This section described the host AF_XDP cutover plan (moving `eth0` to OVS AF_XDP bridge, moving public L3 to `ovsbr0`). The current architecture forbids host AF_XDP; this workflow is obsolete.

---

## Excerpt 3: Host AF_XDP Model from Key Findings

**Partial excerpt:**

```markdown
- `afxdp.md` describes the OVS AF_XDP model: physical NIC ports are added to
  an OVS userspace datapath with `type="afxdp"`, and host IP/routing must live
  on the OVS internal bridge interface rather than the physical NIC.
```

**Note:** While kept in the debugging patterns document for context, the expectation that this model would be applied on the host is now invalid.

---

**Summary:** Excluded sections focused on host-side AF_XDP bridge cutover and reboot orchestration that are no longer part of the architecture.
