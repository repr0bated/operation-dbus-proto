/**
 * Navigation manifest for the AppShell.
 *
 * This is the single place to add/remove pages. In the future it should be
 * hydrated from the plugin registry (e.g. /api/plugins/routes) so the UI
 * only shows what is actually deployed.
 */
import {
  MessageSquare, BarChart3, Link2, FileText, Radio,
  Folder, Zap, Monitor, Settings, Bug, ScrollText, Shield,
  GitBranch, Orbit, Box, Globe, Network, Workflow, Brain,
} from "lucide-react";

export interface NavItem {
  title: string;
  path: string;
  icon: React.ComponentType<{ className?: string }>;
}

export interface NavGroup {
  label: string;
  items: NavItem[];
}

export const NAV_GROUPS: NavGroup[] = [
  {
    label: "Zeroclaw",
    items: [
      { title: "Antigravity Chat", path: "/chat", icon: MessageSquare },
      { title: "Routable Models", path: "/models", icon: Radio },
      { title: "LLM Gateway", path: "/llm", icon: Radio },
    ],
  },
  {
    label: "Control",
    items: [
      { title: "Overview", path: "/", icon: BarChart3 },
      { title: "Orchestration", path: "/orchestration", icon: Orbit },
      { title: "Services", path: "/services", icon: Link2 },
    ],
  },
  {
    label: "Agent",
    items: [
      { title: "Agents", path: "/agents", icon: Folder },
      { title: "Tools", path: "/tools", icon: Zap },
      { title: "Workflows", path: "/workflows", icon: GitBranch },
      { title: "Security", path: "/security", icon: Shield },
      { title: "Accountability", path: "/accountability", icon: ScrollText },
      { title: "Skills", path: "/skills", icon: ScrollText },
      { title: "Knowledge", path: "/knowledge", icon: Brain },
      { title: "Embedding Pipeline", path: "/embedding", icon: Orbit },
    ],
  },
  {
    label: "Infrastructure",
    items: [
      { title: "Containers", path: "/containers", icon: Box },
      { title: "Privacy Network", path: "/privacy-network", icon: Globe },
      { title: "Open vSwitch", path: "/ovs", icon: Network },
      { title: "OpenFlow", path: "/openflow", icon: Workflow },
      { title: "BTRFS Storage", path: "/btrfs", icon: Monitor },
      { title: "Data Stores", path: "/data-stores", icon: Monitor },
    ],
  },
  {
    label: "Settings",
    items: [
      { title: "Config", path: "/config", icon: Settings },
      { title: "Inspector", path: "/inspector", icon: Bug },
      { title: "State", path: "/state", icon: Monitor },
      { title: "Logs", path: "/logs", icon: ScrollText },
      { title: "gRPC Diagnostics", path: "/grpc", icon: Radio },
    ],
  },
];
