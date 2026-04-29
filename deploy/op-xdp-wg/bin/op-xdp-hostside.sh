#!/usr/bin/env bash
# incus lxc.hook.post-start — runs on host after wg-xray starts.
# Sets up XDP steering, TC mirrors, NDP proxy, and container IPs.
set -euo pipefail

ETH="eth0"
VETH="veth-warp"
CT="wg-xray"
CT_IPV6="2607:5300:205:200::5bc7"
GW="2607:5300:205:200::1"

ETH_IF=$(ip -o link show $ETH | awk -F: '{print $1}')
VETH_IF=$(ip -o link show $VETH | awk -F: '{print $1}')

ip link set $VETH up

# clean old XDP/TC
ip link set $ETH xdp off 2>/dev/null || true
ip link set $VETH xdp off 2>/dev/null || true
tc qdisc del dev $ETH clsact 2>/dev/null || true

# XDP on eth0 – steer by IPv6 dst
# 2607:5300:0205:0200:0000:0000:0000:5bc7
cat > /tmp/uplink.c <<EOF
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ipv6.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>
#define VETH ${VETH_IF}
SEC("xdp")
int xdp_steer(struct xdp_md *ctx){
    void *de=(void*)(long)ctx->data_end, *da=(void*)(long)ctx->data;
    struct ethhdr *h=da; if(da+sizeof(*h)>de) return XDP_PASS;
    if(h->h_proto!=bpf_htons(0x86DD)) return XDP_PASS;
    struct ipv6hdr *ip=da+sizeof(*h); if((void*)ip+sizeof(*ip)>de) return XDP_PASS;
    __u8 ct[16]={0x26,0x07,0x53,0x00,0x02,0x05,0x02,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x5b,0xc7};
    if(__builtin_memcmp(&ip->daddr,ct,16)==0) return bpf_redirect(VETH,0);
    return XDP_PASS;
}
char _license[] SEC("license")="GPL";
EOF
clang -O2 -target bpf -c /tmp/uplink.c -o /tmp/uplink.o
ip link set $ETH xdp obj /tmp/uplink.o sec xdp

# TC mirrors for NDP/DHCPv6
tc qdisc add dev $ETH clsact
for t in 133 134 135 136; do
  tc filter add dev $ETH ingress protocol ipv6 pref 1 \
    flower ip_proto icmpv6 type $t action mirred egress mirror dev $VETH
done
tc filter add dev $ETH ingress protocol ipv6 pref 2 \
  flower ip_proto udp dst_port 546 action mirred egress mirror dev $VETH
tc filter add dev $ETH ingress protocol ipv6 pref 2 \
  flower ip_proto udp dst_port 547 action mirred egress mirror dev $VETH

# NDP proxy
sysctl -w net.ipv6.conf.$ETH.proxy_ndp=1 >/dev/null
sysctl -w net.ipv6.conf.all.forwarding=1 >/dev/null
ip -6 neigh replace proxy $CT_IPV6 dev $ETH
ip -6 route replace $CT_IPV6/128 dev $VETH

# container IPs
until incus exec $CT -- true 2>/dev/null; do sleep 1; done
incus exec $CT -- ip -6 addr add $CT_IPV6/64 dev eth0 2>/dev/null || true
incus exec $CT -- ip -6 route replace default via $GW dev eth0
incus exec $CT -- ip addr add 10.200.0.1/30 dev eth0 2>/dev/null || true

echo "=== $(date -u) op-xdp-hostside READY ==="
ip -br link show $ETH || true
ip -br link show $VETH || true
