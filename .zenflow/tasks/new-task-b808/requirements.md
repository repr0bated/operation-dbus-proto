# Product Requirements Document: Agent Orchestration & Dynamic Personas Refactor

## 1. Executive Summary
The system currently possesses a robust conceptual architecture for coordinating AI agents, managing their connections, and exposing their capabilities. However, a significant portion of this architecture relies on simulated execution paths and statically compiled agent personas. The goal of this initiative is to bridge the existing client interfaces to the actual backend execution engine and to migrate over 50 hardcoded agent personas into a dynamic, configuration-driven system. This will unlock the true functionality of the agent orchestration framework, enabling scalable, real-world operation.

## 2. Goals & Objectives
- **End-to-End Execution**: Connect the frontend orchestrator to the backend agent execution engine, replacing all simulated placeholder responses with real data processing and streaming over the established transport protocols.
- **Dynamic Configuration**: Replace statically compiled agent personas with a configuration-driven approach, allowing agents to be modified, tuned, or added without requiring codebase recompilation.
- **Reliable Session Management**: Ensure that user sessions correctly initialize, manage, and tear down the appropriate background agents dynamically based on configuration.
- **Robust Error Handling**: Ensure that all connection failures, missing agents, and processing errors are accurately reported back to the user without silent degradation or fallback to simulations.

## 3. Scope
**In Scope:**
- Full implementation of the client-to-server transport mechanism for agent orchestration.
- Development of the server-side dispatch logic to route incoming execution requests to the correct active agents based on a central registry.
- Migration of all existing persona agents (over 50) from static code structures to a data-driven configuration model (e.g., YAML/JSON) without losing any existing capabilities, operational definitions, or identity details.
- Integration of session lifecycle events to dynamically provision and clean up required background agents.
- Propagation of all orchestration, connection, and agent-level errors explicitly to the caller.

**Out of Scope:**
- Modifications to D-Bus tool integration or the MCP server (`op-mcp`).
- Changes to the underlying LLM tool-call pipeline (`ForcedToolPipeline`).
- Updates to `op-blockchain`, `op-state`, or frontend applications.
- Changing the underlying communication framework versions or upgrading dependencies.

## 4. Functional Requirements

### 4.1. True Transport Implementation
- The client application must establish a real, persistent connection to the agent backend service, completely removing simulation logic.
- The client must be capable of dispatching execution commands to the backend and receiving both complete responses and streamed, chunked responses in real-time.
- The client must be capable of sending session initialization and termination requests directly to the backend over the transport layer.

### 4.2. Backend Agent Dispatch
- A backend service must actively listen for incoming execution requests from the client.
- Upon receiving a request, the backend must identify the targeted agent using a central runtime registry.
- The backend must route the execution payload to the identified agent, process the execution, and relay the final output (or stream) back to the client.
- The system must explicitly fail and report an error if a requested agent is unavailable or unregistered.

### 4.3. Data-Driven Persona Management
- The system must support loading agent personas dynamically from a centralized configuration file (or files) at startup.
- The configuration file must support defining an agent's identity (name, description, type), behaviors (operations, capabilities), security context, and core instructional prompts.
- A single, generic agent handler must be developed to represent any of the configuration-driven personas, feeding their definitions into the standard LLM execution pipeline.
- All existing 50+ persona definitions must be accurately captured in this new configuration format.

### 4.4. Session Lifecycle Wiring
- Starting a new user session must trigger a request to the backend to instantiate and activate a specific set of foundational, run-on-connection agents.
- The list of foundational agents to start upon connection must be dynamically resolved against the central registry (e.g., failing if an expected foundational agent is missing).
- Terminating a session must correctly communicate with the backend to release agent resources and clean up state.

### 4.5. Error Propagation
- The system must bubble up all execution, transport, and validation errors (e.g., timeouts, unavailable agents, network failures) explicitly to the user or caller.
- The system must not fallback to simulated success responses when an error occurs.

## 5. Assumptions & Clarifications
- The existing underlying tool execution pipeline (`ForcedToolPipeline`) and coordination strategies are functionally complete and will correctly execute tasks once the transport and dispatch bridge is established.
- A single generic agent struct can adequately handle the operations for all 50+ existing personas by injecting their specific prompt definitions.
- The data format (YAML or JSON) chosen for personas will natively support complex multi-line strings required for system prompts.