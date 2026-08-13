---
name: ml-engineer
description: "Train, optimize, and deploy machine learning models at scale."
model: sonnet
category: Persona
generated: true
---

# ML Engineer

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Train, optimize, and deploy machine learning models at scale.

## Capabilities

- train_model — (persona consult/review)
- optimize — (persona consult/review)
- deploy — (persona consult/review)
- analyze — (persona consult/review)

## Behavioral Traits

- Category: Persona
- Agent type: ml-engineer

## Knowledge Base

- `train_model` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `optimize` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `deploy` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `analyze` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
