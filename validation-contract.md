# Validation Contract: State Plugin Schema Completion

This contract defines assertions ensuring that each state plugin under `crates/op-plugins/src/state_plugins/` generates a JSON schema (via **schemars**) that includes all required D‑Bus methods as prescribed by the official specifications (systemd, OpenStack, etc.).

---

## Assertions

| Assertion ID | Plugin | Title | Description | Tool | Evidence |
|---|---|---|---|---|---|
| VAL-PLUGIN-ADC | `adc.rs` | ADC schema includes required methods | Verify that the ADC plugin schema contains the D‑Bus methods required for ADC control (e.g., `GetValue`, `SetMode`, `Start`, `Stop`). | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-AGENT_CONFIG | `agent_config.rs` | Agent Config schema includes required methods | Ensure the Agent Config plugin schema defines D‑Bus methods such as `GetConfig`, `SetConfig`, `Reload`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-ANTIGRAVITY_CHAT | `antigravity_chat.rs` | Antigravity Chat schema includes required methods | Check that the schema lists D‑Bus methods like `SendMessage`, `ReceiveMessage`, `ListChats`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-ANTIGRAVITY | `antigravity.rs` | Antigravity schema includes required methods | Verify methods `Initialize`, `Terminate`, `GetStatus`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-BLOCKCHAIN_PLUGIN | `blockchain_plugin.rs` | Blockchain Plugin schema includes required methods | Ensure methods `SubmitTransaction`, `QueryBlock`, `GetBalance` are present. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-BTRFS_PLUGIN | `btrfs_plugin.rs` | Btrfs Plugin schema includes required methods | Verify methods `CreateSubvolume`, `DeleteSubvolume`, `Snapshot`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-COGNITIVE_MCP | `cognitive_mcp.rs` | Cognitive MCP schema includes required methods | Check for methods `ProcessInput`, `GenerateResponse`, `GetContext`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-COMPACT_MCP | `compact_mcp.rs` | Compact MCP schema includes required methods | Verify methods `StartSession`, `EndSession`, `SendHeartbeat`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-CONFIG | `config.rs` | Config schema includes required methods | Ensure methods `Load`, `Save`, `Reload`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-COZO | `cozo.rs` | Cozo schema includes required methods | Verify methods `Query`, `Execute`, `ImportData`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-CRON | `cron.rs` | Cron schema includes required methods | Ensure methods `Schedule`, `Unschedule`, `ListJobs`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-CTL_PLANE_CHATBOT | `ctl_plane_chatbot.rs` | CTL Plane Chatbot schema includes required methods | Verify methods `SendCommand`, `ReceiveOutput`, `GetInfo`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-DATASTORE | `datastore.rs` | Datastore schema includes required methods | Ensure methods `Put`, `Get`, `Delete`, `ListKeys`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-DNSRESOLVER | `dnsresolver.rs` | DNS Resolver schema includes required methods | Verify methods `Resolve`, `ReverseLookup`, `SetCache`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-ENDPOINT | `endpoint.rs` | Endpoint schema includes required methods | Ensure methods `Create`, `Update`, `Delete`, `GetStatus`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-FACTORY | `factory.rs` | Factory schema includes required methods | Verify methods `Instantiate`, `Destroy`, `GetMetadata`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-FAIL2BAN | `fail2ban.rs` | Fail2Ban schema includes required methods | Ensure methods `BanIP`, `UnbanIP`, `ListBans`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-FREEDESKTOP | `freedesktop.rs` | Freedesktop schema includes required methods | Verify methods defined by the Freedesktop D‑Bus spec such as `Get`, `Set`, `List`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-FULL_SYSTEM | `full_system.rs` | Full System schema includes required methods | Ensure comprehensive methods covering system control (`Reboot`, `PowerOff`, `Suspend`). | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-GCLOUD_ADC | `gcloud_adc.rs` | GCloud ADC schema includes required methods | Verify methods `Authenticate`, `RefreshToken`, `GetCredentials`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-GEMMA_BRAIN | `gemma_brain.rs` | Gemma Brain schema includes required methods | Ensure methods `Infer`, `LoadModel`, `UnloadModel`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-HARDWARE | `hardware.rs` | Hardware schema includes required methods | Verify methods `Enumerate`, `GetInfo`, `SetPowerState`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-INCUS_DEVICE | `incus_device.rs` | Incus Device schema includes required methods | Ensure methods `CreateDevice`, `DeleteDevice`, `Inspect`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-INCUS | `incus.rs` | Incus schema includes required methods | Verify methods `CreateContainer`, `StartContainer`, `StopContainer`, `DeleteContainer`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-KEYPAIR | `keypair.rs` | Keypair schema includes required methods | Ensure methods `Generate`, `Export`, `Import`, `Rotate`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-KEYRING | `keyring.rs` | Keyring schema includes required methods | Verify methods `StoreSecret`, `RetrieveSecret`, `DeleteSecret`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-KNOWLEDGE_PLUGIN | `knowledge_plugin.rs` | Knowledge Plugin schema includes required methods | Ensure methods `AddFact`, `QueryFact`, `RemoveFact`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-LARGE_LANGUAGE_MODEL | `large_language_model.rs` | LLM schema includes required methods | Verify methods `GenerateText`, `StreamTokens`, `SetParameters`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-LOGIN1 | `login1.rs` | Login1 schema includes required methods | Ensure methods defined by systemd‑login1 spec such as `ListSessions`, `TerminateSession`, `GetSeat`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-LXC | `lxc.rs` | LXC schema includes required methods | Verify methods `CreateContainer`, `Start`, `Stop`, `Delete`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-MAIL_SERVER | `mail_server.rs` | Mail Server schema includes required methods | Ensure methods `SendMail`, `ReceiveMail`, `ListMailboxes`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-MCP | `mcp.rs` | MCP schema includes required methods | Verify core MCP methods `RegisterPlugin`, `UnregisterPlugin`, `InvokeMethod`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-MEMORY_PLUGIN | `memory_plugin.rs` | Memory Plugin schema includes required methods | Ensure methods `Allocate`, `Free`, `Read`, `Write`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-NETMAKER | `netmaker.rs` | Netmaker schema includes required methods | Verify methods `CreateNetwork`, `JoinNetwork`, `LeaveNetwork`, `DeleteNetwork`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-NET | `net.rs` | Net schema includes required methods | Ensure methods `ConfigureInterface`, `SetIP`, `BringUp`, `BringDown`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-NOTEBOOKLM | `notebooklm.rs` | NotebookLM schema includes required methods | Verify methods `CreateNotebook`, `AddNote`, `QueryNotes`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-OCI | `oci.rs` | OCI schema includes required methods | Ensure methods `PullImage`, `PushImage`, `RunContainer`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-OPENFLOW_OBFUSCATION | `openflow_obfuscation.rs` | OpenFlow Obfuscation schema includes required methods | Verify methods `ObfuscateFlow`, `DeobfuscateFlow`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-OPENFLOW | `openflow.rs` | OpenFlow schema includes required methods | Ensure methods `AddFlow`, `DeleteFlow`, `ModifyFlow`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-OSCAL_SUBID_REGISTRY | `oscal_subid_registry.rs` | OSCAL SubID Registry schema includes required methods | Verify registry management methods `RegisterSubID`, `LookupSubID`, `UpdateSubID`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-OVSDB_BRIDGE | `ovsdb_bridge.rs` | OVSDB Bridge schema includes required methods | Ensure methods `CreateBridge`, `DeleteBridge`, `AddPort`, `RemovePort`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PACKAGEKIT | `packagekit.rs` | PackageKit schema includes required methods | Verify methods `InstallPackage`, `RemovePackage`, `ListPackages`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PCIDECL | `pcidecl.rs` | PCI Decl schema includes required methods | Ensure methods `EnumerateDevices`, `GetDeviceInfo`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PERSONA | `persona.rs` | Persona schema includes required methods | Verify methods `GetIdentity`, `SetIdentity`, `Authenticate`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PLUGIN_SCHEMA_DEFS | `plugin_schema_defs.rs` | Plugin Schema Definitions schema includes required methods | Ensure that the central `PluginSchema` definition provides all required D‑Bus method signatures for every plugin. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PRIVACY_ROUTER | `privacy_router.rs` | Privacy Router schema includes required methods | Verify methods `EnforcePolicy`, `AuditRequest`, `LogDecision`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PRIVACY_ROUTES | `privacy_routes.rs` | Privacy Routes schema includes required methods | Ensure route handling methods `AddRoute`, `RemoveRoute`, `MatchRoute`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PRIVACY | `privacy.rs` | Privacy schema includes required methods | Verify methods `MaskData`, `UnmaskData`, `GetPolicy`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PROCFS | `procfs.rs` | ProcFS schema includes required methods | Ensure methods `ReadProc`, `WriteProc`, `ListEntries`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PROXMox | `proxmox.rs` | Proxmox schema includes required methods | Verify methods `CreateVM`, `StartVM`, `StopVM`, `DeleteVM`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-PROXY_SERVER | `proxy_server.rs` | Proxy Server schema includes required methods | Ensure methods `StartProxy`, `StopProxy`, `Configure`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-QDRANT | `qdrant.rs` | Qdrant schema includes required methods | Verify methods `CreateCollection`, `InsertPoints`, `Search`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-ROVS_COMMANDS | `rovs_commands.rs` | ROVS Commands schema includes required methods | Ensure command methods `Execute`, `Cancel`, `Status`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-RTNETLINK | `rtnetlink.rs` | RTNetlink schema includes required methods | Verify methods `AddLink`, `DeleteLink`, `SetLink`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-S6 | `s6.rs` | S6 schema includes required methods | Ensure methods `StartService`, `StopService`, `ReloadService`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-S6_SYSTEMCTL | `s6_systemctl.rs` | S6 Systemctl schema includes required methods | Verify methods mirroring systemd `systemctl` actions: `Start`, `Stop`, `Restart`, `Status`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SCHEMA_CONTRACT | `schema_contract.rs` | Schema Contract schema includes required methods | Ensure the contract defines validation method `ValidatePluginSchema`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SCHEMA_RENDERER | `schema_renderer.rs` | Schema Renderer schema includes required methods | Verify methods `RenderSchema`, `ExportJSON`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SCHEMARS_ADAPTER | `schemars_adapter.rs` | Schemars Adapter schema includes required methods | Ensure adapter provides `GenerateSchema` and `SerializeSchema`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SERVICE | `service.rs` | Service schema includes required methods | Verify service lifecycle methods `Init`, `Run`, `Shutdown`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SESSDECL | `sessdecl.rs` | Session Declaration schema includes required methods | Ensure methods `CreateSession`, `TerminateSession`, `GetSessionInfo`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SHARED_UNIX_SOCKET | `shared_unix_socket.rs` | Shared Unix Socket schema includes required methods | Verify methods `Connect`, `Send`, `Receive`, `Close`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SOFTWARE | `software.rs` | Software schema includes required methods | Ensure methods `Install`, `Uninstall`, `Update`, `GetInfo`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SYSTEMD_NETWORKD | `systemd_networkd.rs` | Systemd Networkd schema includes required methods | Verify methods as per systemd‑networkd spec: `Reload`, `SetLink`, `GetLink`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-SYSTEMD | `systemd.rs` | Systemd schema includes required methods | Ensure core systemd methods `StartUnit`, `StopUnit`, `RestartUnit`, `GetUnitStatus`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-UNIX_SOCKET | `unix_socket.rs` | Unix Socket schema includes required methods | Verify methods `Bind`, `Listen`, `Accept`, `Close`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-USERS | `users.rs` | Users schema includes required methods | Ensure methods `CreateUser`, `DeleteUser`, `ModifyUser`, `ListUsers`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-WEB_UI | `web_ui.rs` | Web UI schema includes required methods | Verify UI service methods `RenderPage`, `HandleEvent`, `UpdateComponent`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-WGCF | `wgcf.rs` | WGCF schema includes required methods | Ensure methods `GenerateConfig`, `ApplyConfig`, `Refresh`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-WIREGUARD | `wireguard.rs` | Wireguard schema includes required methods | Verify methods `AddPeer`, `RemovePeer`, `ListPeers`, `SetConfig`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-WORKFLOWS_PLUGIN | `workflows_plugin.rs` | Workflows Plugin schema includes required methods | Ensure workflow control methods `StartWorkflow`, `PauseWorkflow`, `ResumeWorkflow`, `CancelWorkflow`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-XRAY | `xray.rs` | XRay schema includes required methods | Verify tracing methods `StartTrace`, `EndTrace`, `RecordSpan`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |
| VAL-PLUGIN-ZEROCLAW | `zeroclaw.rs` | Zeroclaw schema includes required methods | Ensure methods `RegisterAgent`, `ReportStatus`, `ReceiveCommand`. | cargo check | Successful `cargo check` and presence of method fields in the generated schema struct. |

---

## Assertion ID Index

```
VAL-PLUGIN-ADC
VAL-PLUGIN-AGENT_CONFIG
VAL-PLUGIN-ANTIGRAVITY_CHAT
VAL-PLUGIN-ANTIGRAVITY
VAL-PLUGIN-BLOCKCHAIN_PLUGIN
VAL-PLUGIN-BTRFS_PLUGIN
VAL-PLUGIN-COGNITIVE_MCP
VAL-PLUGIN-COMPACT_MCP
VAL-PLUGIN-CONFIG
VAL-PLUGIN-COZO
VAL-PLUGIN-CRON
VAL-PLUGIN-CTL_PLANE_CHATBOT
VAL-PLUGIN-DATASTORE
VAL-PLUGIN-DNSRESOLVER
VAL-PLUGIN-ENDPOINT
VAL-PLUGIN-FACTORY
VAL-PLUGIN-FAIL2BAN
VAL-PLUGIN-FREEDESKTOP
VAL-PLUGIN-FULL_SYSTEM
VAL-PLUGIN-GCLOUD_ADC
VAL-PLUGIN-GEMMA_BRAIN
VAL-PLUGIN-HARDWARE
VAL-PLUGIN-INCUS_DEVICE
VAL-PLUGIN-INCUS
VAL-PLUGIN-KEYPAIR
VAL-PLUGIN-KEYRING
VAL-PLUGIN-KNOWLEDGE_PLUGIN
VAL-PLUGIN-LARGE_LANGUAGE_MODEL
VAL-PLUGIN-LOGIN1
VAL-PLUGIN-LXC
VAL-PLUGIN-MAIL_SERVER
VAL-PLUGIN-MCP
VAL-PLUGIN-MEMORY_PLUGIN
VAL-PLUGIN-NETMAKER
VAL-PLUGIN-NET
VAL-PLUGIN-NOTEBOOKLM
VAL-PLUGIN-OCI
VAL-PLUGIN-OPENFLOW_OBFUSCATION
VAL-PLUGIN-OPENFLOW
VAL-PLUGIN-OSCAL_SUBID_REGISTRY
VAL-PLUGIN-OVSDB_BRIDGE
VAL-PLUGIN-PACKAGEKIT
VAL-PLUGIN-PCIDECL
VAL-PLUGIN-PERSONA
VAL-PLUGIN-PLUGIN_SCHEMA_DEFS
VAL-PLUGIN-PRIVACY_ROUTER
VAL-PLUGIN-PRIVACY_ROUTES
VAL-PLUGIN-PRIVACY
VAL-PLUGIN-PROCFS
VAL-PLUGIN-PROXMox
VAL-PLUGIN-PROXY_SERVER
VAL-PLUGIN-QDRANT
VAL-PLUGIN-ROVS_COMMANDS
VAL-PLUGIN-RTNETLINK
VAL-PLUGIN-S6
VAL-PLUGIN-S6_SYSTEMCTL
VAL-PLUGIN-SCHEMA_CONTRACT
VAL-PLUGIN-SCHEMA_RENDERER
VAL-PLUGIN-SCHEMARS_ADAPTER
VAL-PLUGIN-SERVICE
VAL-PLUGIN-SESSDECL
VAL-PLUGIN-SHARED_UNIX_SOCKET
VAL-PLUGIN-SOFTWARE
VAL-PLUGIN-SYSTEMD_NETWORKD
VAL-PLUGIN-SYSTEMD
VAL-PLUGIN-UNIX_SOCKET
VAL-PLUGIN-USERS
VAL-PLUGIN-WEB_UI
VAL-PLUGIN-WGCF
VAL-PLUGIN-WIREGUARD
VAL-PLUGIN-WORKFLOWS_PLUGIN
VAL-PLUGIN-XRAY
VAL-PLUGIN-ZEROCLAW
```

---

*Each assertion is validated by running `cargo check` for the workspace and confirming that the generated schema struct for the plugin contains fields representing the required D‑Bus method signatures.*
