# Routing Plugin Spec (v1)

## Intent

Own **what gets routed and how identity arrives** — not compliance, not UI gallery, not the xray process.

**Retire `gemma_brain`.** Tags come from the subid registry; this plugin consumes them. Compliance / Cozo vectorized rules wait until that graph form exists.

## Control-plane rule

- **Primary:** D-Bus object `/org/opdbus/v1/plugins/routing` is the payload / authority.
- **Transport:** gRPC (`PluginService.CallMethod` / zcall) reaches that object.
- **Secondary:** `/dev/shm/xray-routes.json` is an optional projection for xray-core. If SHM and D-Bus disagree, **D-Bus wins** — re-`publish`.

## Schema surfaces

1. **Identity injection** — decoy stamps `X-Ghostbridge-*`; forward to 3tched `:8090`; not local `wg show`.
2. **Subdomains** — tag → hosts → backend (public).
3. **Internal services** — file/internal; never emit xray outbounds.

## Methods

| Method | Side | Purpose |
|--------|------|---------|
| `get_state` | read | Full verbal state |
| `list_subdomains` | read | Public routes |
| `list_internal` | read | Non-routable |
| `get_identity_injection` | read | Hop + headers |
| `list_tags` | read | Tag vocabulary |
| `derive` | mut | Rebuild from registry |
| `publish` | mut | Atomic SHM projection |
| `upsert_subdomain` / `remove_subdomain` | mut | Operator override |

## gemma_brain teardown

| Piece | Home |
|-------|------|
| Tag routing | `routing` |
| Identity hop | `routing.identity_injection` |
| Gallery / UI-gen | `json_render` + dev UI (later) |
| LLM pointer | `large_language_model` |
| Perspectives / analyze_intent | drop |
| `gemma_brain` plugin | deleted |

## Default path when xAI / Grok is down

Routing does **not** call an LLM. Default loop:

1. `zcall routing derive` — registry tags → D-Bus object state
2. `zcall routing publish` — optional SHM projection for xray
3. `op-identity-shuttle` — materialize xray config

s6 `gemma` service prefers that zcall path; if zcall/routing dispatch is unavailable it falls back to the local `op-gemma` binary (same registry derive, still no xAI). Grok is only for optional future route *proposals*, never required for the network to work.

## Out of scope

Cozo compliance, UI Gem chat, owning the xray daemon.
