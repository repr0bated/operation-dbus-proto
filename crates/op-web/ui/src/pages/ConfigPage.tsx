import { useState, useMemo } from "react";
import { PageHeader, Pill } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { useEventStore } from "@/stores/event-store";
import { cn } from "@/lib/utils";
import { Settings, Shield, Zap, Bot, Globe, Terminal, Network } from "lucide-react";

const SECTIONS = [
  { key: "general", label: "General", icon: Settings },
  { key: "dbus", label: "D-Bus", icon: Terminal },
  { key: "llm", label: "LLM", icon: Zap },
  { key: "agents", label: "Agents", icon: Bot },
  { key: "security", label: "Security", icon: Shield },
  { key: "network", label: "Network", icon: Network },
];

const SECTION_SCHEMAS: Record<string, Record<string, unknown>> = {
  general: {
    type: "object",
    properties: {
      log_level: { type: "string", title: "Log Level", enum: ["trace", "debug", "info", "warn", "error"] },
      data_dir: { type: "string", title: "Data Directory" },
      pid_file: { type: "string", title: "PID File" },
      telemetry_enabled: { type: "boolean", title: "Telemetry Enabled" },
      max_workers: { type: "number", title: "Max Workers", minimum: 1, maximum: 64 },
    },
  },
  dbus: {
    type: "object",
    properties: {
      system_bus: { type: "boolean", title: "System Bus" },
      session_bus: { type: "boolean", title: "Session Bus" },
      bus_address: { type: "string", title: "Bus Address" },
      introspection_cache: { type: "boolean", title: "Introspection Cache" },
      trace_signals: { type: "boolean", title: "Trace Signals" },
      trace_buffer_size: { type: "number", title: "Trace Buffer Size", minimum: 100, maximum: 100000 },
      sampling_rate: { type: "number", title: "Sampling Rate (%)", minimum: 1, maximum: 100 },
    },
  },
  llm: {
    type: "object",
    properties: {
      default_model: { type: "string", title: "Default Model", enum: ["gpt-4o", "gpt-4o-mini", "claude-sonnet-4", "gemini-2.5-pro"] },
      temperature: { type: "number", title: "Temperature", minimum: 0, maximum: 2 },
      max_tokens: { type: "number", title: "Max Tokens", minimum: 256, maximum: 128000 },
      streaming: { type: "boolean", title: "Streaming Enabled" },
      rate_limit_rpm: { type: "number", title: "Rate Limit (RPM)", minimum: 1, maximum: 10000 },
      fallback_model: { type: "string", title: "Fallback Model", enum: ["gpt-4o-mini", "gemini-2.5-flash", "none"] },
    },
  },
  agents: {
    type: "object",
    properties: {
      max_concurrent: { type: "number", title: "Max Concurrent Agents", minimum: 1, maximum: 50 },
      session_timeout: { type: "number", title: "Session Timeout (s)", minimum: 60, maximum: 86400 },
      memory_backend: { type: "string", title: "Memory Backend", enum: ["qdrant", "redis", "in-memory"] },
      auto_restart: { type: "boolean", title: "Auto Restart on Failure" },
      heartbeat_interval: { type: "number", title: "Heartbeat Interval (s)", minimum: 5, maximum: 300 },
      max_context_window: { type: "number", title: "Max Context Window", minimum: 4096, maximum: 200000 },
    },
  },
  security: {
    type: "object",
    properties: {
      auth_mode: { type: "string", title: "Auth Mode", enum: ["oidc", "gcloud-adc", "wireguard", "none"] },
      tls_enabled: { type: "boolean", title: "TLS Enabled" },
      tls_cert_path: { type: "string", title: "TLS Certificate Path" },
      tls_key_path: { type: "string", title: "TLS Key Path" },
      audit_chain: { type: "boolean", title: "Blockchain Audit Enabled" },
      exec_approval: { type: "boolean", title: "Require Exec Approval" },
      allowed_cidrs: { type: "string", title: "Allowed CIDRs (comma-separated)" },
    },
  },
  network: {
    type: "object",
    properties: {
      listen_addr: { type: "string", title: "Listen Address" },
      ws_enabled: { type: "boolean", title: "WebSocket Enabled" },
      sse_enabled: { type: "boolean", title: "SSE Enabled" },
      cors_origin: { type: "string", title: "CORS Origin" },
      proxy_protocol: { type: "boolean", title: "Proxy Protocol" },
      max_connections: { type: "number", title: "Max Connections", minimum: 10, maximum: 100000 },
      idle_timeout: { type: "number", title: "Idle Timeout (s)", minimum: 10, maximum: 3600 },
    },
  },
};

export default function ConfigPage() {
  const { latestState } = useEventStore();
  const [activeSection, setActiveSection] = useState("general");
  const [mode, setMode] = useState<"structured" | "raw">("structured");
  const [localValues, setLocalValues] = useState<Record<string, Record<string, unknown>>>({});

  const sectionData = useMemo(() => {
    const live = latestState[`config.${activeSection}`] ?? latestState[`config:${activeSection}`];
    const liveData = live && typeof live === "object" ? (live as Record<string, unknown>) : {};
    return { ...liveData, ...(localValues[activeSection] ?? {}) };
  }, [activeSection, latestState, localValues]);

  const rawValue = useMemo(() => JSON.stringify(sectionData, null, 2), [sectionData]);

  const handleChange = (val: unknown) => {
    setLocalValues((p) => ({ ...p, [activeSection]: val as Record<string, unknown> }));
  };

  return (
    <>
      <PageHeader title="Config" subtitle="Edit control plane configuration safely." />
      <div className="grid grid-cols-[260px_1fr] gap-0 rounded-xl border border-border bg-card overflow-hidden" style={{ height: "calc(100vh - 180px)" }}>
        {/* Sidebar */}
        <div className="flex flex-col border-r border-border bg-secondary/30 min-h-0 overflow-hidden">
          <div className="flex items-center justify-between px-4 py-3 border-b border-border">
            <span className="text-sm font-semibold">Settings</span>
            <Pill variant="ok">valid</Pill>
          </div>
          <div className="flex-1 overflow-y-auto p-2.5">
            {SECTIONS.map((s) => (
              <button key={s.key} onClick={() => setActiveSection(s.key)}
                className={cn("w-full flex items-center gap-3 px-3.5 py-2.5 rounded-md text-[13px] font-medium transition-colors",
                  activeSection === s.key ? "bg-primary/10 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-muted/30")}>
                <s.icon className="h-[18px] w-[18px] opacity-70" />
                <span>{s.label}</span>
              </button>
            ))}
          </div>
          <div className="p-3 border-t border-border">
            <div className="flex rounded-md border border-border bg-card overflow-hidden">
              {(["structured", "raw"] as const).map((m) => (
                <button key={m} onClick={() => setMode(m)} className={cn("flex-1 py-2 text-xs font-semibold transition-colors",
                  mode === m ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:text-foreground")}>
                  {m === "structured" ? "Structured" : "Raw JSON"}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Main */}
        <div className="flex flex-col min-h-0">
          <div className="flex items-center justify-between px-5 py-3 border-b border-border bg-secondary/30">
            <div className="flex items-center gap-3">
              <span className="text-base font-semibold text-foreground">{SECTIONS.find((s) => s.key === activeSection)?.label}</span>
            </div>
            <div className="flex gap-2">
              <button onClick={() => setLocalValues((p) => { const n = { ...p }; delete n[activeSection]; return n; })}
                className="px-3 py-1.5 rounded-md border border-border text-xs font-medium hover:bg-muted/30 transition-colors">Reset</button>
              <button className="px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors">Save</button>
              <button className="px-3 py-1.5 rounded-md border border-primary/30 text-primary text-xs font-medium hover:bg-primary/10 transition-colors">Apply</button>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-5">
            {mode === "raw" ? (
              <textarea value={rawValue} readOnly
                className="w-full h-full min-h-[500px] px-3 py-2 rounded-md border border-input bg-card text-sm font-mono resize-y focus:border-ring outline-none" />
            ) : (
              <SchemaRenderer
                schema={SECTION_SCHEMAS[activeSection] as any}
                data={sectionData}
                onChange={handleChange}
              />
            )}
          </div>
        </div>
      </div>
    </>
  );
}
