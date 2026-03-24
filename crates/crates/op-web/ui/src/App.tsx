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
import ChatPage from "./pages/ChatPage";
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
// OpenClaw pages
import OpenClawChat from "./pages/openclaw/ChatPage";
import OpenClawSessions from "./pages/openclaw/SessionsPage";
import OpenClawChannels from "./pages/openclaw/ChannelsPage";
import OpenClawSkills from "./pages/openclaw/SkillsPage";
import OpenClawCron from "./pages/openclaw/CronPage";
import OpenClawDebug from "./pages/openclaw/DebugPage";
import OpenClawLogs from "./pages/openclaw/LogsPage";
import OpenClawConfig from "./pages/openclaw/ConfigPage";

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
                <Route path="/settings" element={<SettingsPage />} />
                {/* OpenClaw routes */}
                <Route path="/openclaw" element={<OpenClawChat />} />
                <Route path="/openclaw/sessions" element={<OpenClawSessions />} />
                <Route path="/openclaw/channels" element={<OpenClawChannels />} />
                <Route path="/openclaw/skills" element={<OpenClawSkills />} />
                <Route path="/openclaw/cron" element={<OpenClawCron />} />
                <Route path="/openclaw/debug" element={<OpenClawDebug />} />
                <Route path="/openclaw/logs" element={<OpenClawLogs />} />
                <Route path="/openclaw/config" element={<OpenClawConfig />} />
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
