---
name: javascript-executor
description: "Executes JavaScript/TypeScript via Node.js. Supports npm, pnpm, jest, and eslint."
model: sonnet
category: Execution
generated: true
---

# JavaScript/TypeScript Executor

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Executes JavaScript/TypeScript via Node.js. Supports npm, pnpm, jest, and eslint.

## Capabilities

- run — (execution agent operation)
- test — (execution agent operation)
- lint — (execution agent operation)
- format — (execution agent operation)
- typecheck — (execution agent operation)
- install — (execution agent operation)

## Behavioral Traits

- Category: Execution
- Agent type: javascript-executor

## Knowledge Base

- `run` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))}), "code": Object({"type": String("string"), "description": String("Source code to execute (language runners)")}), "release": Object({"type": String("boolean"), "description": String("Build/run in release mode when applicable")})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `lint` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `format` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `typecheck` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `install` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
