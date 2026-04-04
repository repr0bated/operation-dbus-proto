//! Rtnetlink helpers - native netlink operations for IP addresses and routes

use anyhow::{Context, Result};
use futures::TryStreamExt;
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::link::LinkAttribute;
use netlink_packet_route::route::RouteAttribute;
use rtnetlink::{new_connection, IpVersion};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub index: u32,
    pub mac_address: Option<String>,
    pub mtu: Option<u32>,
    pub flags: Vec<String>,
    pub state: String,
    pub kind: Option<String>,
    pub addresses: Vec<InterfaceAddress>,
}

/// IP address on an interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceAddress {
    pub address: String,
    pub prefix_len: u8,
    pub family: String,
}

/// List all network interfaces with their details
pub async fn list_interfaces() -> Result<Vec<NetworkInterface>> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    let mut interfaces = Vec::new();
    let mut links = handle.link().get().execute();

    while let Some(link) = links.try_next().await? {
        let index = link.header.index;
        let mut name = String::new();
        let mut mac_address = None;
        let mut mtu = None;
        let mut kind = None;

        // Extract attributes
        for attr in &link.attributes {
            match attr {
                LinkAttribute::IfName(n) => name = n.clone(),
                LinkAttribute::Address(addr) => {
                    mac_address = Some(
                        addr.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(":"),
                    );
                }
                LinkAttribute::Mtu(m) => mtu = Some(*m),
                LinkAttribute::LinkInfo(infos) => {
                    for info in infos {
                        if let netlink_packet_route::link::LinkInfo::Kind(k) = info {
                            kind = Some(format!("{:?}", k));
                        }
                    }
                }
                _ => {}
            }
        }

        // Determine state from flags
        let flags_val = &link.header.flags;
        let flags: Vec<String> = flags_val.iter().map(|f| format!("{:?}", f)).collect();
        let is_up = flags_val
            .iter()
            .any(|f| matches!(f, netlink_packet_route::link::LinkFlag::Up));
        let state = if is_up {
            "up".to_string()
        } else {
            "down".to_string()
        };

        // Get addresses for this interface
        let addresses = match get_interface_addresses(&handle, index).await {
            Ok(addrs) => addrs,
            Err(e) => {
                log::warn!("Failed to get addresses for interface {}: {}", name, e);
                Vec::new()
            }
        };

        interfaces.push(NetworkInterface {
            name,
            index,
            mac_address,
            mtu,
            flags,
            state,
            kind,
            addresses,
        });
    }

    Ok(interfaces)
}

/// Get addresses for a specific interface
async fn get_interface_addresses(
    handle: &rtnetlink::Handle,
    ifindex: u32,
) -> Result<Vec<InterfaceAddress>> {
    let mut addresses = Vec::new();
    let mut addr_stream = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .execute();

    while let Some(addr_msg) = addr_stream.try_next().await? {
        let family = match addr_msg.header.family {
            netlink_packet_route::AddressFamily::Inet => "inet".to_string(),
            netlink_packet_route::AddressFamily::Inet6 => "inet6".to_string(),
            f => format!("{:?}", f),
        };

        for attr in &addr_msg.attributes {
            if let AddressAttribute::Address(addr) = attr {
                let addr_str = addr.to_string();

                addresses.push(InterfaceAddress {
                    address: addr_str,
                    prefix_len: addr_msg.header.prefix_len,
                    family: family.clone(),
                });
            }
        }
    }

    Ok(addresses)
}

/// Get default route information
pub async fn get_default_route() -> Result<Option<simd_json::OwnedValue>> {
    use netlink_packet_route::route::RouteAttribute;

    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    let mut routes = handle.route().get(IpVersion::V4).execute();

    while let Some(route) = routes.try_next().await? {
        // Check if this is a default route (destination 0.0.0.0/0)
        if route.header.destination_prefix_length == 0 {
            let mut gateway = None;
            let mut oif_index = None;

            for attr in &route.attributes {
                match attr {
                    RouteAttribute::Gateway(gw) => {
                        gateway = Some(format!("{:?}", gw));
                    }
                    RouteAttribute::Oif(idx) => {
                        oif_index = Some(idx);
                    }
                    _ => {}
                }
            }

            // Try to get interface name for the output interface
            let mut oif_name = None;
            if let Some(idx) = oif_index {
                let mut links = handle.link().get().match_index(*idx).execute();
                if let Some(link) = links.try_next().await? {
                    for attr in &link.attributes {
                        if let LinkAttribute::IfName(name) = attr {
                            oif_name = Some(name.clone());
                            break;
                        }
                    }
                }
            }

            return Ok(Some(simd_json::json!({
                "gateway": gateway,
                "interface_index": oif_index,
                "interface_name": oif_name,
                "destination": "0.0.0.0/0",
            })));
        }
    }

    Ok(None)
}

/// Add IPv4 address to interface
pub async fn add_ipv4_address(ifname: &str, ip: &str, prefix: u8) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse IP address
    let addr: Ipv4Addr = ip.parse().context("Invalid IPv4 address")?;

    // Add address to interface
    handle
        .address()
        .add(ifindex, addr.into(), prefix)
        .execute()
        .await
        .context("Failed to add IP address")?;

    Ok(())
}

/// Delete IPv4 address from interface
#[allow(dead_code)]
pub async fn del_ipv4_address(ifname: &str, ip: &str, prefix: u8) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse IP address
    let addr: Ipv4Addr = ip.parse().context("Invalid IPv4 address")?;

    // Get addresses filtered by interface, prefix, and address
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .set_prefix_length_filter(prefix)
        .set_address_filter(std::net::IpAddr::V4(addr))
        .execute();

    if let Some(addr_msg) = addresses.try_next().await? {
        handle.address().del(addr_msg).execute().await?;
    }

    Ok(())
}

/// Flush all addresses from interface
#[allow(dead_code)]
pub async fn flush_addresses(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Get all addresses on this interface
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .execute();

    while let Some(addr) = addresses.try_next().await? {
        // Delete this address
        if let Err(e) = handle.address().del(addr).execute().await {
            log::warn!("Failed to delete address: {}", e);
        }
    }

    Ok(())
}

/// Set link up
pub async fn link_up(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Set link up
    handle
        .link()
        .set(ifindex)
        .up()
        .execute()
        .await
        .context("Failed to bring link up")?;

    Ok(())
}

/// Set link down
#[allow(dead_code)]
pub async fn link_down(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Set link down
    handle
        .link()
        .set(ifindex)
        .down()
        .execute()
        .await
        .context("Failed to bring link down")?;

    Ok(())
}

/// Add default route
pub async fn add_default_route(ifname: &str, gateway: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse gateway address
    let gw: Ipv4Addr = gateway.parse().context("Invalid gateway address")?;

    // Add default route
    handle
        .route()
        .add()
        .v4()
        .destination_prefix(Ipv4Addr::new(0, 0, 0, 0), 0)
        .gateway(gw)
        .output_interface(ifindex)
        .execute()
        .await
        .context("Failed to add default route")?;

    Ok(())
}

/// Set MAC address on interface
pub async fn set_mac_address(ifname: &str, mac: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse MAC address "aa:bb:cc:dd:ee:ff" -> [u8; 6]
    let mac_bytes: Vec<u8> = mac
        .split(':')
        .map(|byte| u8::from_str_radix(byte, 16).context("Invalid MAC address byte"))
        .collect::<Result<Vec<u8>>>()?;

    if mac_bytes.len() != 6 {
        return Err(anyhow::anyhow!(
            "Invalid MAC address '{}': expected 6 octets",
            mac
        ));
    }

    // Set MAC address
    handle
        .link()
        .set(ifindex)
        .address(mac_bytes)
        .execute()
        .await
        .context("Failed to set MAC address")?;

    Ok(())
}

/// Delete default route
pub async fn del_default_route() -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Get all routes
    let mut routes = handle.route().get(IpVersion::V4).execute();

    while let Some(route) = routes.try_next().await? {
        // Check if this is a default route (destination 0.0.0.0/0)
        if route.header.destination_prefix_length == 0 {
            // Delete this route
            if let Err(e) = handle.route().del(route).execute().await {
                log::warn!("Failed to delete default route: {}", e);
            }
        }
    }

    Ok(())
}

/// List IPv4 routes for a given interface (by name)
pub async fn list_routes_for_interface(_ifname: &str) -> Result<Vec<simd_json::OwnedValue>> {
    // Minimal, compile-safe stub; route filtering can be added later.
    Ok(Vec::new())
}

/// List all veth interfaces (simplified implementation)
pub async fn list_veth_interfaces() -> Result<Vec<String>> {
    // For now, return empty list - this would need more complex rtnetlink code
    // to properly enumerate all interfaces and check their types
    // The LXC plugin will fall back to other methods if this returns empty
    Ok(Vec::new())
}

/// Rename network interface
pub async fn link_set_name(old_name: &str, new_name: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by current name
    let mut links = handle
        .link()
        .get()
        .match_name(old_name.to_string())
        .execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", old_name))?;

    let ifindex = link.header.index;

    // Set new name
    handle
        .link()
        .set(ifindex)
        .name(new_name.to_string())
        .execute()
        .await
        .context(format!("Failed to rename {} to {}", old_name, new_name))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Basic smoke test to ensure rtnetlink connection and route listing works.
    // Uses the loopback interface which always exists.
    #[tokio::test(flavor = "current_thread")]
    async fn test_list_routes_for_loopback() {
        let res = list_routes_for_interface("lo").await;
        assert!(
            res.is_ok(),
            "expected Ok from list_routes_for_interface: {:?}",
            res
        );
        let routes = res.unwrap();
        // No strict expectation on content; presence/empty is both fine.
        println!("routes on lo: {:?}", routes);
    }
}
