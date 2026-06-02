import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/AppSidebar";
import { DashboardLayout } from "@/components/DashboardLayout";
// Core pages
import Dashboard from "./pages/Dashboard";
import ChatPage from "./pages/openclaw/ChatPage";
import NotFound from "./pages/NotFound";

// Operation-DBUS pages
import UsersPage from "./pages/UsersPage";
import VpnPage from "./pages/VpnPage";
import MailPage from "./pages/MailPage";
import McpPage from "./pages/McpPage";
import ToolsPage from "./pages/ToolsPage";
import AgentsPage from "./pages/AgentsPage";
import WorkflowsPage from "./pages/WorkflowsPage";
import WorkStacksPage from "./pages/WorkStacksPage";
import OrchestrationPage from "./pages/OrchestrationPage";
import AnalyticsPage from "./pages/AnalyticsPage";
import ExecutionLogsPage from "./pages/ExecutionLogsPage";
import DebuggerPage from "./pages/DebuggerPage";
import SettingsPage from "./pages/SettingsPage";
import AccountabilityPage from "./pages/AccountabilityPage";
// Assistant pages (zero-trust via gRPC bridge)
import AssistantChat from "./pages/openclaw/ChatPage";
import AssistantSessions from "./pages/openclaw/SessionsPage";
import AssistantChannels from "./pages/openclaw/ChannelsPage";
import AssistantSkills from "./pages/openclaw/SkillsPage";
import AssistantCron from "./pages/openclaw/CronPage";
import AssistantDebug from "./pages/openclaw/DebugPage";
import AssistantLogs from "./pages/openclaw/LogsPage";
import AssistantConfig from "./pages/openclaw/ConfigPage";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, staleTime: 5000 } },
});

const App = () => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <Toaster />
      <Sonner />
      <BrowserRouter>
        <SidebarProvider>
          <div className="min-h-screen flex w-full bg-background">
            <AppSidebar />
            <DashboardLayout>
              <Routes>
                {/* DBUS routes */}
                <Route path="/" element={<Dashboard />} />
                <Route path="/chat" element={<ChatPage />} />
                <Route path="/users" element={<UsersPage />} />
                <Route path="/vpn" element={<VpnPage />} />
                <Route path="/mail" element={<MailPage />} />
                <Route path="/mcp" element={<McpPage />} />
                <Route path="/tools" element={<ToolsPage />} />
                <Route path="/agents" element={<AgentsPage />} />
                <Route path="/workflows" element={<WorkflowsPage />} />
                <Route path="/workstacks" element={<WorkStacksPage />} />
                <Route path="/orchestration" element={<OrchestrationPage />} />
                <Route path="/analytics" element={<AnalyticsPage />} />
                <Route path="/logs" element={<ExecutionLogsPage />} />
                <Route path="/debugger" element={<DebuggerPage />} />
                <Route
                  path="/accountability"
                  element={<AccountabilityPage />}
                />
                <Route path="/settings" element={<SettingsPage />} />
                {/* Assistant routes (zero-trust via gRPC bridge) */}
                <Route path="/openclaw" element={<AssistantChat />} />
                <Route
                  path="/openclaw/sessions"
                  element={<AssistantSessions />}
                />
                <Route
                  path="/openclaw/channels"
                  element={<AssistantChannels />}
                />
                <Route path="/openclaw/skills" element={<AssistantSkills />} />
                <Route path="/openclaw/cron" element={<AssistantCron />} />
                <Route path="/openclaw/debug" element={<AssistantDebug />} />
                <Route path="/openclaw/logs" element={<AssistantLogs />} />
                <Route path="/openclaw/config" element={<AssistantConfig />} />
                <Route path="*" element={<NotFound />} />
              </Routes>
            </DashboardLayout>
          </div>
        </SidebarProvider>
      </BrowserRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

export default App;
