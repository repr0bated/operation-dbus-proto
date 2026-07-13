# Control-plane network startup

Boot-time network bring-up for main/decoy nodes (Artix s6-rc). This is the
sanctioned bootstrap exception: it creates the network natively; after boot
the rovs/wireguard plugins own all mutation via D-Bus.

Install:

    sudo ./install.sh
    sudo s6 set enable control-plane-network
    sudo s6 set commit -D default && sudo s6 live install   # back-to-back!

Select `NODE_ROLE=main` or `NODE_ROLE=decoy` and override the layout in
`/etc/control-plane-network/network.conf`.

The script:
- waits for D-Bus and OVSDB;
- ensures the ovsbr0 bridge exists and strips rogue ports that resurrect
  from conf.db (ovsbr0-sock, ovsbr0-mgmt, gbr_*);
- assigns the reconciled addresses (10.200.0.1/30, 10.200.0.2/30,
  10.0.0.2/32);
- assigns the netmaker mesh address (100.90.37.254/24) if the interface
  exists — netmaker is the ONLY WireGuard interface on the host; all other
  WireGuard terminates on the oracle decoy;
- writes an IPv6-only NextDNS resolver fragment to the state dir;
- produces one startup report, advances the chatbot heartbeat, and touches
  `$STATE_DIR/<role>.ready`.

Stage failures are collected and reported (service exits nonzero) but never
stop later stages: the script brings up as much network as it can.

Socket provisioning is deliberately NOT done here: `unix_socket bind` is a
one-time mutation issued through the control plane (zcall, with
`--capability network.write --actor <id>`) when a service is provisioned,
not a boot action. noVNC, Grafana, Prometheus, Mosquitto, netclient, and
the Netmaker API are self-contained in the appliance container behind
Caddy; nothing here publishes their ports.
