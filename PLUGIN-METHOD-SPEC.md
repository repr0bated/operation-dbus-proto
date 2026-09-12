# Plugin Method Spec — Section B (method-less plugins)

Researched method surfaces for the kept Section-B plugins, derived from each
service's authoritative SDK/API spec (or in-repo contract). Every method becomes
`method_decl_from_schemars::<Input, Output>(DBusName, side_effect, idempotent, capability, subid)`
with typed `Input`/`Output` structs (derive `schemars::JsonSchema, Serialize, Deserialize`).
Returns are derived from `schema_for::<Output>()` — **no hardcoded `{"type":"object"}`**.
`Ack { success: bool }` is the typed fallback only where no domain return exists.

Legend: R=Read, M=Mutation. cap = required_capability.

---

## keyring  (Secret Service API — freedesktop.org D-Bus)
Source: https://specifications.freedesktop.org/secret-service/latest-single/

| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| OpenSession | OpenSessionInput→OpenSessionOutput | M | n | keyring.read | mut.security.keyring.session.open@v1 |
| SearchItems | attributes→{unlocked,locked} | R | y | keyring.read | obs.security.keyring.item.search@v1 |
| ListCollections | ()→Vec<CollectionInfo> | R | y | keyring.read | obs.security.keyring.collection.list@v1 |
| Unlock | objects→{unlocked,prompt} | M | y | keyring.write | mut.security.keyring.object.unlock@v1 |
| Lock | objects→{locked,prompt} | M | y | keyring.write | mut.security.keyring.object.lock@v1 |
| GetSecrets | {items,session}→secrets map | R | y | keyring.read | obs.security.keyring.secret.get@v1 |
| CreateItem | {collection,label,attrs,secret,replace}→{item,prompt} | M | n | keyring.write | mut.security.keyring.item.create@v1 |
| DeleteItem | item_path→prompt | M | y | keyring.write | mut.security.keyring.item.delete@v1 |

## adc  (Google ADC / gcloud auth application-default)
Source: https://cloud.google.com/sdk/gcloud/reference/auth/application-default — front via D-Bus, not direct spawn.

| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetStatus | ()→{configured,credential_path,account,quota_project} | R | y | adc.read | obs.software.adc.status.get@v1 |
| PrintAccessToken | ()→{access_token,token_expiry} | R | n | adc.read | obs.software.adc.token.print@v1 |
| ListAccounts | ()→Vec<AdcAccount> | R | y | adc.read | obs.software.adc.account.list@v1 |
| Login | {account,scopes}→Ack | M | n | adc.write | mut.software.adc.credential.login@v1 |
| Revoke | ()→Ack | M | y | adc.write | mut.software.adc.credential.revoke@v1 |
| SetQuotaProject | project_id→Ack | M | y | adc.write | mut.software.adc.quota-project.set@v1 |

## cron  (in-repo agent scheduler; POSIX 5-field expr)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListJobs | ()→Vec<CronJob> | R | y | cron.read | obs.service.cron.job.list@v1 |
| GetJob | id→CronJob | R | y | cron.read | obs.service.cron.job.get@v1 |
| AddJob | {name,schedule,agent_id,enabled}→CronJob | M | n | cron.write | mut.service.cron.job.add@v1 |
| UpdateJob | {id,...opts}→CronJob | M | y | cron.write | mut.service.cron.job.update@v1 |
| RemoveJob | id→Ack | M | y | cron.write | mut.service.cron.job.remove@v1 |
| EnableJob | id→{success,enabled} | M | y | cron.write | mut.service.cron.job.enable@v1 |
| DisableJob | id→{success,enabled} | M | y | cron.write | mut.service.cron.job.disable@v1 |
| GetNextRun | id→{next_run,last_run} | R | y | cron.read | obs.service.cron.job.next-run@v1 |
| TriggerJob (opt) | id→Ack | M | n | cron.write | mut.service.cron.job.trigger@v1 |

---

## hardware  (procfs/sysfs; all read-only)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetHardware | ()→HardwareState | R | y | hardware.read | obs.hardware.plugin.hardware.snapshot.get@v1 |
| GetCpu | ()→CpuInfo | R | y | hardware.read | obs.hardware.plugin.hardware.cpu.get@v1 |
| GetMemory | ()→MemoryInfo | R | y | hardware.read | obs.hardware.plugin.hardware.memory.get@v1 |
| ListDisks | ()→DiskList | R | y | hardware.read | obs.hardware.plugin.hardware.disks.list@v1 |
| GetDisk | name→DiskInfo | R | y | hardware.read | obs.hardware.plugin.hardware.disk.get@v1 |

## full_system  (procfs/sysfs + systemd1 D-Bus + OVS)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| CaptureFullState | ()→FullSystemState | R | n | full_system.read | obs.system.plugin.full-system.snapshot.capture@v1 |
| GetSystemInfo | ()→SystemInfo | R | y | full_system.read | obs.system.plugin.full-system.system-info.get@v1 |
| GetNetworkState | ()→NetworkState | R | y | full_system.read | obs.network.plugin.full-system.network-state.get@v1 |
| ListServices | ()→ServiceList | R | y | full_system.read | obs.service.plugin.full-system.services.list@v1 |
| ListPackages | ()→PackageList | R | y | full_system.read | obs.software.plugin.full-system.packages.list@v1 |
| ListUsers | ()→UserList | R | y | full_system.read | obs.system.plugin.full-system.users.list@v1 |
| GetStorage | ()→StorageState | R | y | full_system.read | obs.hardware.plugin.full-system.storage.get@v1 |
| GetContainers | ()→ContainerState | R | y | full_system.read | obs.service.plugin.full-system.containers.get@v1 |
| SetHostname | hostname→Ack | M | y | full_system.write | mut.system.plugin.full-system.hostname.set@v1 |

## config  (in-repo k/v config store)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetAllConfigs | ()→ConfigSchemaState | R | y | config.read | obs.software.plugin.config.snapshot.get@v1 |
| ListConfigKeys | ()→{keys} | R | y | config.read | obs.software.plugin.config.keys.list@v1 |
| GetConfig | key→ConfigEntry | R | y | config.read | obs.software.plugin.config.entry.get@v1 |
| SetConfig | {key,value}→Ack | M | y | config.write | mut.software.plugin.config.entry.set@v1 |
| DeleteConfig | key→Ack | M | y | config.write | mut.software.plugin.config.entry.delete@v1 |

---

## large_language_model  (Ollama HTTP API / OpenAI-compatible)
Source: https://github.com/ollama/ollama/blob/main/docs/api.md

| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListModels | ()→ModelList | R | y | llm.read | obs.service.plugin.large-language-model.list-models@v1 |
| ModelStatus | ()→ProviderStatus | R | y | llm.read | obs.service.plugin.large-language-model.model-status@v1 |
| Generate | GenerateInput→GenerateOutput | M | n | llm.invoke | mut.service.plugin.large-language-model.generate@v1 |
| Chat | ChatInput→ChatOutput | M | n | llm.invoke | mut.service.plugin.large-language-model.chat@v1 |
| Embed | EmbedInput→EmbedOutput | R | y | llm.embed | obs.service.plugin.large-language-model.embed@v1 |
| SetActiveModel | model_id→Ack | M | y | llm.model.set | mut.service.plugin.large-language-model.set-active-model@v1 |
| LoadModel | {model,keep_alive}→Ack | M | y | llm.model.load | mut.service.plugin.large-language-model.load-model@v1 |
| UnloadModel | model→Ack | M | y | llm.model.unload | mut.service.plugin.large-language-model.unload-model@v1 |

## gemma_brain  (op-gemma orchestration; inference delegated to LLM plugin)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| BrainStatus | ()→BrainStatus | R | y | llm.read | obs.service.plugin.gemma-brain.brain-status@v1 |
| Reason | ReasonInput→ReasonOutput | M | n | llm.invoke | mut.service.plugin.gemma-brain.reason@v1 |
| ClassifySubid | {subject,context}→SubidClassification | M | n | llm.invoke | mut.service.plugin.gemma-brain.classify-subid@v1 |
| RouteTags | {flow_id,attributes}→RouteDecision | M | n | llm.invoke | mut.service.plugin.gemma-brain.route-tags@v1 |
| ListSpecs | {limit}→SpecList | R | y | llm.read | obs.service.plugin.gemma-brain.list-specs@v1 |
| RegenerateGallery | {target_count}→Ack | M | n | llm.invoke | mut.service.plugin.gemma-brain.regenerate-gallery@v1 |
| DeleteSpec | id→Ack | M | y | llm.invoke | mut.service.plugin.gemma-brain.delete-spec@v1 |

## notebooklm  (NotebookLM MCP — 40+ methods)
Sources: jacob-bd/notebooklm-mcp-cli (35 MCP tools + full `nlm` CLI), PleasePrompto/notebooklm-mcp (library/session mgmt). NotebookLM has NO official consumer REST API; surface is the `nlm` internal-API/automation client. component-type = service. cap: read=`notebooklm.read`, write/invoke=`notebooklm.invoke`, admin/auth=`notebooklm.admin`.

Notebooks:
| method | side | subid |
|---|---|---|
| ListNotebooks | R | obs.service.plugin.notebooklm.notebook.list@v1 |
| GetNotebook | R | obs.service.plugin.notebooklm.notebook.get@v1 |
| CreateNotebook | M | mut.service.plugin.notebooklm.notebook.create@v1 |
| UpdateNotebook | M | mut.service.plugin.notebooklm.notebook.update@v1 |
| DeleteNotebook | M | mut.service.plugin.notebooklm.notebook.delete@v1 |
| SelectNotebook | M | mut.service.plugin.notebooklm.notebook.select@v1 |
| SearchNotebooks | R | obs.service.plugin.notebooklm.notebook.search@v1 |
| QueryNotebook (ask) | R | obs.service.plugin.notebooklm.notebook.query@v1 |
| GetLibraryStats | R | obs.service.plugin.notebooklm.library.stats@v1 |
| CrossNotebookQuery | R | obs.service.plugin.notebooklm.cross.query@v1 |

Sources:
| method | side | subid |
|---|---|---|
| ListSources | R | obs.service.plugin.notebooklm.source.list@v1 |
| GetSourceContent | R | obs.service.plugin.notebooklm.source.content@v1 |
| AddSourceUrl | M | mut.service.plugin.notebooklm.source.add-url@v1 |
| AddSourceText | M | mut.service.plugin.notebooklm.source.add-text@v1 |
| AddSourceDrive | M | mut.service.plugin.notebooklm.source.add-drive@v1 |
| AddSourceFile | M | mut.service.plugin.notebooklm.source.add-file@v1 |
| DeleteSource | M | mut.service.plugin.notebooklm.source.delete@v1 |
| SyncDriveSources | M | mut.service.plugin.notebooklm.source.sync-drive@v1 |
| AutoLabelSources | M | mut.service.plugin.notebooklm.source.label-auto@v1 |
| CreateLabel | M | mut.service.plugin.notebooklm.source.label-create@v1 |
| RenameLabel | M | mut.service.plugin.notebooklm.source.label-rename@v1 |
| SetLabelEmoji | M | mut.service.plugin.notebooklm.source.label-emoji@v1 |
| MoveSourceLabel | M | mut.service.plugin.notebooklm.source.label-move@v1 |
| DeleteLabel | M | mut.service.plugin.notebooklm.source.label-delete@v1 |

Studio content + download:
| method | side | subid |
|---|---|---|
| CreateAudio (podcast) | M | mut.service.plugin.notebooklm.studio.audio.create@v1 |
| CreateVideo | M | mut.service.plugin.notebooklm.studio.video.create@v1 |
| CreateReport | M | mut.service.plugin.notebooklm.studio.report.create@v1 |
| CreateQuiz | M | mut.service.plugin.notebooklm.studio.quiz.create@v1 |
| CreateFlashcards | M | mut.service.plugin.notebooklm.studio.flashcards.create@v1 |
| CreateInfographic | M | mut.service.plugin.notebooklm.studio.infographic.create@v1 |
| CreateMindMap | M | mut.service.plugin.notebooklm.studio.mindmap.create@v1 |
| CreateSlides | M | mut.service.plugin.notebooklm.studio.slides.create@v1 |
| ReviseSlides | M | mut.service.plugin.notebooklm.studio.slides.revise@v1 |
| DescribeStudio | R | obs.service.plugin.notebooklm.studio.describe@v1 |
| GetAudioStatus | R | obs.service.plugin.notebooklm.studio.audio.status@v1 |
| ListArtifacts | R | obs.service.plugin.notebooklm.studio.artifact.list@v1 |
| DownloadArtifact | R | obs.service.plugin.notebooklm.studio.artifact.download@v1 |

Research / share / batch / pipeline / tag:
| method | side | subid |
|---|---|---|
| StartResearch | M | mut.service.plugin.notebooklm.research.start@v1 |
| ImportResearch | M | mut.service.plugin.notebooklm.research.import@v1 |
| SharePublic | M | mut.service.plugin.notebooklm.share.public@v1 |
| ShareInvite | M | mut.service.plugin.notebooklm.share.invite@v1 |
| GetShareSettings | R | obs.service.plugin.notebooklm.share.settings@v1 |
| DisableShare | M | mut.service.plugin.notebooklm.share.disable@v1 |
| BatchOperation | M | mut.service.plugin.notebooklm.batch.run@v1 |
| RunPipeline | M | mut.service.plugin.notebooklm.pipeline.run@v1 |
| ListPipelines | R | obs.service.plugin.notebooklm.pipeline.list@v1 |
| TagAdd | M | mut.service.plugin.notebooklm.tag.add@v1 |
| TagList | R | obs.service.plugin.notebooklm.tag.list@v1 |
| TagSmartSelect | R | obs.service.plugin.notebooklm.tag.select@v1 |

Sessions / auth / health:
| method | side | subid |
|---|---|---|
| ListSessions | R | obs.service.plugin.notebooklm.session.list@v1 |
| CloseSession | M | mut.service.plugin.notebooklm.session.close@v1 |
| ResetSession | M | mut.service.plugin.notebooklm.session.reset@v1 |
| GetHealth | R | obs.service.plugin.notebooklm.health@v1 |
| SetupAuth | M | mut.service.plugin.notebooklm.auth.setup@v1 |
| RefreshAuth | M | mut.service.plugin.notebooklm.auth.refresh@v1 |
| ReAuth | M | mut.service.plugin.notebooklm.auth.reauth@v1 |

(Total: 50 methods. Typed Input/Output structs per method to be defined from the
`nlm`/notebooklm-py request/response shapes during implementation.)

---

## cognitive_mcp  (MCP gateway :3003; absorbs deprecated `mcp` registry)
Source: https://modelcontextprotocol.io/specification + in-repo CognitiveMcpState + mcp.rs registry.
Gating: chatbot=read-only (`mcp.read`); local agents=execute (`mcp.invoke`); admin (`mcp.admin`).

| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListTools | {cursor,group}→ListToolsOutput | R | y | mcp.read | exp.service.plugin.cognitive-mcp.tools.list@v1 |
| GetToolSchema | name→ToolDescriptor | R | y | mcp.read | sch.service.plugin.cognitive-mcp.tool.resolve@v1 |
| CallTool | {name,arguments}→ToolCallOutput | M | n | mcp.invoke | exp.service.plugin.cognitive-mcp.tool.call@v1 |
| MemoryRead | MemoryToolInput→MemoryReadOutput | R | y | mcp.read | obs.service.plugin.cognitive-mcp.memory.query@v1 |
| MemoryWrite | MemoryToolInput→Ack | M | n | mcp.invoke | mut.service.plugin.cognitive-mcp.memory.mutate@v1 |
| CodeSearch | CodeSearchInput→CodeSearchOutput | R | y | mcp.read | obs.service.code-rag.search@v1 |
| CodeContext | CodeContextInput→CodeSearchOutput | R | y | mcp.read | exp.service.code-context.render@v1 |
| IndexCode | CodeIndexInput→IndexCodeOutput | M | n | mcp.invoke | src.software.workspace.index@v1 |
| GeminiQuery | GeminiQueryRequest→GeminiQueryOutput | R | n | mcp.invoke | exp.service.plugin.cognitive-mcp.gemini.query@v1 |
| GetHealth | ()→CognitiveHealthOutput | R | y | mcp.read | obs.service.plugin.cognitive-mcp.health@v1 |
| ApplyConfig | CognitiveMcpConfig→Ack | M | y | mcp.admin | mut.software.plugin.cognitive-mcp.config.apply@v1 |
| ListServers (merged) | ()→ListServersOutput | R | y | mcp.read | obs.service.plugin.cognitive-mcp.registry.servers.list@v1 |
| RegisterServer (merged) | {name,server}→Ack | M | y | mcp.admin | mut.service.plugin.cognitive-mcp.registry.server.register@v1 |
| RemoveServer (merged) | name→Ack | M | y | mcp.admin | mut.service.plugin.cognitive-mcp.registry.server.remove@v1 |
| ConfigureToolGroups (merged) | ToolGroupsConfig→Ack | M | y | mcp.admin | mut.service.plugin.cognitive-mcp.registry.tool-groups.configure@v1 |

## compact_mcp  (loopback :11436; meta-tools; chatbot no-execute)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListTools | {cursor}→ListToolsOutput | R | y | mcp.read | exp.service.plugin.compact-mcp.tools.list@v1 |
| SearchTools | {query,limit}→ListToolsOutput | R | y | mcp.read | obs.service.plugin.compact-mcp.tools.search@v1 |
| GetToolSchema | name→ToolDescriptor | R | y | mcp.read | sch.service.plugin.compact-mcp.tool.resolve@v1 |
| ExecuteTool | {name,arguments}→ToolCallOutput | M | n | mcp.invoke | exp.service.plugin.compact-mcp.tool.execute@v1 |
| Respond | {text,structured}→Ack | R | n | mcp.read | exp.service.plugin.compact-mcp.respond@v1 |
| GetHealth | ()→CompactHealthOutput | R | y | mcp.read | obs.service.plugin.compact-mcp.health@v1 |
| ApplyConfig | CompactMcpConfig→Ack | M | y | mcp.admin | mut.software.plugin.compact-mcp.config.apply@v1 |

---

## memory  (CognitiveMemoryStore backend)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetState | ()→MemoryState | R | y | memory.read | obs.software.plugin.memory.state@v1 |
| ListNamespaces | {kind}→NamespaceList | R | y | memory.read | obs.software.plugin.memory.namespaces.list@v1 |
| GetStats | ()→MemoryStatsView | R | y | memory.read | obs.software.plugin.memory.stats@v1 |
| GetConfig | ()→MemoryConfig | R | y | memory.read | obs.software.plugin.memory.config@v1 |
| Store | StoreInput→EntryView | M | y | memory.write | mut.software.plugin.memory.entry.store@v1 |
| Recall | RecallInput→RecallResult | R | y | memory.read | obs.software.plugin.memory.entry.recall@v1 |
| Forget | {namespace,key}→Ack | M | y | memory.write | mut.software.plugin.memory.entry.forget@v1 |

## datastore  (read-only projection of canonical op-state-store; obs.* only — OD-30)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetState | ()→DataStoreState | R | y | datastore.read | obs.software.plugin.datastore.state@v1 |
| ListIndex | {namespace}→ObjectIndex | R | y | datastore.read | obs.software.plugin.datastore.index.list@v1 |
| CountNamespaces | ()→NamespaceCounts | R | y | datastore.read | obs.software.plugin.datastore.namespaces.count@v1 |
| GetCounts | ()→StoreCounts | R | y | datastore.read | obs.software.plugin.datastore.counts@v1 |
| ExportCanonical | ()→CanonicalExportView | R | y | datastore.read | obs.software.plugin.datastore.export.canonical@v1 |

## snowball  (StreamingSnowball footprint ledger)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetState | ()→SnowballState | R | y | snowball.read | obs.software.plugin.snowball.state@v1 |
| GetFootprint | {key}→FootprintView | R | y | snowball.read | obs.software.plugin.snowball.footprint.read@v1 |
| VerifyHash | {event_hash,data}→VerifyResult | R | y | snowball.read | evt.software.plugin.snowball.footprint.verify@v1 |
| ListSnapshots | ()→SnapshotList | R | y | snowball.read | obs.software.plugin.snowball.snapshots.list@v1 |
| GetRetention | ()→RetentionConfig | R | y | snowball.read | obs.software.plugin.snowball.retention@v1 |
| AddFootprint | AddFootprintInput→FootprintAck{event_hash} | M | n | snowball.write | evt.software.plugin.snowball.footprint.append@v1 |
| CreateSnapshot | ()→SnapshotAck{event_id} | M | n | snowball.write | evt.software.plugin.snowball.snapshot.create@v1 |

---

## oscal_subid_registry  (OSCAL subid taxonomy — AGENTS.md §4a)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| RegisterSubid | {entry}→{uuid,subid,created} | M | n | oscal.write | mut.standard.plugin.oscal-subid-registry.register@v1 |
| ResolveSubid | {subid,uuid}→{entry} | R | y | oscal.read | sch.standard.plugin.oscal-subid-registry.resolve@v1 |
| ListSubids | {category,component_type,subject}→{entries,total} | R | y | oscal.read | obs.standard.plugin.oscal-subid-registry.list@v1 |
| ValidateSubid | {subid,category}→{valid,errors} | R | y | oscal.read | sch.standard.plugin.oscal-subid-registry.validate@v1 |
| DeregisterSubid | uuid→Ack | M | y | oscal.write | mut.standard.plugin.oscal-subid-registry.deregister@v1 |

## schema_renderer  (schema → GUI JSON render)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| RenderPlugin | {plugin_id,element_type,layout,mode}→{render} | R | y | schema.read | exp.software.plugin.schema-renderer.render@v1 |
| ListRenderable | {category}→{plugins} | R | y | schema.read | obs.software.plugin.schema-renderer.list@v1 |
| GetGallery | {group_by_category}→{gallery} | R | y | schema.read | exp.software.plugin.schema-renderer.gallery@v1 |
| GetRenderConfig | ()→{element_types,field_mappings,layouts,render_config,sub_views} | R | y | schema.read | exp.software.plugin.schema-renderer.render-config@v1 |

## agent_config  (agent configuration)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListAgents | ()→{agents} | R | y | agent.read | obs.software.plugin.agent-config.list@v1 |
| GetAgent | name→{agent} | R | y | agent.read | obs.software.plugin.agent-config.get@v1 |
| SetAgent | {agent}→{agent,created} | M | y | agent.write | mut.software.plugin.agent-config.set@v1 |
| SetAgentEnabled | {name,enabled}→Ack | M | y | agent.write | mut.software.plugin.agent-config.set-enabled@v1 |
| RemoveAgent | name→Ack | M | y | agent.write | mut.software.plugin.agent-config.remove@v1 |

## endpoint  (endpoint registry — host:port list)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| ListEndpoints | ()→{endpoints} | R | y | endpoint.read | obs.service.plugin.endpoint.list@v1 |
| GetEndpoint | endpoint→{present} | R | y | endpoint.read | obs.service.plugin.endpoint.get@v1 |
| RegisterEndpoint | endpoint→{endpoint,added} | M | y | endpoint.write | mut.service.plugin.endpoint.register@v1 |
| RemoveEndpoint | endpoint→Ack | M | y | endpoint.write | mut.service.plugin.endpoint.remove@v1 |

## factory  (Factory.ai API v0 — https://api.factory.ai/api/v0)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| CreateSession | {computer_id,session_settings}→{session_id,status} | M | n | factory.write | mut.service.plugin.factory.create-session@v1 |
| SendMessage | {session_id,text}→{session_id,accepted} | M | n | factory.write | mut.service.plugin.factory.send-message@v1 |
| ListSessions | {computer_id}→{sessions} | R | y | factory.read | obs.service.plugin.factory.list-sessions@v1 |
| CreateComputer | {name,provider}→{computer_id,status} | M | n | factory.write | mut.service.plugin.factory.create-computer@v1 |
| ListModels | {family}→{catalog} | R | y | factory.read | obs.service.plugin.factory.list-models@v1 |
| ListByomSources | {provider,available_only}→{sources,discovery_status} | R | y | factory.read | exp.service.plugin.factory.byom-sources@v1 |

---

## antigravity  (Google Antigravity auth, usage, and safety surface)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| `get_auth_status` | ()→{auth} | R | y | antigravity.read | obs.software.antigravity.auth.status@v1 |
| `get_usage_report` | {project_id,model}→{usage} | R | y | antigravity.read | obs.software.antigravity.usage.report@v1 |
| `configure_safety` | {harassment?,hate_speech?,sexually_explicit?,dangerous?,civic_integrity?}→{safety_settings} | M | n | antigravity.write | mut.software.antigravity.safety.configure@v1 |

## antigravity_chat  (OAuth bridge + headless IDE)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| `get_bridge_status` | ()→{bridge} | R | y | antigravity_chat.read | obs.software.antigravity-chat.bridge.status.get@v1 |
| `refresh_token` | ()→{auth} | M | n | antigravity_chat.write | mut.software.antigravity-chat.auth.token.refresh@v1 |
| `configure` | {headless?,display_service?,vnc_port?,code_assist?,selected_model?}→{config,selected_model} | M | n | antigravity_chat.write | mut.software.antigravity-chat.config.set@v1 |

Both plugins reference their model catalog instead of owning one:
`llm_plugin="large_language_model"`, `provider_route="gemini"`, and an empty
`selected_model` means the provider default. Resolve model pickers by calling
`list_models` / `list_providers` on `large_language_model`; neither Antigravity
plugin declares `list_models`. `provider_route` is currently a schema/render
routing hint—the catalog plugin does not consume that field directly.

The `antigravity_chat` bridge currently reports `offline`; its methods are
schema-declared UI controls without a live headless-IDE bridge implementation.

## ctl_plane_chatbot  (read/query/confront only — NO execute power, AGENTS.md §4b)
| method | I/O | side | idem | cap | subid |
|---|---|---|---|---|---|
| GetStatus | ()→GetStatusOutput | R | y | chatbot.query | obs.service.plugin.ctl-plane-chatbot.status.get@v1 |
| ListEpisodes | ListEpisodesInput→ListEpisodesOutput | R | y | chatbot.query | obs.service.plugin.ctl-plane-chatbot.history.list@v1 |
| GetEpisode | episode_id→GetEpisodeOutput | R | y | chatbot.query | obs.service.plugin.ctl-plane-chatbot.episode.get@v1 |
| SearchReasoning | SearchReasoningInput→SearchReasoningOutput | R | y | chatbot.query | obs.service.plugin.ctl-plane-chatbot.reasoning.search@v1 |
| ExplainDecision | conversation_id→ExplainDecisionOutput | R | y | chatbot.query | obs.service.plugin.ctl-plane-chatbot.decision.explain@v1 |
| ConfigureVectorization | ConfigureVectorizationInput→Ack | M | y | chatbot.configure | mut.service.plugin.ctl-plane-chatbot.vectorization.configure@v1 |

---

### Notes
- subid uniqueness is CI-enforced; every method subid above must be registered in the canonical registry.
- `mut.*` records carry `actor_id` + `capability_id` (supplied by MutationEngine, not the Input struct); `evt.*` carry `event_id`/`event_hash`.
- Full per-method Input/Output field detail lives in the research agent reports; structs to be authored at implementation, derived from each cited SDK/API response shape.
- Capability strings shown short (e.g. `keyring.read`); confirm vs in-repo `cap.<comp>.<plugin>.<verb>@v1` form before finalizing.
