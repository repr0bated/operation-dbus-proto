# Vendored Xray-core commander protos

Source: https://github.com/XTLS/Xray-core
Tag: v26.3.27
Commit: d2758a023cd7f4174a5a5fa4ff66e487d4342ba0

Matches the `xray version` output of the binary actually running in the
`xray` container. These field layouts (especially `RoutingContext` and
`StatsService`'s method set) are version-dependent — re-vendor from the
exact running revision if the xray binary is ever upgraded.

Only the commander control-plane services are vendored here
(StatsService, RoutingService, LoggerService) — not HandlerService
(inbound/outbound mutation, out of scope) and not the data-plane
GRPCService/Hunk transport (unrelated to this daemon).
