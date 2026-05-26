//! op-xdp-wg — Rust orchestration for the wg-xray XDP container path.
//!
//! This binary keeps the existing interface names:
//! - host uplink: `eth0`
//! - Incus host-side peer: `veth-warp`
//! - container: `wg-xray`
//!
//! The BPF program runs on host `eth0` and redirects matching traffic to the
//! Incus peer `veth-warp`. The Incus hook order is explicit:
//! 1. `prepare`: set the container MAC and restart the container.
//! 2. `hostside`: attach XDP after `veth-warp` exists and the container is up.
//! 3. `detach`: remove XDP before container shutdown.
//! 4. `watch`: verify the state and reapply the hostside setup if needed.

use anyhow::{anyhow, bail, Context, Result};
use op_network::rtnetlink;
use std::fmt::Write as _;
use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const CT: &str = "wg-xray";
const HOST_IF: &str = "eth0";
const VETH: &str = "grpc-uplink";
const HOST_MAC: &str = "fa:16:3e:f1:71:d2";
const CT_IPV6: &str = "2607:5300:205:200::5bc7";
const WATCH_INTERVAL_SECS: u64 = 60;
const BPF_DIR: &str = "/etc/op-network/xdp";
const BPF_C_PATH: &str = "/etc/op-network/xdp/op-xdp-wg.c";
const BPF_O_PATH: &str = "/etc/op-network/xdp/op-xdp-wg.o";
const XDP_PROG_NAME: &str = "xdp_steer";
const XDP_PRIO: &str = "50";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Prepare,
    Hostside,
    Chain,
    Detach,
    Watch,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_network=info".parse()?))
        .init();

    let mode = parse_mode()?;
    match mode {
        Mode::Prepare => prepare().await,
        Mode::Hostside => hostside().await,
        Mode::Chain => chain_xdp().await,
        Mode::Detach => detach().await,
        Mode::Watch => watch().await,
    }
}

fn parse_mode() -> Result<Mode> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hostside".to_string());
    match mode.as_str() {
        "prepare" => Ok(Mode::Prepare),
        "hostside" | "attach" => Ok(Mode::Hostside),
        "chain" | "rechain" | "xdp-only" => Ok(Mode::Chain),
        "detach" | "teardown" => Ok(Mode::Detach),
        "watch" => Ok(Mode::Watch),
        other => bail!("unknown op-xdp-wg mode: {}", other),
    }
}

async fn prepare() -> Result<()> {
    wait_for_incus_instance(CT).await?;
    let _ = run("incus", ["stop", CT, "--force"]);
    run(
        "incus",
        [
            "config",
            "set",
            CT,
            &format!("volatile.eth0.hwaddr={HOST_MAC}"),
        ],
    )?;
    run("incus", ["start", CT])?;
    info!(container = CT, "container restarted with host MAC");
    Ok(())
}

async fn hostside() -> Result<()> {
    // When called as an LXC start-host hook, LXC_PID is set to the container
    // init PID. Do NOT call incus info/list here — incusd is blocked waiting
    // for this hook to return, so any incus command would deadlock.
    let ct_pid = hook_pid_or_query(CT)?;

    chain_xdp().await?;

    configure_tc()?;
    configure_container_network(ct_pid)?;

    info!(
        host = HOST_IF,
        redirect = VETH,
        container = CT,
        "XDP hostside setup complete"
    );
    Ok(())
}

async fn chain_xdp() -> Result<()> {
    wait_for_interface(HOST_IF).await?;
    wait_for_interface(VETH).await?;

    let veth_ifindex = interface_index(VETH).await?;
    compile_bpf(veth_ifindex)?;

    bring_link_up(HOST_IF).await?;
    bring_link_up(VETH).await?;

    // Use xdp-loader (libxdp dispatcher) so this program coexists with OVS AF_XDP.
    unload_own_xdp_program(HOST_IF)?;
    run(
        "xdp-loader",
        [
            "load",
            HOST_IF,
            BPF_O_PATH,
            "--mode",
            "native",
            "--prio",
            XDP_PRIO,
            "--actions",
            "XDP_PASS",
            "--prog-name",
            XDP_PROG_NAME,
        ],
    )?;

    info!(
        host = HOST_IF,
        redirect = VETH,
        object = BPF_O_PATH,
        "XDP program chained through dispatcher"
    );
    Ok(())
}

/// Return the container's init PID.
///
/// In hook context `LXC_PID` is already set by the LXC runtime — use it
/// directly so we never call back into incusd (which would deadlock).
/// Outside hook context (e.g. watch mode) fall back to `incus info`.
fn hook_pid_or_query(name: &str) -> Result<u32> {
    if let Ok(pid_str) = std::env::var("LXC_PID") {
        let pid = pid_str
            .trim()
            .parse::<u32>()
            .context("LXC_PID is not a valid u32")?;
        return Ok(pid);
    }
    incus_pid(name)
}

async fn detach() -> Result<()> {
    unload_own_xdp_program(HOST_IF)?;
    run_ignore("tc", ["qdisc", "del", "dev", HOST_IF, "clsact"]);
    info!(host = HOST_IF, "XDP detached");
    Ok(())
}

async fn watch() -> Result<()> {
    info!(interval = WATCH_INTERVAL_SECS, "XDP watch starting");
    loop {
        sleep(Duration::from_secs(WATCH_INTERVAL_SECS)).await;

        let ct_running = incus_is_running(CT)?;
        let host_xdp = link_has_xdp(HOST_IF).await.unwrap_or(false);
        let veth_up = link_is_up(VETH).await.unwrap_or(false);
        let tc_dup_ready = tc_dup_is_ready()?;
        let ndp_ready = ndp_is_ready()?;

        let mut failures = Vec::new();
        if !host_xdp {
            failures.push("host-xdp");
        }
        if !veth_up {
            failures.push("veth-down");
        }
        if !tc_dup_ready {
            failures.push("tc-dup");
        }
        if !ndp_ready {
            failures.push("ndp");
        }

        if failures.is_empty() {
            continue;
        }

        warn!(?failures, ct_running, "XDP watch noticed drift");
        if ct_running {
            let _ = hostside().await;
        } else {
            info!(container = CT, "container is not running; skipping restart");
        }
    }
}

async fn wait_for_interface(ifname: &str) -> Result<()> {
    for _ in 0..60 {
        if interface_exists(ifname).await? {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!("interface {} did not appear", ifname)
}

async fn wait_for_incus_instance(name: &str) -> Result<()> {
    for _ in 0..60 {
        if incus_is_running(name).unwrap_or(false) {
            return Ok(());
        }
        if incus_exists(name).unwrap_or(false) {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!("Incus instance {} did not appear", name)
}

async fn interface_exists(ifname: &str) -> Result<bool> {
    Ok(rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .any(|iface| iface.name == ifname))
}

async fn interface_index(ifname: &str) -> Result<u32> {
    rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .find(|iface| iface.name == ifname)
        .map(|iface| iface.index)
        .ok_or_else(|| anyhow!("interface {} not found", ifname))
}

async fn link_is_up(ifname: &str) -> Result<bool> {
    Ok(rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .find(|iface| iface.name == ifname)
        .map(|iface| iface.state == "up")
        .unwrap_or(false))
}

async fn link_has_xdp(ifname: &str) -> Result<bool> {
    // xdp-loader list shows programs loaded via the dispatcher
    let output = Command::new("xdp-loader")
        .args(["status", ifname])
        .output()
        .with_context(|| format!("failed to inspect {}", ifname))?;
    let out = String::from_utf8_lossy(&output.stdout);
    Ok(out.contains("xdp_steer") || out.contains("prog/xdp") || out.contains("xdp_dispatcher"))
}

fn tc_dup_is_ready() -> Result<bool> {
    let output = Command::new("tc")
        .args(["filter", "show", "dev", HOST_IF, "ingress"])
        .output()
        .context("failed to inspect tc filters")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 'mirror' is the tc keyword for port duplication/dup
    Ok(stdout.matches("mirred").count() >= 6)
}

fn ndp_is_ready() -> Result<bool> {
    let output = Command::new("ip")
        .args(["-6", "neigh", "show", "proxy"])
        .output()
        .context("failed to inspect proxy ndp state")?;
    Ok(String::from_utf8_lossy(&output.stdout).contains(CT_IPV6))
}

async fn bring_link_up(ifname: &str) -> Result<()> {
    rtnetlink::link_up(ifname)
        .await
        .with_context(|| format!("failed to bring {} up", ifname))
}

fn compile_bpf(veth_ifindex: u32) -> Result<()> {
    let mut src = String::new();
    write!(
        &mut src,
        r#"#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ipv6.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

#define VETH {veth_ifindex}

SEC("xdp")
int xdp_steer(struct xdp_md *ctx) {{
    void *de = (void *)(long)ctx->data_end;
    void *da = (void *)(long)ctx->data;
    struct ethhdr *h = da;
    if (da + sizeof(*h) > de) return XDP_PASS;
    if (h->h_proto != bpf_htons(0x86DD)) return XDP_PASS;
    struct ipv6hdr *ip = da + sizeof(*h);
    if ((void *)ip + sizeof(*ip) > de) return XDP_PASS;
    __u8 ct[16] = {{0x26,0x07,0x53,0x00,0x02,0x05,0x02,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x5b,0xc7}};
    if (__builtin_memcmp(&ip->daddr, ct, 16) == 0) return bpf_redirect(VETH, 0);
    return XDP_PASS;
}}
char _license[] SEC("license") = "GPL";
"#
    )
    .expect("write BPF source");

    fs::create_dir_all(BPF_DIR).with_context(|| format!("create {}", BPF_DIR))?;
    fs::write(BPF_C_PATH, src).with_context(|| format!("write {}", BPF_C_PATH))?;
    // -g generates BTF info required by libxdp multiprog dispatcher
    run(
        "clang",
        [
            "-O2", "-g", "-target", "bpf", "-c", BPF_C_PATH, "-o", BPF_O_PATH,
        ],
    )
}

fn configure_tc() -> Result<()> {
    run_ignore("tc", ["qdisc", "del", "dev", HOST_IF, "clsact"]);
    run("tc", ["qdisc", "add", "dev", HOST_IF, "clsact"])?;

    // Use tc mirred egress mirror to duplicate (dup) traffic to the veth.
    // This is separate from the D-Bus tree mirror.
    for t in ["133", "134", "135", "136"] {
        run(
            "tc",
            [
                "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "1",
                "flower", "ip_proto", "icmpv6", "type", t, "action", "mirred", "egress", "mirror",
                "dev", VETH,
            ],
        )?;
    }

    run(
        "tc",
        [
            "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "2", "flower",
            "ip_proto", "udp", "dst_port", "546", "action", "mirred", "egress", "mirror", "dev",
            VETH,
        ],
    )?;
    run(
        "tc",
        [
            "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "2", "flower",
            "ip_proto", "udp", "dst_port", "547", "action", "mirred", "egress", "mirror", "dev",
            VETH,
        ],
    )?;

    Ok(())
}

fn configure_container_network(_ct_pid: u32) -> Result<()> {
    // Container IPv4/IPv6 addresses are managed by systemd-networkd inside the
    // container via eth0.network (static config). Only host-side state is set here.
    run(
        "sysctl",
        [&format!("net.ipv6.conf.{}.proxy_ndp=1", HOST_IF)],
    )?;
    run("sysctl", ["-w", "net.ipv6.conf.all.forwarding=1"])?;
    run("ip", ["addr", "add", "10.200.0.2/30", "dev", VETH])
        .unwrap_or_else(|e| warn!("Failed to add IPv4 to {}: {}", VETH, e));
    run(
        "ip",
        ["-6", "neigh", "replace", "proxy", CT_IPV6, "dev", HOST_IF],
    )?;
    run(
        "ip",
        [
            "-6",
            "route",
            "replace",
            &format!("{}/128", CT_IPV6),
            "dev",
            VETH,
        ],
    )?;
    Ok(())
}

fn incus_exists(name: &str) -> Result<bool> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    Ok(output.status.success())
}

fn incus_is_running(name: &str) -> Result<bool> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    if !output.status.success() {
        return Ok(false);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().any(|line| line.trim() == "Status: RUNNING"))
}

fn incus_pid(name: &str) -> Result<u32> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    if !output.status.success() {
        bail!("no PID for {}", name);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if let Some(pid) = line.strip_prefix("PID:") {
            let pid = pid.trim().parse::<u32>().context("invalid incus PID")?;
            return Ok(pid);
        }
    }
    bail!("no PID for {}", name)
}

fn run(cmd: &str, args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> Result<()> {
    let args_vec: Vec<_> = args
        .into_iter()
        .map(|a| a.as_ref().to_os_string())
        .collect();
    let status = Command::new(cmd)
        .args(&args_vec)
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("failed to execute {}", cmd))?;
    if !status.success() {
        bail!("{} {:?} failed with {}", cmd, args_vec, status);
    }
    Ok(())
}

fn run_ignore(cmd: &str, args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) {
    let _ = run(cmd, args);
}

fn unload_own_xdp_program(ifname: &str) -> Result<()> {
    let output = Command::new("xdp-loader")
        .args(["status", ifname])
        .output()
        .with_context(|| format!("failed to inspect XDP status for {}", ifname))?;

    if !output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout
        .lines()
        .find(|line| line.split_whitespace().any(|token| token == XDP_PROG_NAME))
    else {
        return Ok(());
    };

    let tokens: Vec<_> = line.split_whitespace().collect();
    let Some(prog_pos) = tokens.iter().position(|token| *token == XDP_PROG_NAME) else {
        return Ok(());
    };
    let Some(id_token) = tokens.get(prog_pos + 1) else {
        return Ok(());
    };
    let id = id_token
        .parse::<u32>()
        .with_context(|| format!("invalid XDP program id in status line: {}", line))?;

    run_ignore("xdp-loader", ["unload", ifname, "--id", &id.to_string()]);
    Ok(())
}
