---
name: python-pro
description: Python 3.12+ development environment with modern tooling
model: sonnet
category: Execution
generated: true
---

# Python Pro Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Python 3.12+ development environment with modern tooling

## Capabilities

- run — (execution agent operation)
- test — (execution agent operation)
- lint — (execution agent operation)
- typecheck — (execution agent operation)
- format — (execution agent operation)

## Behavioral Traits

- Category: Execution
- Agent type: python-pro

## Knowledge Base

- `run` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))}), "code": Object({"type": String("string"), "description": String("Source code to execute (language runners)")}), "release": Object({"type": String("boolean"), "description": String("Build/run in release mode when applicable")})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `lint` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `typecheck` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `format` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
