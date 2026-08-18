This is a second revision pass on the netmaker-xray-identity-handoff spec
(.kiro/specs/netmaker-xray-identity-handoff/{requirements.md,design.md,tasks.md}). Read all
three fully — they are the correct baseline (v2, already corrected once for WG termination
point, xray passthrough constraints, and per-registration container scope). This pass adds
three things on top of that baseline. Do not re-run the whole investigation from scratch.

## Addition 1: Egress paths — label service vs. consumer

design.md §10 / requirements.md §5 currently list out-of-scope traffic paths (customer
tunnels, mail, qdrant, netmaker mesh, assistant) as a flat table. Make this clearer by
labeling each path as either:
- **service** — something hosted/provided, inbound-facing (mail, qdrant, netmaker's public
  API, the assistant control plane)
- **consumer** — something initiating outbound traffic (`wgcf-egress`, and now the new
  per-registration identity container's own egress)

Add this distinction to the out-of-scope table and to the per-registration identity
container's egress description (§6.2/§6.3) so it's unambiguous which direction each path
serves.

## Addition 2: OpenFlow identity-awareness — corrected understanding, now in scope

The previous investigation (and the first draft of this spec) concluded OpenFlow couldn't
carry per-peer identity because "OF1.3 has no spare match field." That conclusion was WRONG
and must be corrected. Verified directly against the actual dependency crate
(`/home/admin/.cargo/registry/.../rovs-openflow-0.2.0/src/match_fields.rs` and `oxm.rs`):

- `rovs_openflow::Match` already has `metadata`/`metadata_mask` fields AND
  `ct_mark`/`ct_mark_mask` fields defined in the struct.
- `oxm.rs` has full NXM register encoding for REG0–REG15 plus 128-bit extended registers
  (XXREG0–3) — ample bits for a WG-pubkey-derived identifier.
- `JsonFlowAction::LoadRegister { register: u8, value: u64 }` already exists in the JSON
  schema (`op-plugins/src/state_plugins/openflow.rs`).

The actual gap is narrower than previously stated: `op-network/src/openflow_translate.rs`'s
`parse_match()` (around line 145-230) only exposes `in_port`, `dl_type`, `dl_vlan`, `dl_src`,
`tcp_flags`, `tp_src`/`tp_dst`, `nw_src`/`nw_dst`, `ct_state` as parseable JSON match_fields
keys — `metadata`, `ct_mark`, and `reg[N]` matching are not wired into that translation layer
yet, even though the crate underneath already supports them. This is missing plumbing in one
file, not a protocol limitation.

**Design requirement**: the per-registration identity container's own OVS port IS the
identity — not a separate abstract pubkey floating disconnected from network state. Each
provisioned identity container gets its own dedicated OVS port (per §6 of the existing
design). OpenFlow flows should tag traffic entering on that specific port with a register or
`ct_mark` value identifying the container (loaded via the existing `LoadRegister` action at
flow-install time, tied to the container's provisioning step). Downstream flows and/or the
in-container verification logic can then check that register/mark instead of (or in addition
to) doing a pure IP-based lookup in the TransportBindingIndex — the OVS port assignment itself
becomes an additional, datapath-level unforgeable binding, since only that container's traffic
can physically arrive on its own port.

Add a new design.md section covering: (1) extending `parse_match()`/`build_actions()` in
`openflow_translate.rs` to support `metadata`, `ct_mark`, and `reg[N]` match/load fields,
(2) how container provisioning (§6.1/P-3) assigns each container a dedicated OVS port and a
corresponding register/ct_mark value, (3) how that value is set on flow install and read
downstream. Add corresponding tasks (new task section, e.g. "OpenFlow Identity Tagging") with
the same fail-closed/evidence-backed rigor as the rest of tasks.md. This is real in-scope work
for this spec now, not deferred.

## Addition 3: Extended container lifecycle actions

design.md §6.4 / tasks.md §7 (Container Lifecycle Management) currently cover only
provision/deprovision/TTL-expiry. Add these additional lifecycle actions, and for each,
explicitly decide whether it's in-scope for this spec now or belongs in an explicit "Backlog /
Future Work" section (do not silently omit any of them either way — every one needs a stated
decision with a one-line reason):

- Workspace upgrade (upgrading a running identity container's contents/verification binary
  without full reprovisioning)
- OS upgrade (upgrading the container's base image/OS)
- Account activation (re-enabling a previously deactivated identity without full
  reprovisioning)
- Account deactivation (suspending access without deprovisioning — distinct from revocation,
  which tasks.md's L-1 already covers as permanent removal)

If any of these are marked in-scope, add corresponding tasks in the same
fail-closed/evidence-backed style. If deferred, add a "Backlog / Future Work" section to
design.md listing them with a one-line reason each (e.g. "requires container image versioning
strategy not yet decided").

## Addition 4: Human-friendly container alias, kept separate from identity

Container naming (§6.1: `format!("identity-{}", event.tenant_id)`) uses a raw tenant UUID —
not something a human can recognize in `incus list`, logs, or audit trails. Add a
human-friendly alias, generated deterministically at provisioning time (e.g. a petname-style
adjective-noun pair derived from a hash of the tenant_id/pubkey — the exact scheme is Kiro's
call, but it must be deterministic so the same identity always gets the same alias).

Hard requirement: this alias is for **display/reference only** — container naming, logs,
`incus list` output, audit trail readability. It must NEVER be accepted as input to any
verification, capability-grants, or footprint-matching logic. Only the real
Blake3-derived footprint is authoritative for authorization decisions. State this
explicitly in design.md as a named constraint (something like "alias is identifying, not
authenticating") so a future implementer doesn't accidentally wire the alias into the trust
path. Add this to §6.2 (container contents) and as a task in the provisioning section
(§5/P-*).

## Output

Revise requirements.md, design.md, and tasks.md in place. Keep the same evidence-backed
`[x]`/`[ ]` checkbox rigor and fail-closed discipline as the existing files.
