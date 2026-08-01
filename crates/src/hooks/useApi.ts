import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchHealth,
  fetchStatus,
  fetchTools,
  fetchTool,
  executeTool,
  fetchAgents,
  fetchAgent,
  spawnAgent,
  fetchLlmStatus,
  fetchLlmModels,
  switchModel,
  fetchConfig,
} from "@/api/client";

// ── Health ────────────────────────────────────────────────────

export function useHealth() {
  return useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
    refetchInterval: 10_000,
    retry: 2,
  });
}

// ── Status ────────────────────────────────────────────────────

export function useStatus() {
  return useQuery({
    queryKey: ["status"],
    queryFn: fetchStatus,
    refetchInterval: 5_000,
    retry: 2,
  });
}

// ── Tools ─────────────────────────────────────────────────────

export function useTools() {
  return useQuery({
    queryKey: ["tools"],
    queryFn: fetchTools,
  });
}

export function useTool(name: string) {
  return useQuery({
    queryKey: ["tool", name],
    queryFn: () => fetchTool(name),
    enabled: !!name,
  });
}

export function useExecuteTool() {
  return useMutation({
    mutationFn: ({ toolName, args }: { toolName: string; args: Record<string, unknown> }) =>
      executeTool(toolName, args),
  });
}

// ── Agents ────────────────────────────────────────────────────

export function useAgents() {
  return useQuery({
    queryKey: ["agents"],
    queryFn: fetchAgents,
  });
}

export function useAgent(id: string) {
  return useQuery({
    queryKey: ["agent", id],
    queryFn: () => fetchAgent(id),
    enabled: !!id,
  });
}

export function useSpawnAgent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (agentType: string) => spawnAgent(agentType),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["agents"] }),
  });
}

// ── LLM ───────────────────────────────────────────────────────

export function useLlmStatus() {
  return useQuery({
    queryKey: ["llm", "status"],
    queryFn: fetchLlmStatus,
    refetchInterval: 15_000,
  });
}

export function useLlmModels() {
  return useQuery({
    queryKey: ["llm", "models"],
    queryFn: fetchLlmModels,
  });
}

export function useSwitchModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (model: string) => switchModel(model),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["llm"] }),
  });
}

// ── Config ────────────────────────────────────────────────────

export function useConfig() {
  return useQuery({
    queryKey: ["config"],
    queryFn: fetchConfig,
  });
}
