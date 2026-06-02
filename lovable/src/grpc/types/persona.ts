/**
 * Agent Orchestration / Persona types (operation.agents.v1)
 * @see docs/architecture-flow.md §6
 */

export interface PersonaDefinition {
  name: string;
  systemPrompt: string;
  model: string;           // e.g. "openclaw:reasoner-claude-sonnet"
  tools: string[];
  tags: string[];
  description?: string;
}

export interface ListPersonasResponse {
  personas: PersonaDefinition[];
  source: string;          // e.g. "config/agents/personas.yaml"
}

export interface GetPersonaRequest {
  name: string;
}

export interface GetPersonaResponse {
  persona: PersonaDefinition;
  activeSessions: number;
  lastInvokedAt?: string;
}

export interface CreatePersonaRequest {
  persona: PersonaDefinition;
}

export interface CreatePersonaResponse {
  persona: PersonaDefinition;
  created: boolean;
}

export interface UpdatePersonaRequest {
  name: string;
  persona: Partial<PersonaDefinition>;
}

export interface UpdatePersonaResponse {
  persona: PersonaDefinition;
  updated: boolean;
}

export interface DeletePersonaRequest {
  name: string;
}

export interface DeletePersonaResponse {
  deleted: boolean;
}

// ── OpenClaw agent routing ──────────────────────────────────────────────────

export interface AgentRoute {
  modelString: string;     // e.g. "openclaw:embedder-voyage4lite"
  resolvedModel: string;   // e.g. "voyage-4-lite"
  provider: string;        // e.g. "Voyage API"
  fallback?: string;       // e.g. "op-ml local ONNX"
  activeRequests: number;
}

export interface ListAgentRoutesResponse {
  routes: AgentRoute[];
}
