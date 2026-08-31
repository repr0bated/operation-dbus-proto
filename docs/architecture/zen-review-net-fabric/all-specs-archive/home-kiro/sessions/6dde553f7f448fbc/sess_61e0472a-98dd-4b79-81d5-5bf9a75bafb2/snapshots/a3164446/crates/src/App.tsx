import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import OverviewPage from "./pages/OverviewPage";
import ChatPage from "./pages/ChatPage";
import ToolsPage from "./pages/ToolsPage";
import AgentsPage from "./pages/AgentsPage";
import LlmPage from "./pages/LlmPage";
import ServicesPage from "./pages/ServicesPage";
import SecurityPage from "./pages/SecurityPage";
import ConfigPage from "./pages/ConfigPage";
import InspectorPage from "./pages/InspectorPage";
import StatePage from "./pages/StatePage";
import KnowledgeStorePage from "./pages/claw/KnowledgeStorePage";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const App = () => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <Toaster />
      <Sonner />
      <BrowserRouter>
        <AppShell>
          <Routes>
            <Route path="/" element={<OverviewPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/tools" element={<ToolsPage />} />
            <Route path="/agents" element={<AgentsPage />} />
            <Route path="/llm" element={<LlmPage />} />
            <Route path="/services" element={<ServicesPage />} />
            <Route path="/security" element={<SecurityPage />} />
            <Route path="/config" element={<ConfigPage />} />
            <Route path="/inspector" element={<InspectorPage />} />
            <Route path="/state" element={<StatePage />} />
            <Route path="/claw/search" element={<KnowledgeStorePage />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </AppShell>
      </BrowserRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

export default App;
