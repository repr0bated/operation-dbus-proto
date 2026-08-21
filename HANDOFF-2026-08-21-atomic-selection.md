# Atomic selection handoff — 2026-08-21

## VPS access

```sh
ssh -F /dev/null -i /home/jeremy/.ssh/vps_key -o IdentitiesOnly=yes jeremy@100.69.0.1
```

Remote repos:

- bridge/plugin: `/srv/git/odbus`
- dashboard: `/srv/git/operation-dashboard-ui-07`

Host services use runit only: `sudo sv restart op-grpc-bridge` and
`sudo sv restart tched-router`.

## Confirmed current bug

The dashboard picker in
`src/json-render/catalog/components/tched-router-picker.tsx` performs two
mutations when a provider changes:

1. `SetProvider(providerId)`
2. `SetModel(nextModels[0])`, calculated from a stale route list

That creates a race and can restore OpenRouter after another provider is
selected. Browser reproduction on `http://100.69.0.1:8080/`:

```sh
agent-browser --session tched-debug open http://100.69.0.1:8080/
agent-browser --session tched-debug select 'select:first-of-type' openai
```

The selected value returns to OpenRouter.

## Required implementation

Replace the two independent mutations with an atomic plugin method:

`SetSelection { provider_id, model_id } -> { selected_provider, selected_model }`

1. In `crates/op-plugins/src/state_plugins/tched_router.rs`:
   - add input/output structs;
   - declare the method with a dedicated capability/subid;
   - add dispatch and validation that the model belongs to the provider.
2. In `crates/op-grpc-bridge/src/tched_router_runtime.rs`:
   - validate provider/model pair against the provider catalog;
   - return one `RuntimeSelection`.
3. In `crates/op-grpc-bridge/src/mutation_engine.rs`:
   - handle `SetSelection` once;
   - update both `selected_provider` and `selected_model`, projection router
     fields, cache, and one publish.
4. In the dashboard picker:
   - replace `setProvider` and `setModel` with `setSelection`;
   - provider and model changes call only that operation.
5. Build/deploy:

```sh
cd /srv/git/odbus
CXXFLAGS='-include cstdint' cargo build --release -p op-grpc-bridge
sudo install -m 0755 target/release/op-grpc-bridge /usr/local/bin/op-grpc-bridge
sudo sv restart op-grpc-bridge

cd /srv/git/operation-dashboard-ui-07
./ui-render-update.sh
```

## Existing uncommitted bridge edits

`git diff` includes earlier bridge fixes in:

- `crates/op-grpc-bridge/src/grpc_server.rs` (protobuf JSON field names)
- `crates/op-grpc-bridge/src/mutation_engine.rs`
- `crates/op-grpc-bridge/src/tched_router_runtime.rs`

The currently deployed bridge preserves catalog model choices; do not discard
those changes.

## Router/provider state

- tched router config: `/var/lib/tched-router/config.toml`
- ChatGPT OAuth is valid in router auth store.
- xAI OAuth completed.
- Gemini device flow needs Google OAuth client id+secret.
- Claude needs `claude setup-token` through TTY.
- Anthropic provider is declared but has no token yet.
- Fixed malformed `providers.models.custom.default` with Gnoppix URI/model.

