const API_BASE = "/api";

async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json", ...options?.headers },
    ...options,
  });
  if (!res.ok) throw new Error(`API error: ${res.status} ${res.statusText}`);
  return res.json();
}

// Dashboard
export const getPrivacyStatus = () => fetchApi<any>("/privacy/status");
export const getHealth = () => fetchApi<any>("/health");
export const getMailStats = () => fetchApi<any>("/mail/stats");
export const getMcpStatus = () => fetchApi<any>("/mcp/status");
export const getConnectionHistory = () => fetchApi<any>("/analytics/connections");
export const getHealthMetrics = () => fetchApi<any>("/health/metrics");
export const getRecentActivity = () => fetchApi<any>("/activity/recent");

// Chat
export const getChatSessions = () => fetchApi<any>("/chat/sessions");
export const createChatSession = (title?: string) =>
  fetchApi<any>("/chat/sessions", { method: "POST", body: JSON.stringify({ title }) });
export const deleteChatSession = (sessionId: string) =>
  fetchApi<any>(`/chat/sessions/${sessionId}`, { method: "DELETE" });
export const getChatMessages = (sessionId: string) => fetchApi<any>(`/chat/history/${sessionId}`);
export const sendChatMessage = (data: { session_id?: string; message: string }) =>
  fetchApi<any>("/chat", { method: "POST", body: JSON.stringify(data) });
export const getSystemPrompt = () => fetchApi<any>("/chat/system-prompt");
export const updateSystemPrompt = (prompt: string) =>
  fetchApi<any>("/chat/system-prompt", { method: "PUT", body: JSON.stringify({ prompt }) });
export const getSystemPromptTemplates = () => fetchApi<any>("/chat/system-prompt-templates");

// Users
export const getUsers = () => fetchApi<any>("/users/list");
export const createUser = (data: any) =>
  fetchApi<any>("/users", { method: "POST", body: JSON.stringify(data) });
export const deleteUser = (id: string | number) =>
  fetchApi<any>(`/users/${id}`, { method: "DELETE" });
export const revokeUser = (id: string | number) =>
  fetchApi<any>(`/users/${id}/revoke`, { method: "POST" });
export const getUserActivity = (id: string | number) =>
  fetchApi<any>(`/users/${id}/activity`);
export const getUserDetail = (id: string | number) =>
  fetchApi<any>(`/users/${id}`);

// VPN
export const getVpnConnections = () => fetchApi<any>("/vpn/connections");
export const getVpnConfig = () => fetchApi<any>("/vpn/config");
export const disconnectVpnUser = (id: string) =>
  fetchApi<any>(`/vpn/connections/${id}/disconnect`, { method: "POST" });

// Mail
export const getMailQueue = () => fetchApi<any>("/mail/queue");
export const getMailDnsStatus = () => fetchApi<any>("/mail/dns-status");
export const getRecentEmails = () => fetchApi<any>("/mail/recent");
export const getMailServerStatus = () => fetchApi<any>("/mail/server-status");
export const resendEmail = (id: string) =>
  fetchApi<any>(`/mail/${id}/resend`, { method: "POST" });
export const getEmailDetail = (id: string) => fetchApi<any>(`/mail/${id}`);

// MCP
export const getMcpServers = () => fetchApi<any>("/mcp/servers");
export const getMcpServerDetail = (id: string) => fetchApi<any>(`/mcp/servers/${id}`);
export const getMcpServerTools = (id: string) => fetchApi<any>(`/mcp/servers/${id}/tools`);
export const getMcpServerConfig = (id: string) => fetchApi<any>(`/mcp/servers/${id}/config`);
export const updateMcpServerConfig = (id: string, config: any) =>
  fetchApi<any>(`/mcp/servers/${id}/config`, { method: "PUT", body: JSON.stringify(config) });
export const getMcpServerLogs = (id: string) => fetchApi<any>(`/mcp/servers/${id}/logs`);
export const restartMcpServer = (id: string) =>
  fetchApi<any>(`/mcp/servers/${id}/restart`, { method: "POST" });

// MCP Cognitive Server
export const getMcpAgents = () => fetchApi<any>("/mcp/cognitive/agents");
export const setMcpAgents = (agentIds: string[]) =>
  fetchApi<any>("/mcp/cognitive/agents", { method: "POST", body: JSON.stringify({ agent_ids: agentIds }) });
export const queryMcpMemory = (query: any) =>
  fetchApi<any>("/mcp/cognitive/memory", { method: "POST", body: JSON.stringify(query) });
export const deleteMcpMemory = (key: string) =>
  fetchApi<any>(`/mcp/cognitive/memory/${key}`, { method: "DELETE" });
export const getMcpMemoryStats = () => fetchApi<any>("/mcp/cognitive/memory/stats");

// Analytics
export const getAnalytics = (type: string, range?: string) =>
  fetchApi<any>(`/analytics/${type}${range ? `?range=${range}` : ""}`);
export const getAnalyticsVpnTraffic = (range: string) => fetchApi<any>(`/analytics/vpn/traffic?period=${range}`);
export const getAnalyticsVpnUsers = () => fetchApi<any>("/analytics/vpn/users-by-status");
export const getAnalyticsVpnPeakHours = () => fetchApi<any>("/analytics/vpn/peak-hours");
export const getAnalyticsChatMessages = (range: string) => fetchApi<any>(`/analytics/chat/messages?period=${range}`);
export const getAnalyticsChatLengths = () => fetchApi<any>("/analytics/chat/conversation-lengths");
export const getAnalyticsChatIntents = () => fetchApi<any>("/analytics/chat/intents");
export const getAnalyticsMailVolume = (range: string) => fetchApi<any>(`/analytics/mail/volume?period=${range}`);
export const getAnalyticsMailDelivery = (range: string) => fetchApi<any>(`/analytics/mail/delivery-rate?period=${range}`);
export const getAnalyticsMailDomains = () => fetchApi<any>("/analytics/mail/top-domains");

// Settings
export const getSettings = (tab: string) => fetchApi<any>(`/settings/${tab}`);
export const updateSettings = (tab: string, data: any) =>
  fetchApi<any>(`/settings/${tab}`, { method: "PUT", body: JSON.stringify(data) });
export const getApiKeys = () => fetchApi<any>("/settings/api-keys");
export const createApiKey = (name: string) =>
  fetchApi<any>("/settings/api-keys", { method: "POST", body: JSON.stringify({ name }) });
export const revokeApiKey = (id: string) =>
  fetchApi<any>(`/settings/api-keys/${id}/revoke`, { method: "POST" });
export const sendTestEmail = () =>
  fetchApi<any>("/settings/smtp/test", { method: "POST" });
export const exportConfig = () => fetchApi<any>("/settings/backup/export");
export const backupDatabase = () =>
  fetchApi<any>("/settings/backup/database", { method: "POST" });

// Logs
export const getLogsStream = () => `${API_BASE}/logs/stream`;

// Tools
export const getBuiltinTools = () => fetchApi<any>("/tools/builtin");
export const getMcpTools = () => fetchApi<any>("/tools/mcp");
export const getCustomTools = () => fetchApi<any>("/tools/custom");
export const getToolHistory = (id: string) => fetchApi<any>(`/tools/${id}/history`);
export const executeTool = (id: string, input: any) =>
  fetchApi<any>(`/tools/${id}/execute`, { method: "POST", body: JSON.stringify(input) });
export const toggleTool = (id: string, enabled: boolean) =>
  fetchApi<any>(`/tools/${id}/toggle`, { method: "PUT", body: JSON.stringify({ enabled }) });

// Agents
export const getAgents = () => fetchApi<any>("/agents/list");
export const getAgentDetail = (id: string) => fetchApi<any>(`/agents/${id}`);
export const getAgentHistory = (id: string) => fetchApi<any>(`/agents/${id}/history`);
export const getAgentMetrics = (id: string) => fetchApi<any>(`/agents/${id}/metrics`);
export const updateAgentConfig = (id: string, config: any) =>
  fetchApi<any>(`/agents/${id}/config`, { method: "PUT", body: JSON.stringify(config) });
export const restartAgent = (id: string) =>
  fetchApi<any>(`/agents/${id}/restart`, { method: "POST" });
export const getAgentActivityStream = (id: string) => `${API_BASE}/agents/${id}/activity`;

// Workflows
export const getWorkflows = () => fetchApi<any>("/workflows/list");
export const getWorkflowDetail = (id: string) => fetchApi<any>(`/workflows/${id}`);
export const createWorkflow = (data: any) =>
  fetchApi<any>("/workflows/create", { method: "POST", body: JSON.stringify(data) });
export const updateWorkflow = (id: string, data: any) =>
  fetchApi<any>(`/workflows/${id}`, { method: "PUT", body: JSON.stringify(data) });
export const runWorkflow = (id: string) =>
  fetchApi<any>(`/workflows/${id}/run`, { method: "POST" });
export const deleteWorkflow = (id: string) =>
  fetchApi<any>(`/workflows/${id}`, { method: "DELETE" });
export const getWorkflowTemplates = () => fetchApi<any>("/workflows/templates");

// Work Stacks
export const getWorkStacks = () => fetchApi<any>("/workstacks/active");
export const getWorkStackDetail = (id: string) => fetchApi<any>(`/workstacks/${id}`);
export const getWorkStackHistory = (id: string) => fetchApi<any>(`/workstacks/${id}/history`);
export const createWorkStack = (data: any) =>
  fetchApi<any>("/workstacks/create", { method: "POST", body: JSON.stringify(data) });
export const controlWorkStack = (id: string, action: string) =>
  fetchApi<any>(`/workstacks/${id}/control`, { method: "POST", body: JSON.stringify({ action }) });
export const updateWorkStackContext = (id: string, context: any) =>
  fetchApi<any>(`/workstacks/${id}/context`, { method: "PUT", body: JSON.stringify(context) });

// Orchestration
export const getOrchestrationQueue = () => fetchApi<any>("/orchestration/queue");
export const getAntiHallucination = () => fetchApi<any>("/orchestration/anti-hallucination");
export const getOrchestrationResources = () => fetchApi<any>("/orchestration/resources");
export const getOrchestrationExecutions = () => fetchApi<any>("/orchestration/executions");
export const getOrchestrationPolicies = () => fetchApi<any>("/orchestration/policies");
export const updateOrchestrationPolicies = (data: any) =>
  fetchApi<any>("/orchestration/policies", { method: "PUT", body: JSON.stringify(data) });
export const getProcessMining = () => fetchApi<any>("/orchestration/process-mining");

// Execution Logs
export const getExecutionLogs = (filters?: Record<string, string>) => {
  const params = filters ? "?" + new URLSearchParams(filters).toString() : "";
  return fetchApi<any>(`/execution/logs${params}`);
};
export const replayExecution = (id: string) =>
  fetchApi<any>(`/execution/${id}/replay`, { method: "POST" });

// Debugger
export const getDebugExecutions = () => fetchApi<any>("/orchestration/debug/active");
export const getDebugTrace = (executionId: string) =>
  fetchApi<any>(`/orchestration/debug/${executionId}/trace`);
export const debugControl = (executionId: string, action: string) =>
  fetchApi<any>(`/orchestration/debug/${executionId}/control`, { method: "POST", body: JSON.stringify({ action }) });
export const getDebugVariables = (executionId: string) =>
  fetchApi<any>(`/orchestration/debug/${executionId}/variables`);
export const getDebugBreakpoints = () => fetchApi<any>("/orchestration/breakpoints");
export const addBreakpoint = (data: any) =>
  fetchApi<any>("/orchestration/breakpoints", { method: "POST", body: JSON.stringify(data) });
