---
name: typescript-pro
description: TypeScript development environment
model: sonnet
category: Execution
generated: true
---

# TypeScript Pro Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

TypeScript development environment

## Capabilities

- check — (execution agent operation)
- build — (execution agent operation)
- test — (execution agent operation)
- lint — (execution agent operation)

## Behavioral Traits

- Category: Execution
- Agent type: typescript-pro

## Knowledge Base

- `check` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `build` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `lint` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
