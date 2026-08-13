//! Navigation model — mirrors the operation-dashboard `NAV_MANIFEST`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Route {
    // Console (top tab)
    Chat,
    Gallery,
    AntigravityLlm,
    Catalog,
    ZeroCatalog,
    ZeroSpecs,
    ZeroAgent,
    ZeroQuery,
    Overview,
    Orchestration,
    Services,
    Llm,
    Agents,
    Assistant,
    Models,
    Tools,
    Workflows,
    Security,
    Accountability,
    Skills,
    Knowledge,
    Embedding,
    Containers,
    PrivacyNetwork,
    Network,
    Ovs,
    OpenFlow,
    Btrfs,
    DataStores,
    Config,
    Inspector,
    State,
    Logs,
    Grpc,
    GrpcExplorer,
    // Coming Soon (placeholders)
    Dreams,
    Reflections,
    Observability,
    LiveTrace,
    Gateway,
    Channels,
    Cron,
    Integrations,
    Hardware,
    EStop,
    Invoke,
    Blobs,
    Plugins,
    // Top-level tab
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TopTab {
    Console,
    Install,
}

pub struct Item {
    pub title: &'static str,
    pub icon: &'static str,
    pub route: Route,
    pub placeholder: bool,
}

pub struct Group {
    pub label: &'static str,
    pub items: &'static [Item],
}

const fn it(title: &'static str, icon: &'static str, route: Route) -> Item {
    Item {
        title,
        icon,
        route,
        placeholder: false,
    }
}
const fn placeholder(title: &'static str, icon: &'static str, route: Route) -> Item {
    Item {
        title,
        icon,
        route,
        placeholder: true,
    }
}

pub static GROUPS: &[Group] = &[
    Group {
        label: "Chat",
        items: &[it("Chat", "💬", Route::Chat)],
    },
    Group { label: "UI Model", items: &[
        it("Gallery", "▦", Route::Gallery),
        it("Antigravity LLM", "✦", Route::AntigravityLlm),
        it("Catalog", "▤", Route::Catalog),
    ]},
    Group { label: "Zerolang", items: &[
        placeholder("Zero Catalog", "▦", Route::ZeroCatalog),
        placeholder("Zero Specs", "◇", Route::ZeroSpecs),
        placeholder("Zero Agent", "✦", Route::ZeroAgent),
        placeholder("Zero Query", "⌕", Route::ZeroQuery),
    ]},
    Group {
        label: "Control",
        items: &[
            it("Overview", "▦", Route::Overview),
            it("Orchestration", "◎", Route::Orchestration),
            it("Services", "⇄", Route::Services),
            it("LLM", "◉", Route::Llm),
        ],
    },
    Group {
        label: "Agent",
        items: &[
            it("Agents", "▤", Route::Agents),
            it("Assistant", "✦", Route::Assistant),
            it("Routable Models", "◉", Route::Models),
            it("Tools", "⚡", Route::Tools),
            it("Workflows", "⌥", Route::Workflows),
            it("Security", "⛨", Route::Security),
            it("Accountability", "✎", Route::Accountability),
            it("Skills", "✎", Route::Skills),
            it("Knowledge", "✷", Route::Knowledge),
            it("Embedding Pipeline", "◎", Route::Embedding),
        ],
    },
    Group {
        label: "Infrastructure",
        items: &[
            it("Containers", "▣", Route::Containers),
            it("Privacy Network", "⌖", Route::PrivacyNetwork),
            it("Network Control", "⌁", Route::Network),
            it("Open vSwitch", "⌬", Route::Ovs),
            it("OpenFlow", "⌘", Route::OpenFlow),
            it("BTRFS Storage", "▥", Route::Btrfs),
            it("Data Stores", "▥", Route::DataStores),
        ],
    },
    Group { label: "ZeroClaw", items: &[
        placeholder("Gateway", "⇄", Route::Gateway),
        placeholder("Channels", "◌", Route::Channels),
        placeholder("Cron", "◷", Route::Cron),
        placeholder("Integrations", "⌘", Route::Integrations),
        placeholder("Hardware", "▣", Route::Hardware),
    ]},
    Group {
        label: "Settings",
        items: &[
            it("Config", "⚙", Route::Config),
            it("Inspector", "✱", Route::Inspector),
            it("State", "▥", Route::State),
            it("Logs", "✎", Route::Logs),
            it("gRPC Diagnostics", "◉", Route::Grpc),
            it("gRPC Explorer", "⌁", Route::GrpcExplorer),
            placeholder("E-Stop", "■", Route::EStop),
            it("Invoke", "▶", Route::Invoke),
        ],
    },
    Group {
        label: "Coming Soon",
        items: &[
            placeholder("Dreams", "☾", Route::Dreams),
            placeholder("Reflections", "✦", Route::Reflections),
            placeholder("Observability", "◉", Route::Observability),
            placeholder("Live Trace", "≈", Route::LiveTrace),
        ],
    },
    Group { label: "Blobs", items: &[
        it("Sealed Blobs", "⬢", Route::Blobs),
        it("Plugins", "◆", Route::Plugins),
    ]},
];

pub fn route_title(r: Route) -> &'static str {
    for g in GROUPS {
        for i in g.items {
            if i.route == r {
                return i.title;
            }
        }
    }
    match r {
        Route::Install => "Install",
        _ => "ZeroClaw",
    }
}
