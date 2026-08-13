---
name: ruby-pro
description: Ruby development environment with RSpec and Rubocop
model: sonnet
category: Persona
generated: true
---

# Ruby Pro Agent

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Ruby development environment with RSpec and Rubocop

## Capabilities

- run — (persona consult/review)
- test — (persona consult/review)
- lint — (persona consult/review)

## Behavioral Traits

- Category: Persona
- Agent type: ruby-pro

## Knowledge Base

- `run` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `test` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `lint` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
