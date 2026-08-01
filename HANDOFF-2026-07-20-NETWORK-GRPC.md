# Network, gRPC, Xray, and Netmaker handoff

Date: 2026-07-20 09:01 UTC

## Stop condition

The requested live network cutover was stopped. Do **not** run
`install/3tched-artix-s6-install.sh` on this server as a deployment mechanism.
It is a broad installer, has stale/unrelated behavior, lacks a service-only
generation mode, and could overwrite working state. No repository network or
container-generator changes described below were deployed to `/etc/s6` or
activated.

The only part worth extracting for a future focused implementation is the
atomic uplink sequence:

1. Acquire DHCP on physical `eth0` before OVS owns it.
2. Capture all IPv4 addresses and the default gateway to a tmpfs snapshot.
3. Create `ovsbr0` and attach `eth0` in the same OVSDB transaction.
4. Move the captured addresses and default route to `ovsbr0`.
5. Apply host service addresses.
6. Start container-dependent services only after the host network is stable.

This must be implemented as a small, reviewed boot unit, not by rerunning the
comprehensive installer.

## Live state verified

The host network was working when this handoff was written:

- `eth0`: up, no global IPv4 address; it is an OVS port.
- `ovsbr0`: `10.200.0.1/24`, `10.0.0.2/24`,
  `188.68.58.237/22`, and `10.200.0.2/24`.
- Default route: `188.68.56.1` through `ovsbr0`.
- Gateway and public connectivity previously passed with zero packet loss.
- OVSDB and ovs-vswitchd were running.
- Netmaker container was running and its existing host veth was attached to
  `ovsbr0`.

Active relevant host services included:

- `ovsdb-server-pipeline`
- `ovs-vswitchd-pipeline`
- `ovsbr0-addr`
- `ovsbr0-uplink-addr`
- `incus-ct-netmaker-pipeline`
- `op-grpc-bridge-pipeline`
- `op-xray-daemon-pipeline`
- `opdbus-rundirs`

Host s6 services must be managed only with `sudo service6 ...`.

## Xray

Verified live policy compliance:

- Live configuration: `/dev/shm/xray_config.json` only.
- Process argument: `xray run -config /dev/shm/xray_config.json`.
- File mode/owner observed: `0640 root:xray`.
- Xray listens on `188.68.58.237:8090` and forwards to the loopback gRPC
  backend at `127.0.0.1:8090`.
- Xray HTTP proxy listens on `127.0.0.1:10809`.

Do not introduce a disk-backed live Xray configuration.

## gRPC and application sockets

Verified live endpoints:

- Host native gRPC Unix socket:
  `/run/ghostbridge/grpc.sock`, owned by `op-grpc-bridge`.
- Host gRPC-Web/TCP backend: `127.0.0.1:8090`.
- Xray uplink listener: `188.68.58.237:8090`.
- Host session D-Bus: `/run/opdbus/session-bus.sock`.
- OVSDB: `/run/openvswitch/db.sock`.
- Netmaker-container Rust network manager:
  `/run/rust-network-manager/rust-network-manager.sock`.
- Rust network manager health endpoint: `127.0.0.1:9100` inside the container.
- Proxy endpoint: `127.0.0.1:3128` inside the container. This is TCP, not a
  filesystem Unix socket.

The gRPC bridge currently works. Its boot log showed a roughly 40-second retry
loop because `/dev/shm/opdbus/plugin-blobs` was absent initially. It later
hydrated 64 schemas and opened both `/run/ghostbridge/grpc.sock` and
`127.0.0.1:8090`.

The live `/usr/local/libexec/3tched/opdbus-rundirs-up` was observed calling:

```text
/usr/local/bin/opblob seal-shm
```

The canonical installer was also edited to seal blobs before starting gRPC,
but those installer edits were not deployed. The remaining gRPC warning was a
missing optional Voyage API key; semantic trace search is unavailable, but the
bridge itself is operational.

Historical logs contain an old gRPC startup entry that serialized TLS identity
bytes, including private-key bytes. The newer deployed binary was reported as
fixed not to emit that material. Treat the historical log as sensitive and
rotate it through the approved logging procedure.

## Netmaker container

Verified listeners inside the running container:

- Netmaker API: TCP `8081`.
- EMQX: TCP `1883`, `8083`, `8084`, and `8883`.
- Rust network manager Unix socket and TCP `9100` health endpoint.
- Proxy: TCP `3128`.

Container application services were reported active through their internal
D-Bus-managed service manager. Application lifecycle operations must use
`busctl`, not `systemctl`.

Do not create another container NIC. The Netmaker container already has an
existing configured `eth0`. A temporary repository edit introduced
`incus config device add ... eth0`; it was removed before handoff and was never
deployed. The current installer template now refuses to invent a missing NIC.

## Confirmed boot-order defects

1. OVS boot logged that `eth0` had no IPv4 address when it tried to create the
   uplink snapshot. Address capture is occurring too late.
2. The deployed `ovsbr0-uplink-addr` waits for Netmaker before applying uplink
   and service addresses. This dependency is backwards: the host uplink must
   be complete before Netmaker.
3. The deployed container run script still contains an old
   `systemctl is-system-running --wait` readiness check. It violates the
   D-Bus-only container service-manager policy.
4. The comprehensive installer contains raw s6 lifecycle calls and does not
   clear stale generated dependencies. It is not safe as a live synchronization
   tool.
5. `op-ovsbr0-setup` source contains raw `s6-*` fallback/restart code outside
   its normal `--seed-only` boot path. This violates policy and needs separate
   review; do not exercise that restart path.

## Repository-only work in progress

`install/3tched-artix-s6-install.sh` has uncommitted edits including:

- port `18789` changed to `8090`;
- removal of the retired `op-grpc-bridge-zeroclaw` binary from its build list;
- SHM Xray bootstrap generation;
- blob resealing before gRPC startup;
- an experimental DHCP-first `uplink-dhcp` oneshot;
- snapshot consumption by `ovsbr0-addr`;
- removal of implicit Netmaker NIC creation.

These changes are mixed with pre-existing work and have **not** been approved
as a deployable solution. Review or extract them into a small dedicated
network-bootstrap implementation. Do not run the full installer to test them.

The worktree is heavily dirty, including a large intentional cleanup of
deprecated `deploy/` and `docs/` material. Preserve all existing changes and
do not reset or restore files globally.

## Recommended next action

Create one focused POSIX/Rust network-bootstrap artifact with no unrelated
installer behavior. It should:

- have an explicit rollback/snapshot contract;
- use the existing atomic OVSDB bridge-and-uplink transaction;
- acquire/capture DHCP before that transaction;
- apply all host addresses before container startup;
- avoid creating or removing Incus NICs;
- leave container application lifecycle to `busctl`;
- be represented by a reviewed s6 dependency graph and activated only with
  `sudo service6 ...`;
- be staged for reboot rather than piecemeal restarted over the live uplink.

Before activation, compare the focused artifact against the current live
scripts under `/usr/local/libexec/3tched` and the source definitions under
`/etc/s6/sv`. Test from noVNC with the mirror available for recovery.

