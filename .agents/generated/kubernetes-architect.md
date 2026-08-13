---
name: kubernetes-architect
description: Kubernetes cluster management and analysis
model: sonnet
category: Persona
generated: true
---

# Kubernetes Architect

<!-- Generated from crates/op-agents/src/agent_catalog.rs. Do not edit by hand;
     edit the agent's Rust implementation and re-run gen-agent-defs. -->

## Purpose

Kubernetes cluster management and analysis

## Capabilities

- get — (persona consult/review)
- describe — (persona consult/review)
- logs — (persona consult/review)
- helm-list — (persona consult/review)

## Behavioral Traits

- Category: Persona
- Agent type: kubernetes-architect

## Knowledge Base

- `get` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `describe` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `logs` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
- `helm-list` input: {"type": String("object"), "properties": Object({"query": Object({"type": String("string"), "description": String("User query / prompt to augment with persona expertise")}), "context": Object({"type": String("string"), "description": String("Optional conversation or session context")}), "args": Object({"type": String("object"), "additionalProperties": Static(Bool(true))})}), "additionalProperties": Static(Bool(true))}
