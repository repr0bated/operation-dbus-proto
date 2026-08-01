// Shared TypeScript types for Operation-DBUS and OpenClaw

// API Response types
export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
}

// User types
export interface User {
  id: string;
  email?: string;
  username?: string;
  created_at?: string;
  status?: "active" | "suspended" | "pending";
}

// VPN types
export interface VpnConnection {
  id: string;
  user_id: string;
  ip_address: string;
  connected_at: string;
  bytes_sent?: number;
  bytes_received?: number;
}

// Chat types
export interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
  timestamp?: string;
}

export interface ChatSession {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

// OpenClaw types
export interface OpenClawStatus {
  available: boolean;
  endpoint: string;
  model: string;
  container_ip: string;
  authenticated: boolean;
  error?: string;
}

export interface OpenClawConfig {
  endpoint: string;
  model: string;
  token_configured: boolean;
  container_ip: string;
  container_port: number;
}

// Tool types
export interface Tool {
  name: string;
  description: string;
  category?: string;
  enabled?: boolean;
}

// Agent types
export interface Agent {
  id: string;
  name: string;
  type: string;
  status: "running" | "stopped" | "error";
  created_at: string;
}

// Log types
export interface LogEntry {
  id?: string;
  timestamp: string;
  level: "ERROR" | "WARN" | "INFO" | "DEBUG";
  service: string;
  message: string;
}

// MCP types
export interface McpServer {
  id: string;
  name: string;
  status: "active" | "inactive" | "error";
  tools_count?: number;
}

// Analytics types
export interface MetricDataPoint {
  hour?: string;
  timestamp?: string;
  value: number;
  [key: string]: any;
}
