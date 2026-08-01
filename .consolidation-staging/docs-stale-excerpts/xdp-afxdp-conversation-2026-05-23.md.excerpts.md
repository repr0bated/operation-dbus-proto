# Stale Excerpts from xdp-afxdp-conversation-2026-05-23.md

**Reason for exclusion:** These sections contained reboot guidance, host AF_XDP cutover recommendations, and instructions to move host L3 onto an OVS bridge that contradict the current architecture.

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

**Note:** Generic reboot guidance removed; reboot decisions should follow standard s6 service deployment patterns.

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

**Note:** This model describes moving host L3 onto an OVS bridge and is invalid under the current architecture.

---

## Excerpt 4: Host L3 Route Observation from Bugs Identified

**Partial excerpt:**

```markdown
5. The prior route failure was valid: moving a `/32` public default route to
   an OVS internal port requires `onlink`.
```

**Note:** Moving the host's public default route onto an OVS internal port is no longer a valid operation; host AF_XDP and the associated L3 cutover are forbidden.

---

**Summary:** Excluded sections focused on host-side AF_XDP bridge cutover, host L3 migration, and reboot orchestration that are no longer part of the architecture.

<!-- Extracted from xdp-afxdp-conversation-2026-05-23.md on 2026-07-20 -->
