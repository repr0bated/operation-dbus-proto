---
name: shell-executor
description: Executes whitelisted shell commands for system operations.
model: sonnet
category: Execution
generated: true
---

# Shell Executor

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Executes whitelisted shell commands for system operations.

## Capabilities

- exec — (execution agent operation)

## Behavioral Traits

- Category: Execution
- Agent type: shell-executor

## Knowledge Base

- `exec` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))}), "code": Object({"type": String("string"), "description": String("Source code to execute (language runners)")}), "release": Object({"type": String("boolean"), "description": String("Build/run in release mode when applicable")})}), "additionalProperties": Static(Bool(true))}
