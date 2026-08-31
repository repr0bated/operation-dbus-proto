# ZeroClaw Router → op-grpc-bridge Wiring

## Status: Ready for Implementation

## Context

- **Router**: OpenWrt at 100.69.0.3 (netmaker) / 10.0.0.3 (wg0) / 192.168.1.1 (LAN)
- **Host bridge**: op-grpc-bridge at 10.0.0.2:8090 (verified working)
- **Mesh**: Netmaker tunnel operational, router can reach 10.0.0.2:8090 (verified via wget)
- **ZeroClaw binary**: Already on router at /fast/zeroclaw/bin/zeroclaw

## Network Path (Verified)

```
Router (100.69.0.3) 
  → netmaker tunnel 
  → VPS egress (100.69.0.2) 
  → netmaker peer (100.69.0.1/10.0.0.2) 
  → Host op-grpc-bridge (10.0.0.2:8090)
```

Connectivity confirmed:
- `ping 10.0.0.2` from router: 2/2 received, ~158ms RTT
- `wget http://10.0.0.2:8090/` from router: successful

## Requirements

### R1: ZeroClaw Gateway Configuration
Configure zeroclaw on router to use op-grpc-bridge as its backend provider.

**Config location**: `/fast/zeroclaw/config/config.toml`

**Key settings**:
- Gateway port: 42617
- Gateway host: 0.0.0.0 (allow LAN clients)
- Provider: OpenAI-compatible pointing to `http://10.0.0.2:8090/v1`
- Auth: Ghostbridge headers required

### R2: Ghostbridge Authentication
The bridge requires identity headers on every request:
- `x-ghostbridge-footprint`: `4a99dfe92638818966af0687189b6f2c85c7807e663f15cade14d2308b848e0e`
- `x-ghostbridge-trace-id`: `8423bb899e754ded9673ac45752b5bc9`

These must be injected via zeroclaw's `extra_headers` provider config.

### R3: Service Enablement
- Init script: `/etc/init.d/zeroclaw` (procd)
- Enable and start service
- Verify listening on port 42617

### R4: End-to-End Verification
- Router gateway responds on 127.0.0.1:42617
- Request through gateway reaches bridge and returns valid response
- LAN clients can access router:42617

## Out of Scope (This Pass)
- Salad provider secrets configuration
- odbus plugin schema regeneration
- Chatbot ↔ zeroclaw integration
