import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { AppShell } from "@/components/shell/AppShell";
import OverviewPage from "@/pages/OverviewPage";
import ChatPage from "@/pages/ChatPage";
import ToolsPage from "@/pages/ToolsPage";
import AgentsPage from "@/pages/AgentsPage";
import LlmPage from "@/pages/LlmPage";
import ServicesPage from "@/pages/ServicesPage";
import SecurityPage from "@/pages/SecurityPage";
import ConfigPage from "@/pages/ConfigPage";
import InspectorPage from "@/pages/InspectorPage";
import StatePage from "@/pages/StatePage";
import LogsPage from "@/pages/LogsPage";
import NotFound from "@/pages/NotFound";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});

const App = () => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <Toaster />
      <Sonner />
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<Navigate to="/overview" replace />} />
            <Route path="/overview" element={<OverviewPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/tools" element={<ToolsPage />} />
            <Route path="/agents" element={<AgentsPage />} />
            <Route path="/llm" element={<LlmPage />} />
            <Route path="/services" element={<ServicesPage />} />
            <Route path="/security" element={<SecurityPage />} />
            <Route path="/config" element={<ConfigPage />} />
            <Route path="/inspector" element={<InspectorPage />} />
            <Route path="/state" element={<StatePage />} />
            <Route path="/logs" element={<LogsPage />} />
          </Route>
          <Route path="*" element={<NotFound />} />
        </Routes>
      </BrowserRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

export default App;
