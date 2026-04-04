import { api } from "@/api/client";
import type { HealthResponse, StatusResponse, ToolInfo, LlmStatus, LlmModel, AgentInfo } from "@/api/types";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

export function useHealth() {
  return useQuery<HealthResponse>({ queryKey: ["health"], queryFn: api.getHealth, refetchInterval: 10000 });
}

export function useStatus() {
  return useQuery<StatusResponse>({ queryKey: ["status"], queryFn: api.getStatus, refetchInterval: 5000 });
}

export function useTools() {
  return useQuery<ToolInfo[]>({ queryKey: ["tools"], queryFn: api.getTools });
}

export function useTool(name: string) {
  return useQuery<ToolInfo>({ queryKey: ["tool", name], queryFn: () => api.getTool(name), enabled: !!name });
}

export function useToolExecution() {
  return useMutation({
    mutationFn: ({ name, args }: { name: string; args: Record<string, unknown> }) =>
      api.executeTool(name, { arguments: args }),
  });
}

export function useLlmStatus() {
  return useQuery<LlmStatus>({ queryKey: ["llm-status"], queryFn: api.getLlmStatus, refetchInterval: 15000 });
}

export function useLlmModels() {
  return useQuery<LlmModel[]>({ queryKey: ["llm-models"], queryFn: api.getLlmModels });
}

export function useSetLlmModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (model: string) => api.setLlmModel(model),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["llm-status"] }),
  });
}

export function useAgents() {
  return useQuery<AgentInfo[]>({ queryKey: ["agents"], queryFn: api.getAgents });
}
