---
name: tdd-orchestrator
description: Test-Driven Development workflow orchestration
model: sonnet
category: Orchestration
generated: true
---

# TDD Orchestrator

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Test-Driven Development workflow orchestration

## Capabilities

- red — (orchestration workflow step)
- green — (orchestration workflow step)
- refactor — (orchestration workflow step)
- cycle — (orchestration workflow step)

## Behavioral Traits

- Category: Orchestration
- Agent type: tdd-orchestrator

## Knowledge Base

- `red` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `green` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `refactor` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `cycle` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
