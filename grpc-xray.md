Skip to content

OpenRouter

Search
⌘
K
Home
Models
Fusion
Chat
Rankings
Apps
Docs

Personal
Personal
Fusion


New fusion
⌘/
Search runs...
3 runs
Create a configuration guide for video a…

fix this error {"code":400,"message":"Th…

Provide a deep research technical overvi…


D
Default Workspace
a136a355-9e17-563e-86f1-9154ebfe52b4
Prompt
Quality
21 hours ago
Web Search


Provide a deep research technical overview of the interaction between Xray (Project X), gRPC, and Unix Domain Sockets (UDS). Focus specifically on how these components work together for local process-to-process communication. Emphasize best practices and configuration syntax, prioritizing examples in Protobuf format.

Show more
Step 1/3
Sources

Favicon for anthropic
Claude Opus Latest
Complete

Favicon for openai
OpenAI GPT Latest
Complete

Favicon for google
Google Gemini Pro Latest
Complete
Step 2/3
Analysis

Agreement
16
Xray uses gRPC in two conceptually distinct ways: (1) a control-plane management API via app/commander (HandlerService, …


Key Differences
5
Name of the data-plane gRPC transport service in Xray-core, MultiHunk message definition, domainSocket transport setting…


Partial Coverage
16
Include ListInbounds/ListOutbounds RPCs in HandlerService (Claude's HandlerService omits list operations, showing only A…


Unique Insights
9
Explains MultiHunk's rationale: batching multiple payload chunks per gRPC message reduces HTTP/2 framing overhead and CP…


Blind Spots
9
No concrete benchmark numbers or measured latency/throughput deltas for UDS vs TCP loopback — all claims remain qualitat…

Step 3/3
Result
Favicon for anthropic
Claude Opus Latest
Favicon for anthropic
Favicon for openai
Favicon for google
Fused


Xray-core, gRPC, and Unix Domain Sockets: A Technical Overview of Local IPC
1. Architecture: Two Distinct Uses of gRPC
Before any configuration, one distinction must be internalized because it is the single largest source of confusion: Xray uses gRPC in two completely unrelated ways. They share the gRPC runtime, HTTP/2, and Protobuf, but they are not interchangeable.

1.1 The Control Plane — the management API (app/commander)
Xray embeds a real grpc.Server for runtime management: querying stats, adding/removing handlers and users, testing routing, and restarting the logger. Enabling it looks like:


{
  "api": {
    "tag": "api",
    "services": ["HandlerService", "StatsService", "RoutingService", "LoggerService"]
  }
}
Architecturally, the commander is

Read full response
Continue in Chat


Fusion Run | OpenRouter
Favicon for anthropic
Claude Opus Latest
Favicon for anthropic
Favicon for openai
Favicon for google
Fused




Prompt

Analysis
Xray-core, gRPC, and Unix Domain Sockets: A Technical Overview of Local IPC
1. Architecture: Two Distinct Uses of gRPC
Before any configuration, one distinction must be internalized because it is the single largest source of confusion: Xray uses gRPC in two completely unrelated ways. They share the gRPC runtime, HTTP/2, and Protobuf, but they are not interchangeable.

1.1 The Control Plane — the management API (app/commander)
Xray embeds a real grpc.Server for runtime management: querying stats, adding/removing handlers and users, testing routing, and restarting the logger. Enabling it looks like:


{
  "api": {
    "tag": "api",
    "services": ["HandlerService", "StatsService", "RoutingService", "LoggerService"]
  }
}
Architecturally, the commander is not a standalone socket server in the traditional pattern. It registers as an internal outbound under the api tag. Traffic reaches it through a three-step path:


controller ──gRPC/HTTP2──► API inbound ──routing rule──► internal "api" outbound ──► commander grpc.Server
An inbound (conventionally dokodemo-door, now also called tunnel) accepts the connection.
A routing rule matches its tag and sends it to the api outbound tag.
The commander hijacks that connection and serves gRPC over it.
The crucial consequence: the API's transport (TCP vs UDS) is entirely determined by the inbound that fronts it. This is why "gRPC over UDS" is achieved simply by making that fronting inbound a UDS listener. Recent Xray (roughly 1.8.12+) also offers a direct api.listen shortcut that provisions this listener without manual inbound/routing wiring.

1.2 The Data Plane — gRPC as a stream transport (transport/internet/grpc)
Completely separately, gRPC is a pluggable stream transport ("network": "grpc" in streamSettings) that tunnels proxied VLESS/VMess/Trojan bytes inside a bidirectional gRPC stream, typically to survive CDNs and HTTP/2-aware middleboxes. The proxied bytes are chopped into Hunk/MultiHunk Protobuf messages and shipped over long-lived streams via a service historically named GunService.

1.3 Why the distinction matters
Control-plane API	Stream transport
Package	app/commander + app/*/command	transport/internet/grpc
Proto service	HandlerService, StatsService, …	GunService / GRPCService
RPC path	/xray.app.stats.command.StatsService/GetStats	/…GunService/Tun, /…GunService/TunMulti
Payload	Structured RPC messages	Opaque bytes hunks
serviceName config	N/A (fixed names)	User-chosen
The anti-pattern to avoid: attaching "network": "grpc" to the API forwarding inbound. This introduces a second, outer Xray gRPC transport, forcing management clients to speak gRPC-over-gRPC. For direct API access over UDS, the socket should carry the commander's HTTP/2 bytes directly — via a raw/domainsocket listener or the direct api.listen UDS — not an additional proxy grpc stream transport.

2. Unix Domain Sockets in Xray
There are, symmetrically, two distinct UDS mechanisms.

2.1 UDS as a listen/dial address (the modern, preferred form)
Xray accepts a socket path directly in the address field:


{ "listen": "/dev/shm/xray.sock", "port": 0 }
Three forms exist:

"/run/xray/xray.sock" — filesystem-path socket
"@xray-api" — Linux abstract-namespace socket (leading @ is translated to a NUL byte; nothing appears on the filesystem)
"@@xray-api" — abstract socket with padding (HAProxy compatibility, uses the full sockaddr_un.sun_path length)
When a UDS is used, port and allocate are ignored — "port": 0 is a schema-satisfying placeholder, not a magic feature. Some versions also accept a mode suffix, e.g. "listen": "/run/xray/api.sock,0660".

2.2 domainSocket as a stream transport
Xray also has a dedicated domain-socket transport, enumerated as TransportProtocol_DomainSocket = 5:


{
  "streamSettings": {
    "network": "domainsocket",
    "dsSettings": {
      "path": "/run/xray/transport.sock",
      "abstract": false,
      "padding": false
    }
  }
}
Version note: the settings key is commonly dsSettings, though some documentation/forks use domainSocketSettings. Validate against your build. The network name is domainsocket.

The difference:

UDS-as-address	domainsocket transport
Path in listen/server address	Selected by streamSettings.network
Can host raw/WS/gRPC on top	The domain socket is the selected transport
Uses /path, @name, @@name	Uses dsSettings.{path,abstract,padding}
Both ultimately resolve to the same low-level ListenUnix/dial machinery. The address form is now generally recommended for brevity; the transport form remains for explicit padding control.

2.3 dokodemo-door / tunnel, and the "port": 0 pattern
The historical API forwarding inbound was dokodemo-door; recent docs call the evolved protocol tunnel (with rewriteAddress, rewritePort, allowedNetwork). With a UDS the port: 0 is a placeholder because the real listener is the socket path.

Important version caveat: native UDS support in the tunnel/dokodemo path changed recently. Xray had to fix a panic where the tunnel tried to inherit the local listening port even when a Unix connection has no port — see commit e61eeae ("Tunnel inbound: Fix panic when listening on UDS for Xray's internal services (e.g. API)"). Consequently, on older builds prefer network: "domainsocket" + dsSettings for the API; on newer builds native listen: "/path" + tunnel works. Test against your exact binary.

2.4 Filesystem vs abstract sockets
Filesystem socket (/run/xray/api.sock):

Has an inode with owner/group/mode; directory execute/search permission also matters.
Inspectable via stat, ss -xl.
Can survive an unclean crash as a stale file, causing bind: address already in use (EADDRINUSE); must be unlinked before rebind.
Portable across Unix-like systems (with different sun_path limits — ~108 bytes Linux, ~104 macOS/BSD).
Abstract socket (@name, Linux/Android only):

No inode, no owner/group/mode — cannot be secured with filesystem permissions.
Self-cleaning — vanishes when the last reference closes (no stale-file problem).
Scoped to a network namespace; any process in that namespace able to name it may connect.
Non-portable — silently fails off Linux.
Path length: sockaddr_un.sun_path is short (~108 bytes). Keep paths like /run/xray/api.sock; overly deep paths surface as invalid argument/bind: invalid argument.

3. gRPC over UDS for Local IPC — Three Approaches
3.1 Direct api.listen UDS (recent Xray)
Recent mainline lets app/commander bind a UDS directly when listen begins with / or @:


{
  "log": { "loglevel": "warning" },
  "api": {
    "tag": "api",
    "listen": "/run/xray/api.sock",
    "services": ["HandlerService", "StatsService", "RoutingService", "LoggerService"]
  },
  "stats": {},
  "policy": {
    "levels": { "0": { "statsUserUplink": true, "statsUserDownlink": true } },
    "system": {
      "statsInboundUplink": true, "statsInboundDownlink": true,
      "statsOutboundUplink": true, "statsOutboundDownlink": true
    }
  },
  "outbounds": [ { "tag": "direct", "protocol": "freedom" } ]
}
No inbound or routing rule needed.

Version warning: published docs historically described api.listen as IP:port; direct-UDS support is newer, and the ,0660 suffix may not be honored by early implementations. Prefer a restrictive service UMask plus controlled chown/chmod.

3.2 Traditional routing over domainsocket (version-robust)

{
  "api": {
    "tag": "api",
    "services": ["HandlerService", "StatsService", "RoutingService", "LoggerService"]
  },
  "stats": {},
  "inbounds": [
    {
      "tag": "api-in",
      "listen": "127.0.0.1",
      "port": 0,
      "protocol": "dokodemo-door",
      "settings": { "address": "127.0.0.1", "network": "tcp" },
      "streamSettings": {
        "network": "domainsocket",
        "dsSettings": { "path": "/run/xray/api.sock", "abstract": false, "padding": false }
      }
    }
  ],
  "outbounds": [ { "tag": "direct", "protocol": "freedom" } ],
  "routing": {
    "rules": [ { "type": "field", "inboundTag": ["api-in"], "outboundTag": "api" } ]
  }
}
The bytes on the socket are ordinary management HTTP/2/gRPC; the inbound + route deliver them to the commander. Use distinct tags (api-in vs api) to avoid ambiguity.

3.3 Native UDS tunnel inbound (newer builds)

{
  "inbounds": [
    {
      "tag": "api-in",
      "listen": "/run/xray/api.sock,0660",
      "port": 0,
      "protocol": "tunnel",
      "settings": { "allowedNetwork": "tcp", "rewriteAddress": "127.0.0.1" }
    }
  ],
  "routing": {
    "rules": [ { "type": "field", "inboundTag": ["api-in"], "outboundTag": "api" } ]
  }
}
3.4 Dialing from a local controller (Go)
An explicit Unix context dialer is robust across gRPC-Go resolver versions; no TLS is needed for a local socket:


dialer := func(ctx context.Context, _ string) (net.Conn, error) {
    var d net.Dialer
    return d.DialContext(ctx, "unix", "/run/xray/api.sock") // abstract: "@xray-api"
}
conn, err := grpc.NewClient(
    "passthrough:///xray-api",
    grpc.WithTransportCredentials(insecure.NewCredentials()),
    grpc.WithContextDialer(dialer),
    grpc.WithAuthority("localhost"),
)
client := statscmd.NewStatsServiceClient(conn)
Some recent gRPC-Go accepts grpc.NewClient("unix:///run/xray/api.sock", …) directly. With grpcurl:


grpcurl -plaintext -unix /run/xray/api.sock list
grpcurl -plaintext -unix @xray-api xray.app.stats.command.StatsService/GetSysStats
What crosses the socket: ordinary gRPC — HTTP/2 preface, SETTINGS, HEADERS with :path=/xray.app.stats.command.StatsService/GetSysStats, DATA frames carrying Protobuf. UDS replaces only TCP as the byte transport; it removes none of the HTTP/2 framing, HPACK, gRPC length prefixes, or Protobuf serialization.

4. gRPC Data-Plane Transport Configuration

{
  "streamSettings": {
    "network": "grpc",
    "security": "none",
    "grpcSettings": {
      "serviceName": "local-vless",
      "multiMode": false,
      "idle_timeout": 60,
      "health_check_timeout": 20,
      "permit_without_stream": false,
      "initial_windows_size": 0
    }
  }
}
serviceName — service/path discriminator; both peers must agree.
multiMode — selects the batched TunMulti path rather than per-tunnel Tun.
idle_timeout, health_check_timeout, permit_without_stream, initial_windows_size — HTTP/2 keepalive/flow-control knobs.
Casing trap: serviceName and multiMode are camelCase, but the timeout/window fields are snake_case (idle_timeout, not idleTimeout). Wrong casing is silently ignored, so timeouts "don't work." These fields configure the data-plane transport only — they do not configure the commander's server (which uses gRPC defaults).

A proxy gRPC inbound over UDS is a GunService endpoint over a socket — not a management API socket:


{
  "tag": "local-vless-grpc",
  "listen": "/run/xray/vless-grpc.sock,0660",
  "port": 0,
  "protocol": "vless",
  "settings": { "clients": [ { "id": "…" } ], "decryption": "none" },
  "streamSettings": {
    "network": "grpc",
    "grpcSettings": { "serviceName": "local-vless", "multiMode": false }
  }
}
5. Protobuf Definitions
These are reconstructed from Xray-core sources/generated stubs. Field numbers, types, and method sets are version-dependent — always generate controller stubs from the exact revision you manage. Notable areas of drift are flagged.

5.1 Data-plane encoder — transport/internet/grpc/encoding/stream.proto
The most version-sensitive item. V2Ray legacy uses GunService with only Tun; Xray added TunMulti. Some builds also rename the service to GRPCService.


syntax = "proto3";
package xray.transport.internet.grpc.encoding;
option go_package = "github.com/xtls/xray-core/transport/internet/grpc/encoding";

message Hunk {
  bytes data = 1;
}

// The MultiHunk `data` field type is version-divergent: current Xray sources
// most commonly define it as a repeated bytes field for batching; some
// generated stubs show a single bytes field. Verify against your revision.
message MultiHunk {
  repeated bytes data = 1;
}

service GunService {              // Xray may register this as GRPCService
  rpc Tun(stream Hunk) returns (stream Hunk);
  rpc TunMulti(stream MultiHunk) returns (stream MultiHunk);
}
multiMode: true selects TunMulti, batching multiple payload chunks per gRPC message to reduce HTTP/2 framing overhead and context switches. Interop with a strict V2Ray peer (only Tun) requires multiMode: false. A serviceName/multiMode mismatch surfaces as connection resets, not clear errors.

5.2 HandlerService — app/proxyman/command/command.proto

syntax = "proto3";
package xray.app.proxyman.command;

message AddUserOperation    { xray.common.protocol.User user = 1; }
message RemoveUserOperation { string email = 1; }

message AddInboundRequest    { xray.core.InboundHandlerConfig inbound = 1; }
message AddInboundResponse   {}
message RemoveInboundRequest { string tag = 1; }
message RemoveInboundResponse{}
message AlterInboundRequest  { string tag = 1; xray.common.serial.TypedMessage operation = 2; }
message AlterInboundResponse {}
message ListInboundsRequest  { bool isOnlyTags = 1; }
message ListInboundsResponse { repeated xray.core.InboundHandlerConfig inbounds = 1; }

// Newer versions add user-management RPCs:
message GetInboundUserRequest       { string tag = 1; string email = 2; }
message GetInboundUserResponse      { repeated xray.common.protocol.User users = 1; }
message GetInboundUsersCountResponse{ int64 count = 1; }

message AddOutboundRequest    { xray.core.OutboundHandlerConfig outbound = 1; }
message AddOutboundResponse   {}
message RemoveOutboundRequest { string tag = 1; }
message RemoveOutboundResponse{}
message AlterOutboundRequest  { string tag = 1; xray.common.serial.TypedMessage operation = 2; }
message AlterOutboundResponse {}

service HandlerService {
  rpc AddInbound(AddInboundRequest) returns (AddInboundResponse);
  rpc RemoveInbound(RemoveInboundRequest) returns (RemoveInboundResponse);
  rpc AlterInbound(AlterInboundRequest) returns (AlterInboundResponse);
  rpc ListInbounds(ListInboundsRequest) returns (ListInboundsResponse);
  // version-dependent:
  rpc GetInboundUsers(GetInboundUserRequest) returns (GetInboundUserResponse);
  rpc GetInboundUsersCount(GetInboundUserRequest) returns (GetInboundUsersCountResponse);
  rpc AddOutbound(AddOutboundRequest) returns (AddOutboundResponse);
  rpc RemoveOutbound(RemoveOutboundRequest) returns (RemoveOutboundResponse);
  rpc AlterOutbound(AlterOutboundRequest) returns (AlterOutboundResponse);
}
Key subtlety: AlterInbound/AlterOutbound carry a polymorphic TypedMessage operation. Adding a user historically means serializing AddUserOperation into a TypedMessage — sending the User directly is insufficient.

5.3 StatsService — app/stats/command/command.proto

syntax = "proto3";
package xray.app.stats.command;

message Stat { string name = 1; int64 value = 2; }
message GetStatsRequest    { string name = 1; bool reset = 2; }
message GetStatsResponse   { Stat stat = 1; }
message QueryStatsRequest  { string pattern = 1; bool reset = 2; }
message QueryStatsResponse { repeated Stat stat = 1; }
message SysStatsRequest    {}
message SysStatsResponse {
  uint32 NumGoroutine = 1; uint32 NumGC = 2;
  uint64 Alloc = 3; uint64 TotalAlloc = 4; uint64 Sys = 5; uint64 Mallocs = 6;
  uint64 Frees = 7; uint64 LiveObjects = 8; uint64 PauseTotalNs = 9; uint32 Uptime = 10;
}

service StatsService {
  rpc GetStats(GetStatsRequest) returns (GetStatsResponse);
  rpc QueryStats(QueryStatsRequest) returns (QueryStatsResponse);
  rpc GetSysStats(SysStatsRequest) returns (SysStatsResponse);
  // version-dependent online-user/IP RPCs (GetStatsOnline, GetStatsOnlineIpList,
  // GetAllOnlineUsers, GetUsersStats) exist in recent builds; detect Unimplemented.
}
Stat names follow inbound>>>TAG>>>traffic>>>uplink|downlink and user>>>EMAIL>>>traffic>>>…, but only exist if stats: {} plus the matching policy counters are enabled — otherwise QueryStats returns empty even though the API works.

5.4 RoutingService — app/router/command/command.proto
RoutingContext field layout is genuinely version-divergent. Older schemas used int64 SourcePort=1/TargetPort=2, repeated string SourceIPs, GeoIP-country fields, and RuleTag=15. Current mainline uses the typed layout below. Verify against your revision.


syntax = "proto3";
package xray.app.router.command;

message RoutingContext {
  string InboundTag = 1;
  xray.common.net.Network Network = 2;
  repeated bytes SourceIPs = 3;
  repeated bytes TargetIPs = 4;
  uint32 SourcePort = 5;
  uint32 TargetPort = 6;
  string TargetDomain = 7;
  string Protocol = 8;
  string User = 9;
  map<string, string> Attributes = 10;
  repeated string OutboundGroupTags = 11;
  string OutboundTag = 12;
  // additional fields (LocalIPs/LocalPort/VlessRoute) appear in newer builds
}

message SubscribeRoutingStatsRequest { repeated string FieldSelectors = 1; }
message TestRouteRequest {
  RoutingContext RoutingContext = 1;
  repeated string FieldSelectors = 2;
  bool PublishResult = 3;
}
message OverrideBalancerTargetRequest  { string balancerTag = 1; string target = 2; }
message OverrideBalancerTargetResponse {}
message AddRuleRequest  { xray.common.serial.TypedMessage config = 1; bool shouldAppend = 2; }
message AddRuleResponse {}

service RoutingService {
  rpc SubscribeRoutingStats(SubscribeRoutingStatsRequest) returns (stream RoutingContext);
  rpc TestRoute(TestRouteRequest) returns (RoutingContext);
  rpc OverrideBalancerTarget(OverrideBalancerTargetRequest) returns (OverrideBalancerTargetResponse);
  rpc AddRule(AddRuleRequest) returns (AddRuleResponse);
  // some builds add GetBalancerInfo, RemoveRule, ListRule
}
SubscribeRoutingStats is the one server-streaming control RPC — over UDS it gives a live, low-latency feed of routing decisions with zero network exposure.

5.5 LoggerService — app/log/command/config.proto
Minimal; the proto lives in config.proto despite the directory holding command.go.


syntax = "proto3";
package xray.app.log.command;

message RestartLoggerRequest  {}
message RestartLoggerResponse {}

service LoggerService {
  rpc RestartLogger(RestartLoggerRequest) returns (RestartLoggerResponse);
}
Mainline exposes only RestartLogger; some forks add a streaming FollowLog — don't depend on it universally.

5.6 Transport config messages

// transport/internet/grpc/config.proto
message Config {
  string authority = 1;            // older forks name field 1 "host" (same wire tag)
  string service_name = 2;
  bool   multi_mode = 3;
  int32  idle_timeout = 4;
  int32  health_check_timeout = 5;
  bool   permit_without_stream = 6;
  int32  initial_windows_size = 7;
  string user_agent = 8;
}

// transport/internet/domainsocket/config.proto
message Config {
  string path = 1;
  bool   abstract = 2;
  bool   padding = 3;
}
5.7 How services map onto one socket
All commander services share one gRPC server and one listener. Over a single UDS:


/run/xray/api.sock
  └─ HTTP/2 connection
      ├─ stream 1: /xray.app.stats.command.StatsService/GetSysStats
      ├─ stream 3: /xray.app.proxyman.command.HandlerService/ListInbounds
      └─ stream 5: /xray.app.router.command.RoutingService/SubscribeRoutingStats
The socket does not map one-to-one to a service. Xray also registers legacy v2ray.core.* names alongside xray.app.* for backward compatibility; reflection output shows both. New code should use the xray.app.* stubs matching its revision — a v2ray.core.*/xray.app.* mismatch produces "symbol not found"/UNIMPLEMENTED.

6. Best Practices & Pitfalls
Security — the core argument for UDS
The management API is unauthenticated by design. Any caller who reaches the socket may reset counters, enumerate users, add/remove inbounds/outbounds, add/remove users, modify routing/balancers, and restart logging. Therefore:

Never bind the API to 0.0.0.0 (the recurring real-world compromise). TLS alone does not supply authorization.
Prefer UDS + filesystem permissions. This reduces authorization to a kernel-enforced ACL — far stronger and simpler than firewalling a TCP port.
If remote control is genuinely required, front the UDS with an authenticated local gateway or an SSH tunnel with an explicit access policy.
Recommended socket layout

/run/xray            xray:xrayctl  0750
/run/xray/api.sock   xray:xrayctl  0660
The controller runs in group xrayctl. A systemd unit provisions this atomically:


RuntimeDirectory=xray
RuntimeDirectoryMode=0750
UMask=0007
Remember: the parent directory needs execute/search permission for clients to connect, and the socket mode is filtered by the creating process's umask. Don't "fix" permission errors by setting 0666.

Path location: /run vs /dev/shm vs /tmp
/run — best default: tmpfs, cleared at boot, FHS-correct for runtime sockets, easily provisioned via systemd RuntimeDirectory/tmpfiles.d.
/dev/shm — usable and fast (tmpfs) but unconventional and typically world-accessible; use a private subdirectory. No meaningful perf gain — a UDS payload isn't stored as a file.
/tmp — least desirable: world-writable, stale-name/symlink risk, and background sweepers (tmpfiles.d) can silently delete an in-use socket. Never blindly rm -f a /tmp socket from a privileged process without verifying it's actually a socket in a safely owned directory.
Abstract (@name) — self-cleaning and container-friendly, but no permission bits, Linux-only, and no SO_PEERCRED authorization by default in the commander. For a privileged API, prefer a filesystem path in /run.
Performance: UDS vs loopback TCP
UDS avoids IP routing, ephemeral-port allocation, IP/TCP header processing, and accidental non-loopback exposure. But be realistic: data still crosses the kernel/userspace boundary and is buffered/copied, and gRPC's HTTP/2 framing, flow control, HPACK, and Protobuf encoding remain. For the low-rate control API, security and operational simplicity matter far more than the marginal throughput win. For the data plane, gains are small since crypto and the remote network dominate. All of this is qualitative — no reliable benchmark deltas are established here; measure on your kernel/workload.

Keepalive / idle timeout
grpcSettings fields configure the data-plane transport, not the commander. The commander uses gRPC server defaults, adjustable only in code, not by attaching grpcSettings to the API config.
For control clients: reuse one ClientConn, set explicit RPC deadlines (long/cancellable for streaming subscriptions), handle GOAWAY/Unavailable, and use conservative keepalive. permit_without_stream: true keeps a channel warm with no active RPC — useful for a long-lived control channel, wasteful otherwise.
For the data plane: too-short idle_timeout causes stream churn; too-aggressive keepalive can trip gRPC enforcement even locally; a larger initial_windows_size helps high-BDP links but wastes memory over a local UDS.
Common errors
bind: address already in use (EADDRINUSE) — stale filesystem socket after a crash; unlink before rebind (verify owner first) or use an abstract socket. Also appears if a placeholder TCP port collides.
permission denied — controller UID/GID not in the socket's group; a parent directory lacks x; or SELinux/AppArmor/service-sandbox confinement. Fix the whole directory chain.
no such file / connection refused — Xray hasn't created the socket yet, path differs across container mounts, or a live socket file has no listener.
transport: Error while dialing — wrong UDS target syntax, using a TCP resolver target without a Unix dialer, dialing a GunService proxy socket with a management client (or vice versa), or a TLS/plaintext mismatch.
UNIMPLEMENTED — service omitted from api.services, method too new for the build, wrong package/service name, connected to the wrong socket, or v2ray.core.* vs xray.app.* mismatch.
Abstract socket works on Linux only — expected; use filesystem paths for portable controllers and where group-based access control is required.
Socket path too long — shorten it (sun_path ~108 bytes on Linux).
7. Practical Recommendation
For local administrative IPC over Xray:

Use a recent build's direct api.listen UDS if available; otherwise use the traditional API-outbound + routing pattern with a domainsocket inbound (most version-robust). Native tunnel-on-UDS works on builds that include commit e61eeae.
Place the socket in /run/xray — 0750 directory, 0660 socket owned by a dedicated controller group.
Do not add streamSettings.network: "grpc" to the API inbound; that selects the separate proxy GunService transport (gRPC-over-gRPC).
Reuse one gRPC channel, set deadlines, avoid aggressive keepalive.
Keep the API off TCP entirely unless a separate authenticated layer is deliberately added.
Generate controller stubs from the exact Xray-core revision you manage — especially for the version-dependent stats/routing RPCs and the RoutingContext/MultiHunk field layouts flagged above.
Keep the two "gRPCs" mentally separate throughout: the commander (HandlerService/StatsService/RoutingService/LoggerService, structured management RPCs) and the stream transport (GunService/Hunk, opaque proxy bytes). They share only the letters "gRPC," and UDS is merely the reliable byte transport beneath HTTP/2 in both cases.
