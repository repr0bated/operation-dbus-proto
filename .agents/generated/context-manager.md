---
name: context-manager
description: Session context management and state persistence
model: sonnet
category: Orchestration
generated: true
---

# Context Manager

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Session context management and state persistence

## Capabilities

- save — (orchestration workflow step)
- restore — (orchestration workflow step)
- list — (orchestration workflow step)
- clear — (orchestration workflow step)

## Behavioral Traits

- Category: Orchestration
- Agent type: context-manager

## Knowledge Base

- `save` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `restore` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `list` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `clear` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
