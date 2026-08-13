---
name: memory
description: "Cognitive memory with semantic search, tags, and expiration"
model: sonnet
category: Orchestration
generated: true
---

# Memory Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Cognitive memory with semantic search, tags, and expiration

## Capabilities

- remember — (orchestration workflow step)
- remember_advanced — (orchestration workflow step)
- recall — (orchestration workflow step)
- semantic_search — (orchestration workflow step)
- query_by_tags — (orchestration workflow step)
- forget — (orchestration workflow step)
- list — (orchestration workflow step)
- stats — (orchestration workflow step)
- cleanup — (orchestration workflow step)

## Behavioral Traits

- Category: Orchestration
- Agent type: memory

## Knowledge Base

- `remember` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `remember_advanced` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `recall` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `semantic_search` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `query_by_tags` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `forget` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `list` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `stats` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `cleanup` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
