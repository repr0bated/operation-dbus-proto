# Deferred build/deploy — 2026-08-15

Code is written and type-checked. **Builds and restarts deliberately deferred**
to a single batch at the end rather than run interactively (an op-grpc-bridge
release build is ~11–21 min).

## Not yet built or deployed

`crates/op-grpc-bridge/src/mutation_engine.rs` — bounded event-chain replay.

- `rebuild_chain_from_disk` previously read and parsed **every** record in
  `/var/lib/opdbus/snowball/timing` (1,709,617 of them) into one `Vec`, then
  replayed all of them. Measured cost: ~34 s of the boot path and **13.15 GB
  RSS**.
- Now selects the newest `OP_EVENT_CHAIN_REPLAY_LIMIT` records (default
  50,000) **by filename**, so nothing is opened or parsed outside the window,
  then replays them oldest-first.
- `cargo check --release -p op-grpc-bridge` passes. Unit tests
  (`mod replay_window_tests`) are written but **have not been run**.

### To finish

```sh
cargo test --release -p op-grpc-bridge --lib replay_window
cargo build --release -p op-grpc-bridge
sudo sv stop op-grpc-bridge
# reseed, since the running binary writes blocks without maintaining the checkpoint
cd /var/lib/opdbus/snowball/timing && \
  ls -U | sed -n 's/^block-0*\([0-9]\+\)\.json$/\1/p' | sort -n | tail -1 \
  | sudo tee /var/lib/opdbus/snowball/.block-counter
sudo install -m 755 -o root -g root \
  /srv/git/odbus/target/release/op-grpc-bridge /usr/local/bin/op-grpc-bridge
sudo sv start op-grpc-bridge
```

Expected after deploy: `event chain rebuilt ... replayed=` drops from 1709617 to
≤50000, RSS well under 13 GB, and time-to-socket-bind well under 3m37s.

## Already built, installed, and verified today

| Change | State |
|---|---|
| `zeroclaw-gui` → `unix:/run/ghostbridge/container.sock` (+ `AUTO_UNIX_SOCKET`, accountability default) | installed, connecting |
| `op-plugin-lint` dedupe on protobuf JSON name (`json_name_key`) | built, 11+5 tests pass |
| `zeroclaw.rs` — 10 colliding generated fields removed (556→546) | built, installed |
| `opblob stage-shm` / `persist`; boot script stages instead of seals | installed; staging measured **62 ms** from empty tmpfs |
| `snowball.rs` block-counter checkpoint + stale-checkpoint guard | built, installed, running; 32 tests pass |

## Known-open

- Startup is still ~3m37s and 13 GB **before** the deferred change lands; the
  checkpoint fix only removed ~6 s of it.
- `cognitive_mcp` disabled at runtime: `op-web-server` (pid 1218) holds the
  CozoDB lock at `/var/lib/op-cognitive-mcp/memory.db`. Pre-existing.
- `op-web` was rebuilt on a hypothesis that proved wrong (it does not depend on
  `op-plugins`); **not installed**, deliberately.
