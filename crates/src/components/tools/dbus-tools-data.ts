export interface DbusObjectTool {
  name: string;
  description: string;
  dbusPath: string;
  interface: string;
  methods: { name: string; signature: string; description: string }[];
  properties: { name: string; type: string; value: string; access: "read" | "readwrite" }[];
  signals: { name: string; args: string }[];
  category: string;
  tags: string[];
}

export const mockDbusTools: DbusObjectTool[] = [
  {
    name: "WireGuard.wg0",
    description: "WireGuard tunnel interface management",
    dbusPath: "/com/3tched/wireguard/wg0",
    interface: "com.3tched.WireGuard1",
    methods: [
      { name: "ListPeers", signature: "() → a{ss}", description: "List all connected peers" },
      { name: "AddPeer", signature: "(s pubkey, s endpoint, s allowed_ips) → b", description: "Add a new peer (requires confirmation)" },
      { name: "RemovePeer", signature: "(s pubkey) → b", description: "Remove peer by public key (destructive)" },
      { name: "GetStats", signature: "() → a{sv}", description: "Interface traffic statistics" },
    ],
    properties: [
      { name: "PublicKey", type: "s", value: "5xQ…kR=", access: "read" },
      { name: "ListenPort", type: "u", value: "51820", access: "read" },
      { name: "PeerCount", type: "u", value: "3", access: "read" },
      { name: "TxBytes", type: "t", value: "1284902", access: "read" },
    ],
    signals: [
      { name: "PeerConnected", args: "s pubkey, s endpoint" },
      { name: "PeerDisconnected", args: "s pubkey" },
    ],
    category: "network",
    tags: ["vpn", "tunnel", "privacy"],
  },
  {
    name: "Incus.Manager",
    description: "Incus container lifecycle manager",
    dbusPath: "/com/3tched/incus/manager",
    interface: "com.3tched.Incus.Manager1",
    methods: [
      { name: "ListContainers", signature: "() → a{sa{sv}}", description: "List all containers with status" },
      { name: "StartContainer", signature: "(s name) → b", description: "Start a stopped container" },
      { name: "StopContainer", signature: "(s name) → b", description: "Stop a running container (requires confirmation)" },
      { name: "CreateSnapshot", signature: "(s name, s snap_name) → b", description: "Create BTRFS snapshot" },
      { name: "GetContainerInfo", signature: "(s name) → a{sv}", description: "Detailed container info" },
    ],
    properties: [
      { name: "ContainerCount", type: "u", value: "5", access: "read" },
      { name: "RunningCount", type: "u", value: "4", access: "read" },
      { name: "StorageBackend", type: "s", value: "btrfs", access: "read" },
    ],
    signals: [
      { name: "ContainerStateChanged", args: "s name, s old_state, s new_state" },
      { name: "SnapshotCreated", args: "s container, s snapshot" },
    ],
    category: "containers",
    tags: ["lxc", "btrfs", "isolation"],
  },
  {
    name: "OVS.Bridge.br-ghost",
    description: "Open vSwitch bridge for GhostBridge network",
    dbusPath: "/com/3tched/ovs/bridges/br_ghost",
    interface: "com.3tched.OVS.Bridge1",
    methods: [
      { name: "ListPorts", signature: "() → as", description: "List all bridge ports" },
      { name: "AddPort", signature: "(s port_name) → b", description: "Add port to bridge" },
      { name: "RemovePort", signature: "(s port_name) → b", description: "Remove port (destructive)" },
      { name: "GetFlows", signature: "() → a{sv}", description: "OpenFlow rules on this bridge" },
    ],
    properties: [
      { name: "PortCount", type: "u", value: "4", access: "read" },
      { name: "DatapathType", type: "s", value: "system", access: "read" },
      { name: "STPEnabled", type: "b", value: "false", access: "readwrite" },
    ],
    signals: [
      { name: "PortAdded", args: "s port_name" },
      { name: "PortRemoved", args: "s port_name" },
    ],
    category: "network",
    tags: ["ovs", "switching", "sdn"],
  },
  {
    name: "Audit.Chain",
    description: "Blockchain audit trail interface",
    dbusPath: "/com/3tched/audit/chain",
    interface: "com.3tched.Audit.Chain1",
    methods: [
      { name: "GetLatestBlock", signature: "() → a{sv}", description: "Latest block info" },
      { name: "QueryLog", signature: "(s filter, u limit) → aa{sv}", description: "Query audit entries" },
      { name: "VerifyIntegrity", signature: "() → b", description: "Verify chain integrity" },
    ],
    properties: [
      { name: "BlockHeight", type: "u", value: "44201", access: "read" },
      { name: "PendingTxns", type: "u", value: "0", access: "read" },
      { name: "ChainHash", type: "s", value: "e3f2…a901", access: "read" },
    ],
    signals: [
      { name: "BlockCommitted", args: "u height, s hash" },
    ],
    category: "audit",
    tags: ["blockchain", "logging", "integrity"],
  },
  {
    name: "Dinit.Manager",
    description: "dinit service manager control interface",
    dbusPath: "/com/3tched/dinit/manager",
    interface: "com.3tched.Dinit.Manager1",
    methods: [
      { name: "ListServices", signature: "() → a{sa{sv}}", description: "All managed services with state" },
      { name: "StartService", signature: "(s name) → b", description: "Start a service unit" },
      { name: "StopService", signature: "(s name) → b", description: "Stop a service (requires confirmation)" },
      { name: "RestartService", signature: "(s name) → b", description: "Restart a service unit" },
      { name: "GetServiceLog", signature: "(s name, u lines) → as", description: "Tail service log" },
    ],
    properties: [
      { name: "ServiceCount", type: "u", value: "10", access: "read" },
      { name: "BootState", type: "s", value: "complete", access: "read" },
    ],
    signals: [
      { name: "ServiceStateChanged", args: "s name, s old, s new" },
    ],
    category: "system",
    tags: ["init", "services", "lifecycle"],
  },
  {
    name: "LLM.Gateway",
    description: "LLM inference gateway for AI chat",
    dbusPath: "/com/3tched/llm/gateway",
    interface: "com.3tched.LLM.Gateway1",
    methods: [
      { name: "Chat", signature: "(s prompt, a{sv} opts) → s", description: "Send chat message" },
      { name: "StreamChat", signature: "(s prompt) → stream", description: "Streaming chat response" },
      { name: "ListModels", signature: "() → as", description: "Available models" },
      { name: "GetTokenUsage", signature: "(s session_id) → a{sv}", description: "Session token stats" },
    ],
    properties: [
      { name: "ActiveModel", type: "s", value: "mistral-7b-instruct", access: "readwrite" },
      { name: "ActiveSessions", type: "u", value: "1", access: "read" },
    ],
    signals: [
      { name: "TokenBudgetWarning", args: "s session_id, u percent" },
    ],
    category: "ai",
    tags: ["llm", "inference", "chat"],
  },
];
