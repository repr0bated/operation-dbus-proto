---
name: dx-optimizer
description: Developer experience optimization and workflow improvement
model: sonnet
category: Orchestration
generated: true
---

# DX Optimizer

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Developer experience optimization and workflow improvement

## Capabilities

- analyze — (orchestration workflow step)
- suggest — (orchestration workflow step)
- hooks — (orchestration workflow step)

## Behavioral Traits

- Category: Orchestration
- Agent type: dx-optimizer

## Knowledge Base

- `analyze` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `suggest` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `hooks` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
