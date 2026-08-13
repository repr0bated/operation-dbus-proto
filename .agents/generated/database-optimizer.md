---
name: database-optimizer
description: Database query optimization and analysis
model: sonnet
category: Persona
generated: true
---

# Database Optimizer

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Database query optimization and analysis

## Capabilities

- explain — (persona consult/review)
- indexes — (persona consult/review)
- stats — (persona consult/review)

## Behavioral Traits

- Category: Persona
- Agent type: database-optimizer

## Knowledge Base

- `explain` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `indexes` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `stats` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
