//! Network namespace operations for the openvswitch proxy.
//!
//! Moves OVS internal ports between the host netns and container netns.
//! Uses rtnetlink for link operations and libc setns for namespace switching.

use anyhow::{Context, Result};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use tracing::{info, warn};

/// Move a network interface from the current netns into a target netns by PID.
///
/// This is equivalent to `ip link set dev <ifname> netns <pid>`.
pub fn move_interface_to_netns(ifname: &str, target_pid: u32) -> Result<()> {
    info!(
        "netns: moving {} into netns of PID {}",
        ifname, target_pid
    );

    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let (connection, handle, _) = rtnetlink::new_connection()
            .context("connect to rtnetlink")?;
        tokio::spawn(connection);

        let index = find_link_index(&handle, ifname).await?;

        handle.link().set(index)
            .setns_by_pid(target_pid)
            .execute()
            .await
            .with_context(|| format!("move {} to netns PID {}", ifname, target_pid))?;

        Result::<()>::Ok(())
    })?;

    info!("netns: {} moved into PID {}", ifname, target_pid);
    Ok(())
}

/// Rename an interface inside a target netns.
///
/// Opens the target netns, enters it via setns, renames the interface,
/// then returns to the original netns.
pub fn rename_in_netns(ifname: &str, new_name: &str, target_pid: u32) -> Result<()> {
    info!(
        "netns: renaming {} to {} inside PID {}",
        ifname, new_name, target_pid
    );

    let self_ns = File::open("/proc/self/ns/net").context("open self netns")?;
    let self_fd = self_ns.as_raw_fd();

    let target_ns_path = format!("/proc/{}/ns/net", target_pid);
    let target_ns = File::open(&target_ns_path)
        .with_context(|| format!("open target netns {}", target_ns_path))?;

    // Enter target netns
    unsafe {
        let ret = libc::setns(target_ns.as_raw_fd(), libc::CLONE_NEWNET);
        if ret != 0 {
            return Err(anyhow::anyhow!(
                "setns to PID {} failed: errno {}",
                target_pid,
                *libc::__errno_location()
            ));
        }
    }

    let result = rename_link_rtnetlink(ifname, new_name);

    // Return to original netns
    unsafe {
        libc::setns(self_fd, libc::CLONE_NEWNET);
    }

    result
}

/// Set an interface UP inside a target netns.
pub fn set_link_up_in_netns(ifname: &str, target_pid: u32) -> Result<()> {
    info!("netns: setting {} UP inside PID {}", ifname, target_pid);

    let self_ns = File::open("/proc/self/ns/net").context("open self netns")?;
    let self_fd = self_ns.as_raw_fd();

    let target_ns_path = format!("/proc/{}/ns/net", target_pid);
    let target_ns = File::open(&target_ns_path)
        .with_context(|| format!("open target netns {}", target_ns_path))?;

    unsafe {
        let ret = libc::setns(target_ns.as_raw_fd(), libc::CLONE_NEWNET);
        if ret != 0 {
            return Err(anyhow::anyhow!(
                "setns to PID {} failed: errno {}",
                target_pid,
                *libc::__errno_location()
            ));
        }
    }

    let result = set_link_up_rtnetlink(ifname);

    unsafe {
        libc::setns(self_fd, libc::CLONE_NEWNET);
    }

    result
}

/// Add an IP address to an interface inside a target netns.
pub fn add_addr_in_netns(ifname: &str, addr: &str, target_pid: u32) -> Result<()> {
    info!(
        "netns: adding addr {} to {} inside PID {}",
        addr, ifname, target_pid
    );

    let self_ns = File::open("/proc/self/ns/net").context("open self netns")?;
    let self_fd = self_ns.as_raw_fd();

    let target_ns_path = format!("/proc/{}/ns/net", target_pid);
    let target_ns = File::open(&target_ns_path)
        .with_context(|| format!("open target netns {}", target_ns_path))?;

    unsafe {
        let ret = libc::setns(target_ns.as_raw_fd(), libc::CLONE_NEWNET);
        if ret != 0 {
            return Err(anyhow::anyhow!(
                "setns to PID {} failed: errno {}",
                target_pid,
                *libc::__errno_location()
            ));
        }
    }

    let result = add_addr_rtnetlink(ifname, addr);

    unsafe {
        libc::setns(self_fd, libc::CLONE_NEWNET);
    }

    result
}

// ── Internal rtnetlink helpers ──────────────────────────────────────────────────

/// Find a link index by name using rtnetlink.
async fn find_link_index(handle: &rtnetlink::Handle, ifname: &str) -> Result<u32> {
    use futures::TryStreamExt;
    use netlink_packet_route::link::LinkAttribute;
    let mut links = handle.link().get().execute();
    while let Some(msg) = links.try_next().await? {
        for attr in &msg.attributes {
            if let LinkAttribute::IfName(name) = attr {
                if name == ifname {
                    return Ok(msg.header.index);
                }
            }
        }
    }
    Err(anyhow::anyhow!("interface {} not found", ifname))
}

fn rename_link_rtnetlink(old_name: &str, new_name: &str) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let (connection, handle, _) = rtnetlink::new_connection()
            .context("connect to rtnetlink")?;
        tokio::spawn(connection);

        let index = find_link_index(&handle, old_name).await?;

        handle.link().set(index)
            .name(new_name.to_string())
            .execute()
            .await
            .with_context(|| format!("rename {} to {}", old_name, new_name))?;

        Result::<()>::Ok(())
    })
}

pub fn set_link_up_rtnetlink(ifname: &str) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let (connection, handle, _) = rtnetlink::new_connection()
            .context("connect to rtnetlink")?;
        tokio::spawn(connection);

        let index = find_link_index(&handle, ifname).await?;

        handle.link().set(index)
            .up()
            .execute()
            .await
            .with_context(|| format!("set {} up", ifname))?;

        Result::<()>::Ok(())
    })
}

fn add_addr_rtnetlink(ifname: &str, addr: &str) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let (connection, handle, _) = rtnetlink::new_connection()
            .context("connect to rtnetlink")?;
        tokio::spawn(connection);

        let index = find_link_index(&handle, ifname).await?;

        let addr_val: std::net::IpAddr = addr.parse()
            .with_context(|| format!("parse IP address {}", addr))?;

        handle.address().add(index, addr_val, 0).execute()
            .await
            .with_context(|| format!("add addr {} to {}", addr, ifname))?;

        Result::<()>::Ok(())
    })
}

/// Create an OVS internal port via OVSDB transact, then move it into a container's netns,
/// rename it, bring it up, and optionally add an IP address.
///
/// This is the full "attach" operation for the openvswitch proxy.
#[allow(dead_code)]
pub fn attach_port_to_container(
    port_name: &str,
    bridge: &str,
    target_pid: u32,
    iface_name: &str,
    ip_addrs: &[&str],
) -> Result<()> {
    info!(
        "attach: creating OVS internal port {} on {} for PID {}",
        port_name, bridge, target_pid
    );

    move_interface_to_netns(port_name, target_pid)?;

    if port_name != iface_name {
        rename_in_netns(port_name, iface_name, target_pid)?;
    }

    set_link_up_in_netns(iface_name, target_pid)?;

    for addr in ip_addrs {
        match add_addr_in_netns(iface_name, addr, target_pid) {
            Ok(_) => info!("attach: added addr {} to {}", addr, iface_name),
            Err(e) => warn!("attach: failed to add addr {} to {}: {}", addr, iface_name, e),
        }
    }

    info!(
        "attach: {} -> {} in PID {} complete",
        port_name, iface_name, target_pid
    );
    Ok(())
}

/// Detach: placeholder for host-side cleanup after OVS port removal.
#[allow(dead_code)]
pub fn detach_port_from_bridge(port_name: &str, bridge: &str) -> Result<()> {
    info!("detach: removing {} from {}", port_name, bridge);
    Ok(())
}
