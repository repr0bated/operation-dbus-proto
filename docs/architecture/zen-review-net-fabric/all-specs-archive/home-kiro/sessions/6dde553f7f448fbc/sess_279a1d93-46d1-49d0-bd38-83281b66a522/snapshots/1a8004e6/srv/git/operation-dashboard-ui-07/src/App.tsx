// Dashboard SPA. Identity gate: if no Ghostbridge footprint/trace is available
// (config, env, pair session, or live sled), show PairingPage before the shell.
import { useEffect, useState } from "react";
import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { useEventStream } from "@/hooks/use-event-stream";
import { ShellRenderer, PageSpecOutlet } from "@/json-render";

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
import AssistantPage from "./pages/AssistantPage";
import GrpcExplorerPage from "./pages/GrpcExplorerPage";
import InvokePage from "./pages/InvokePage";
import GalleryPage from "./pages/GalleryPage";
import GalleryGenPage from "./pages/GalleryGenPage";
import CatalogPage from "./pages/CatalogPage";
import BlobsPage from "./pages/BlobsPage";
import PluginsPage from "./pages/PluginsPage";
import PluginPage from "./components/PluginPage";
import NotFound from "./pages/NotFound";
import RegisterPage from "./pages/RegisterPage";
import PairingPage from "./pages/PairingPage";
import GatewayPage from "./pages/GatewayPage";
import ChannelsPage from "./pages/ChannelsPage";
import CronPage from "./pages/CronPage";
import IntegrationsPage from "./pages/IntegrationsPage";
import EStopPage from "./pages/EStopPage";
import HardwarePage from "./pages/HardwarePage";
import {
  resolveIdentity,
  type IdentityStatus,
  type PairSession,
} from "@/lib/identity";
import { resetTransport } from "@/grpc/client";
import { Loader2 } from "lucide-react";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5000, refetchOnWindowFocus: false },
  },
});

// Dashboard routes live inside the spec-driven shell; the event stream only
// runs here so public pages don't open the admin SSE channel.
//
// PageSpecOutlet is a layout route: if PAGE_SPECS has the pathname it renders
// the json-render spec; otherwise <Outlet /> falls through to the React page.
function ShellRoutes() {
  useEventStream();
  return (
    <ShellRenderer>
      <Routes>
        <Route element={<PageSpecOutlet />}>
          {/* Index keeps `/` matched so PAGE_SPECS["/"] (Overview) can render. */}
          <Route index element={<></>} />
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/gallery" element={<GalleryPage />} />
          <Route path="/catalog" element={<CatalogPage />} />
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
          <Route path="/explorer" element={<GrpcExplorerPage />} />
          <Route path="/invoke" element={<InvokePage />} />
          <Route path="/accountability" element={<AccountabilityPage />} />
          <Route path="/btrfs" element={<BtrfsPage />} />
          <Route path="/data-stores" element={<DataStoresPage />} />
          <Route path="/embedding" element={<EmbeddingPipelinePage />} />
          <Route path="/assistant" element={<AssistantPage />} />
          <Route path="/blobs" element={<BlobsPage />} />
          <Route path="/plugins" element={<PluginsPage />} />
          <Route path="/plugin/:pluginId" element={<PluginPage />} />
          <Route path="/gateway" element={<GatewayPage />} />
          <Route path="/channels" element={<ChannelsPage />} />
          <Route path="/cron" element={<CronPage />} />
          <Route path="/integrations" element={<IntegrationsPage />} />
          <Route path="/estop" element={<EStopPage />} />
          <Route path="/hardware" element={<HardwarePage />} />
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
    </ShellRenderer>
  );
}


function IdentityGate({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<IdentityStatus>({ state: "loading" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const next = await resolveIdentity();
      if (cancelled) return;
      if (next.state === "ready") {
        resetTransport();
      }
      setStatus(next);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (status.state === "loading") {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (status.state === "needs_pairing") {
    return (
      <PairingPage
        reason={status.reason}
        onPaired={(session: PairSession) => {
          resetTransport();
          setStatus({ state: "ready", identity: session });
        }}
      />
    );
  }

  return <>{children}</>;
}

function AppInner() {
  return (
    <Routes>
      {/* Public surfaces — no identity gate */}
      <Route path="/register" element={<RegisterPage />} />
      <Route
        path="/*"
        element={
          <IdentityGate>
            <ShellRoutes />
          </IdentityGate>
        }
      />
    </Routes>
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
