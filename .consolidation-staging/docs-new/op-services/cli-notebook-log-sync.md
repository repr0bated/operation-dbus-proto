# CLI NotebookLM Log Sync

## Requirements

- `OP_NOTEBOOK_ID` is required for every discovered source.
- Each ZeroClaw-declared CLI/model pair writes one stable Google Drive file.
- Add that Drive file to the matching NotebookLM notebook once.
- The service overwrites the Drive file; it does not add new NotebookLM sources.
- ZeroClaw owns discovery. The sync service only consumes declared source specs.

## Design

NotebookLM has source limits, so this service treats Google Drive as the stable source of truth. ZeroClaw discovers clients/models/sessions through its plugin schema and publishes source specs under:

```text
/run/op-dbus/zeroclaw/notebooklm-sources.d/*.env
```

Each spec maps one CLI/model/session stream to one stable Drive file and one NotebookLM notebook id.
NotebookLM should contain that Drive file as a source. When the service updates the file, refresh/sync the existing Drive source in NotebookLM instead of adding another source.

The service deliberately does not hardcode VS Code, Antigravity, OpenCode, or model paths. Those details belong in ZeroClaw's D-Bus/MCP projection because `PluginSchema` is the interface contract.

ZeroClaw also declares a dedicated NotebookLM MCP instance for model transcript research:

```text
server_name: zeroclaw-model-transcripts
profile: model-transcripts
tool_prefix: zeroclaw_model_transcript
subid: exp.service.zeroclaw-model-transcripts.mcp@v1
```

That gives the chatbot/MCP surface a separate transcript research namespace while still using the regular NotebookLM MCP sidecar under cognitive-mcp. The transcript MCP reads the NotebookLM notebook whose sources are the stable Drive files maintained by this sync service.

## Code Implementation

Install the runner:

```sh
install -m 0755 deploy/bin/op-cli-notebook-log-sync.sh /usr/local/sbin/op-cli-notebook-log-sync.sh
```

Systemd instance:

```sh
install -d -m 0755 /etc/op-dbus/cli-notebook-logger
install -m 0644 deploy/config/cli-notebook-logger/zeroclaw-discovery.env.example \
  /etc/op-dbus/cli-notebook-logger/zeroclaw-discovery.env
install -m 0644 deploy/systemd/op-cli-notebook-log-sync@.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now op-cli-notebook-log-sync@zeroclaw-discovery.service
```

s6 instance:

```sh
install -d -m 0755 /etc/s6/sv/op-cli-notebook-logger/env
cp -a deploy/s6/op-cli-notebook-logger/. /etc/s6/sv/op-cli-notebook-logger/
cp -a deploy/s6/op-cli-notebook-logger-log /etc/s6/sv/op-cli-notebook-logger-log
```

For s6 envdir, create one file per variable under the service `env/` directory.
