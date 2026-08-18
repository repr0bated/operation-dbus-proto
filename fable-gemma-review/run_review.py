#!/usr/bin/env python3
"""Drive gemma-4 code review of PR #30 groups via the gnoppix proxy (curl transport)."""
import json
import os
import re
import subprocess
import sys
import time

REPO = "/srv/git/odbus"
BASE = "52f07e68"
OUT = "/tmp/gnoppix-review"
URL = "https://us.gnoppix.com/v1/chat/completions"
MODELS = ["google/gemma-4-26b-a4b-it:free", "google/gemma-4-31b-it:free"]
FILE_CAP = 40_000   # bytes per file section
TOTAL_CAP = 90_000  # bytes per group diff

SYSTEM = """You are a senior staff Rust engineer reviewing a pull request diff for the OP-DBUS control plane.
Report ONLY high-confidence, actionable bugs visible in the diff: definite runtime failures, wrong logic with observable effects, security issues with a realistic path, data loss, dead code left by refactors, breaking API/contract changes, or violations of the project invariants below.
Do NOT report style, speculation, missing tests, or hypothetical hardening.

Project invariants (violations are reportable):
- D-Bus is the only control plane: no Command::new subprocesses, no polling loops, no direct file reads for live state in plugin/service code.
- PluginSchema is the single source of truth; derived values computed in exactly one place.
- Method outputs must be real typed structs; no `bool success` fields (errors via tonic::Status / D-Bus errors).
- Capability ids used by methods must be declared in schema.capabilities (legacy short-form `{plugin}.{read|invoke}` tolerated).
- subid taxonomy: `<cat>.<component>.<subject>.<verb>[.<facet>][@vN]`, cat in {src,prj,sch,mut,obs,evt,exp}.
- op-web is Axum 0.7 (`:param` route syntax, not `{param}`).
- Host runs runit; deploy scripts must not use systemctl or other service managers.
- Plugin id rename zeroclaw->tched_router: consumers must have a consistent fallback story (reading tched_router in one place but zeroclaw in another for the same datum is a bug).

Output format: a STRICT JSON array, nothing else. Each element:
{"path": "<file>", "line": <int or null>, "priority": "P0|P1|P2|P3", "title": "<imperative, <=80 chars>", "why": "<one paragraph: why it is a bug and how it manifests>"}
Return [] if the group is clean. Since you cannot run tools, only report what the diff itself evidences, and prefer P2 when you cannot see the full context."""

GROUPS = {
    "g1": {
        "focus": "Group 1: tched_router plugin core. The PR renames the zeroclaw D-Bus plugin to tched_router (65-method generated config surface), adds vendor stubs (vendor/zeroclawlabs) so the workspace builds without /srv/git/zeroclaw, and registers the plugin. Watch for: plugin id/blob naming inconsistency, schema surface errors, vendor stub type drift vs real zeroclaw::Config, registry registration gaps, dead code from the rename. Note: tched_router_config_surface.rs and the .inc file are GENERATED; sections may be truncated.",
        "paths": [
            "crates/op-plugins/src/state_plugins/tched_router.rs",
            "crates/op-plugins/src/default_registry.rs",
            "crates/op-plugins/src/lib.rs",
            "crates/op-plugins/src/state_plugins/mod.rs",
            "crates/op-plugins/Cargo.toml",
            "Cargo.toml",
            "vendor/zeroclawlabs",
            "crates/op-plugins/examples",
            "crates/op-plugins/src/bin",
            "crates/op-plugins/src/state_plugins/tched_router_config_surface.rs",
        ],
    },
    "g2": {
        "focus": "Group 2: PluginSchema capability model. Adds PluginSchema.capabilities/CapabilityDecl with 3-place resolution (plugin decl -> method string -> legacy derived {plugin}.{read|invoke}), plus a sweep of schema.capabilities.insert declarations across plugins. Watch for: resolution-order logic errors, serde compatibility of the new field with existing sealed blobs (missing #[serde(default)]), sweep hunks whose declared ids do not match the required_capability strings used by the same file's methods, subid registry duplicates.",
        "paths": [
            "crates/op-state-store/src/plugin_schema.rs",
            "crates/op-state-store/src/lib.rs",
            "crates/op-plugins/src/state_plugins/plugin_scaffold_helpers.rs",
            "crates/op-plugins/src/state_plugins/oscal_subid_registry.rs",
            "crates/op-plugins/src/state_plugins/adc.rs",
            "crates/op-plugins/src/state_plugins/antigravity.rs",
            "crates/op-plugins/src/state_plugins/cognitive_mcp.rs",
            "crates/op-plugins/src/state_plugins/config.rs",
            "crates/op-plugins/src/state_plugins/factory.rs",
            "crates/op-plugins/src/state_plugins/incus.rs",
            "crates/op-plugins/src/state_plugins/json_render.rs",
            "crates/op-plugins/src/state_plugins/large_language_model.rs",
            "crates/op-plugins/src/state_plugins/mcp.rs",
            "crates/op-plugins/src/state_plugins/proxy_server.rs",
            "crates/op-plugins/src/state_plugins/service.rs",
            "crates/op-plugins/src/state_plugins/users.rs",
            "crates/op-plugins/src/state_plugins/web_ui.rs",
            "crates/op-plugins/src/state_plugins/gemma_brain.rs",
            "crates/op-plugins/src/state_plugins/identity_sled.rs",
            "crates/op-plugins/src/state_plugins/schema_renderer.rs",
        ],
    },
    "g3": {
        "focus": "Group 3: op-grpc-bridge and LLM consumers rewired from state_plugins::zeroclaw to TchedRouterState.catalog. gRPC pipeline: per-plugin surface generated from schema.methods; dynamic reflection hydrates from sealed SHM blobs named <plugin_id>.<hash16>.blob; dispatch must route through D-Bus PluginService.CallMethod. Watch for: inconsistent tched_router/zeroclaw fallbacks across lookups of the same datum, wrong catalog field access, reflection breaking on a pre-reseal host where only the zeroclaw blob exists, D-Bus bypasses.",
        "paths": [
            "crates/op-grpc-bridge/src/chat_service.rs",
            "crates/op-grpc-bridge/src/dynamic_reflection.rs",
            "crates/op-grpc-bridge/src/lib.rs",
            "crates/op-grpc-bridge/src/mutation_engine.rs",
            "crates/op-grpc-bridge/src/schema_passthrough.rs",
            "crates/op-grpc-bridge/src/zeroclaw_object_blob.rs",
            "crates/op-grpc-bridge/examples/repro_subscribe.rs",
            "crates/op-llm/src/schema.rs",
            "crates/op-cognitive-mcp/src/qdrant_shuttle.rs",
        ],
    },
    "g4": {
        "focus": "Group 4: op-web HTTP surface + frontend contract. Contract: /api/llm/status emits active_provider/active_model plus legacy provider/model keys; /api/llm/models lists tched_router model_routes (flattened catalog, or nested projection/catalog); POST /api/llm/model switches model ({\"model\": ...}); /api/zeroclaw/* stays, /api/tched_router/schema is an alias; plugin lookup falls back to zeroclaw until reseal. Watch for: JSON field-name mismatches between handlers and LlmPage.tsx, projection vs catalog nesting bugs, Axum 0.7 route syntax, panics/unwraps on SHM JSON.",
        "paths": [
            "crates/op-web/src/handlers/zeroclaw.rs",
            "crates/op-web/src/handlers/llm.rs",
            "crates/op-web/src/routes/llm.rs",
            "crates/op-web/src/routes/mod.rs",
            "crates/op-web/src/zeroclaw_routes.rs",
            "crates/src/pages/LlmPage.tsx",
            "crates/op-cozo-store/examples/delete_identity_session.rs",
        ],
    },
    "g5": {
        "focus": "Group 5: network/OVS plugins + deploy scripts. The fabric: one OVS bridge ovsbr0, native OVSDB JSON-RPC (no CLI subprocesses), netmaker WG interface is an ovsbr0 port carried by L3->encap OpenFlow flows. openflow.rs recently merge-resolved: six datapath methods plus openflow.read/openflow.write capability declarations must be coherent (every schema.methods key needs a dispatch arm and input struct, no duplicate inserts). deploy/chrome-remote-desktop is new; host runs runit so systemctl usage in executable scripts is a violation.",
        "paths": [
            "crates/op-plugins/src/state_plugins/openflow.rs",
            "crates/op-plugins/src/state_plugins/ovsdb_bridge.rs",
            "crates/op-plugins/src/state_plugins/rovs_commands.rs",
            "crates/op-plugins/src/state_plugins/net.rs",
            "crates/op-plugins/src/state_plugins/netmaker.rs",
            "crates/op-plugins/src/state_plugins/rtnetlink.rs",
            "crates/op-plugins/src/state_plugins/unix_socket.rs",
            "crates/op-plugins/src/state_plugins/shared_unix_socket.rs",
            "crates/op-plugins/src/state_plugins/wireguard.rs",
            "crates/op-plugins/src/state_plugins/wg_opdbus.rs",
            "crates/op-plugins/src/state_plugins/wgcf.rs",
            "crates/op-plugins/src/state_plugins/xray.rs",
            "crates/op-plugins/src/state_plugins/ghostbridge.rs",
            "deploy/chrome-remote-desktop",
        ],
    },
}


def group_diff(paths):
    diff = subprocess.run(
        ["git", "diff", f"{BASE}..HEAD", "--"] + paths,
        cwd=REPO, capture_output=True, text=True, check=True,
    ).stdout
    sections = re.split(r"(?m)^(?=diff --git )", diff)
    out, total = [], 0
    for sec in sections:
        if not sec.strip():
            continue
        if len(sec) > FILE_CAP:
            sec = sec[:FILE_CAP] + "\n...[TRUNCATED: generated/large file, remainder omitted]\n"
        if total + len(sec) > TOTAL_CAP:
            out.append("\n...[GROUP BUDGET REACHED: remaining file diffs omitted]\n")
            break
        out.append(sec)
        total += len(sec)
    return "".join(out)


def call_model(payload_path, out_path):
    key = os.environ["GNOPPIX_API_KEY"]
    for attempt in range(6):
        model = MODELS[0] if attempt < 3 else MODELS[1]
        # rewrite model in payload file
        with open(payload_path) as f:
            payload = json.load(f)
        payload["model"] = model
        with open(payload_path, "w") as f:
            json.dump(payload, f)
        r = subprocess.run(
            ["curl", "-s", "-m", "300", URL,
             "-H", "Content-Type: application/json",
             "-H", f"Authorization: Bearer {key}",
             "--data-binary", f"@{payload_path}"],
            capture_output=True, text=True,
        )
        body = r.stdout
        try:
            j = json.loads(body)
        except json.JSONDecodeError:
            j = {"error": {"message": body[:200]}}
        if "choices" in j:
            with open(out_path, "w") as f:
                f.write(body)
            return j["choices"][0]["message"]["content"], model
        msg = str(j.get("error", {}))[:160]
        print(f"    attempt {attempt+1} ({model}): {msg}", flush=True)
        time.sleep(25)
    return None, None


def main():
    os.makedirs(OUT, exist_ok=True)
    only = sys.argv[1:] or list(GROUPS)
    for name in only:
        spec = GROUPS[name]
        diff = group_diff(spec["paths"])
        user = (f"PR #30 'Port OpenCode tched_router migration onto main' "
                f"(repo repr0bated/operation-dbus-proto, Rust workspace).\n\n"
                f"{spec['focus']}\n\n=== DIFF (base {BASE}) ===\n{diff}")
        payload = {
            "model": MODELS[0],
            "temperature": 0.2,
            "max_tokens": 3000,
            "messages": [
                {"role": "system", "content": SYSTEM},
                {"role": "user", "content": user},
            ],
        }
        ppath = f"{OUT}/{name}.payload.json"
        with open(ppath, "w") as f:
            json.dump(payload, f)
        print(f"{name}: diff {len(diff)} bytes, prompt ~{(len(SYSTEM)+len(user))//4} tokens", flush=True)
        content, model = call_model(ppath, f"{OUT}/{name}.response.json")
        if content is None:
            print(f"{name}: FAILED after retries", flush=True)
            continue
        with open(f"{OUT}/{name}.findings.txt", "w") as f:
            f.write(content)
        print(f"{name}: ok via {model}, {len(content)} chars", flush=True)


if __name__ == "__main__":
    main()
