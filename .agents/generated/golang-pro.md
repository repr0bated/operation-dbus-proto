---
name: golang-pro
description: "Go development environment with build, test, and analysis tools"
model: sonnet
category: Execution
generated: true
---

# Go Pro Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Go development environment with build, test, and analysis tools

## Capabilities

- build — (execution agent operation)
- test — (execution agent operation)
- fmt — (execution agent operation)
- vet — (execution agent operation)
- run — (execution agent operation)

## Behavioral Traits

- Category: Execution
- Agent type: golang-pro

## Knowledge Base

- `build` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `fmt` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `vet` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))})}), "additionalProperties": Static(Bool(true))}
- `run` input: {"type": String("object"), "properties": Object({"path": Object({"type": String("string"), "description": String("Working directory or target path")}), "args": Object({"type": String("object"), "description": String("Operation-specific arguments"), "additionalProperties": Static(Bool(true))}), "timeout": Object({"type": String("integer"), "description": String("Optional timeout in seconds"), "minimum": Static(I64(1))}), "code": Object({"type": String("string"), "description": String("Source code to execute (language runners)")}), "release": Object({"type": String("boolean"), "description": String("Build/run in release mode when applicable")})}), "additionalProperties": Static(Bool(true))}
