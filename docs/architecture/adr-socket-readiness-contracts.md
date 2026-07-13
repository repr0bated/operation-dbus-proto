# ADR: Schema-backed container socket readiness

## Status

Accepted

## Context

An Incus instance can report `RUNNING` while its workload service, Unix socket,
or TCP listeners never materialized. Identity containers may also be NIC-less,
so provisioning cannot rely on package or Git access from inside the instance.

## Decision

- Declare artifacts, owner services, Unix sockets, TCP ports, and ordering
  dependencies in `deploy/config/container-socket-contracts.json`.
- Validate that document with the adjacent JSON Schema.
- Treat service activity plus actual declared listeners as the minimum readiness
  proof. Container state alone is insufficient.
- Install `rust-network-mgr` from its pinned Cargo Git revision. Build a static
  musl artifact on the host and push it into NIC-less identity containers.
- Let the supervised Rust process directly own its control socket through a
  systemd-managed runtime directory.
- Fail provisioning when the owner service or declared socket is absent.

## Trade-offs

The host becomes the artifact builder for NIC-less containers and must have the
Rust musl target available. In return, container provisioning is independent of
container DNS, package mirrors, and builder-host glibc versions.

## Consequences

Existing drift becomes visible immediately. A failed audit is actionable state,
not a successful provision with a warning. New workspace provisioning reuses
the same pinned installer and readiness gate.
