# OP-DBUS Antigravity Bridge

Local OpenAI-compatible bridge that forwards requests through an IDE's
authenticated language model session.

## Build

```bash
cd extensions/antigravity-bridge
npm install
npm run compile
```

## Run in Antigravity (VS Code compatible)

1. Open the extension folder in Antigravity.
2. Run the `OP-DBUS: Start Antigravity Bridge` command.
3. Confirm the health endpoint:

```bash
curl http://127.0.0.1:3333/health
```

## Configuration

Settings are under `opDbusBridge.*` in the IDE settings:

- `opDbusBridge.host` (default `127.0.0.1`)
- `opDbusBridge.port` (default `3333`)
- `opDbusBridge.modelFamily` (optional)
- `opDbusBridge.fallbackCommand` (default `cursor.chat.sendMessage`)

Point op-dbus to the bridge:

```
LLM_PROVIDER=antigravity
ANTIGRAVITY_BRIDGE_URL=http://127.0.0.1:3333
```

## Headless Server Note

This bridge must run inside the IDE session. On a headless op-dbus server, run
the bridge on your workstation in Antigravity and tunnel the port:

```bash
# From server to workstation (reverse tunnel)
ssh -R 3333:127.0.0.1:3333 user@workstation
```

Then keep `ANTIGRAVITY_BRIDGE_URL=http://127.0.0.1:3333` on the server so it
reaches the tunnel.
