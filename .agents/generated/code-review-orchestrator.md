---
name: code-review-orchestrator
description: Coordinates comprehensive code review with multiple expert agents
model: sonnet
category: Orchestration
generated: true
---

# Code Review Orchestrator

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Coordinates comprehensive code review with multiple expert agents

## Capabilities

- run_workflow — (orchestration workflow step)
- list_steps — (orchestration workflow step)
- validate — (orchestration workflow step)

## Behavioral Traits

- Category: Orchestration
- Agent type: code-review-orchestrator

## Knowledge Base

- `run_workflow` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")}), "workflow": Object({"type": String("string"), "description": String("Named workflow to run")})}), "additionalProperties": Static(Bool(true))}
- `list_steps` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
- `validate` input: {"type": String("object"), "properties": Object({"args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))}), "context": Object({"type": String("string")})}), "additionalProperties": Static(Bool(true))}
