import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "@/components/shell/AppShell";
import { useEventStream } from "@/hooks/use-event-stream";
import OverviewPage from "./pages/OverviewPage";
import ChatPage from "./pages/ChatPage";
import ToolsPage from "./pages/ToolsPage";
import RoutableModelsPage from "./pages/RoutableModelsPage";
import AgentsPage from "./pages/AgentsPage";
import LlmPage from "./pages/LlmPage";
import ServicesPage from "./pages/ServicesPage";
import SecurityPage from "./pages/SecurityPage";
import ConfigPage from "./pages/ConfigPage";
import InspectorPage from "./pages/InspectorPage";
import StatePage from "./pages/StatePage";
import LogsPage from "./pages/LogsPage";
import WorkflowsPage from "./pages/WorkflowsPage";
import OrchestrationPage from "./pages/OrchestrationPage";
import SkillsPage from "./pages/SkillsPage";
import ContainersPage from "./pages/ContainersPage";
import PrivacyNetworkPage from "./pages/PrivacyNetworkPage";
import OpenSwitchPage from "./pages/OpenSwitchPage";
import OpenFlowPage from "./pages/OpenFlowPage";
import KnowledgePage from "./pages/KnowledgePage";
import GrpcDiagnosticsPage from "./pages/GrpcDiagnosticsPage";
import AccountabilityPage from "./pages/AccountabilityPage";
import BtrfsPage from "./pages/BtrfsPage";
import DataStoresPage from "./pages/DataStoresPage";
import EmbeddingPipelinePage from "./pages/EmbeddingPipelinePage";
import PluginPage from "@/pages/PluginPage";
import SchemaGalleryPage from "@/pages/SchemaGalleryPage";
import GemmaUIPage from "@/pages/GemmaUIPage";
import GemmaPresentationPage from "@/pages/GemmaPresentationPage";
import GemmaFreeUIPage from "@/pages/GemmaFreeUIPage";
import GemmaFreePresentationPage from "@/pages/GemmaFreePresentationPage";
import { GemmaDataPage, GemmaSpatialPage, GemmaFlowPage, GemmaAestheticPage } from "@/pages/GemmaPerspectivePages";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, staleTime: 5000, refetchOnWindowFocus: false } },
});

function AppInner() {
  useEventStream();
  return (
    <AppShell>
      <Routes>
        <Route path="/" element={<OverviewPage />} />
        <Route path="/chat" element={<ChatPage />} />
        <Route path="/tools" element={<ToolsPage />} />
        <Route path="/agents" element={<AgentsPage />} />
        <Route path="/models" element={<RoutableModelsPage />} />
        <Route path="/llm" element={<LlmPage />} />
        <Route path="/services" element={<ServicesPage />} />
        <Route path="/security" element={<SecurityPage />} />
        <Route path="/config" element={<ConfigPage />} />
        <Route path="/inspector" element={<InspectorPage />} />
        <Route path="/state" element={<StatePage />} />
        <Route path="/logs" element={<LogsPage />} />
        <Route path="/workflows" element={<WorkflowsPage />} />
        <Route path="/orchestration" element={<OrchestrationPage />} />
        <Route path="/skills" element={<SkillsPage />} />
        <Route path="/containers" element={<ContainersPage />} />
        <Route path="/privacy-network" element={<PrivacyNetworkPage />} />
        <Route path="/ovs" element={<OpenSwitchPage />} />
            <Route path="/openflow" element={<OpenFlowPage />} />
            <Route path="/knowledge" element={<KnowledgePage />} />
            <Route path="/grpc" element={<GrpcDiagnosticsPage />} />
            <Route path="/accountability" element={<AccountabilityPage />} />
            <Route path="/btrfs" element={<BtrfsPage />} />
            <Route path="/data-stores" element={<DataStoresPage />} />
            <Route path="/embedding" element={<EmbeddingPipelinePage />} />
            <Route path="/plugin/:pluginId" element={<PluginPage />} />
            <Route path="/schema-gallery" element={<SchemaGalleryPage />} />
            <Route path="/gemma-ui" element={<GemmaUIPage />} />
            <Route path="/gemma-presentation" element={<GemmaPresentationPage />} />
            <Route path="/gemma-free-ui" element={<GemmaFreeUIPage />} />
            <Route path="/gemma-free-presentation" element={<GemmaFreePresentationPage />} />
            <Route path="/gemma-data" element={<GemmaDataPage />} />
            <Route path="/gemma-spatial" element={<GemmaSpatialPage />} />
            <Route path="/gemma-flow" element={<GemmaFlowPage />} />
            <Route path="/gemma-aesthetic" element={<GemmaAestheticPage />} />
            <Route path="*" element={<NotFound />} />
      </Routes>
    </AppShell>
  );
}

const App = () => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <Toaster />
      <Sonner />
      <BrowserRouter>
        <AppInner />
      </BrowserRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

export default App;
