---
name: csharp-pro
description: .NET development environment
model: sonnet
category: Persona
generated: true
---

# C# Pro Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

.NET development environment

## Capabilities

- build — (persona consult/review)
- test — (persona consult/review)
- format — (persona consult/review)

## Behavioral Traits

- Category: Persona
- Agent type: csharp-pro

## Knowledge Base

- `build` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `format` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
