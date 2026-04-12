//! Shared helpers for server-side D-Bus connections.

use crate::types::BusType;
use anyhow::{anyhow, bail, Context, Result};
use std::env;
use zbus::{connection::Builder, fdo::DBusProxy, names::BusName, Address, Connection};

/// Resolve the concrete D-Bus address to use for a server connection.
///
/// Session bus service startup must not silently fall back to a different bus;
/// it should bind to the configured session bus socket explicitly.
pub fn resolve_bus_address(bus_type: BusType) -> Result<Address> {
    match bus_type {
        BusType::System => match env::var("DBUS_SYSTEM_BUS_ADDRESS") {
            Ok(value) => Address::try_from(value.as_str())
                .with_context(|| format!("invalid DBUS_SYSTEM_BUS_ADDRESS: {value}")),
            Err(_) => Address::system().context("failed to resolve system bus address"),
        },
        BusType::Session => {
            let value = env::var("DBUS_SESSION_BUS_ADDRESS")
                .context("DBUS_SESSION_BUS_ADDRESS must be set for session bus services")?;
            Address::try_from(value.as_str())
                .with_context(|| format!("invalid DBUS_SESSION_BUS_ADDRESS: {value}"))
        }
    }
}

/// Connect to the configured bus and claim a well-known name.
pub async fn connect_and_claim_name(
    bus_type: BusType,
    well_known_name: &'static str,
) -> Result<Connection> {
    let address = resolve_bus_address(bus_type)?;
    tracing::info!(
        bus_type = %bus_type,
        address = %address,
        name = well_known_name,
        "Connecting D-Bus service"
    );

    let connection = Builder::address(address.clone())?
        .name(well_known_name)?
        .build()
        .await
        .with_context(|| {
            format!("failed to connect to {bus_type} bus at {address} and claim {well_known_name}")
        })?;

    assert_name_owner(&connection, well_known_name).await?;

    Ok(connection)
}

/// Request an additional well-known name on an existing connection.
pub async fn request_additional_name(
    connection: &Connection,
    well_known_name: &str,
) -> Result<()> {
    connection.request_name(well_known_name).await
        .with_context(|| format!("failed to request additional name: {well_known_name}"))?;

    assert_name_owner(connection, well_known_name).await?;

    Ok(())
}

/// Verify that the current connection owns the expected well-known name.
pub async fn assert_name_owner(connection: &Connection, well_known_name: &str) -> Result<()> {
    let unique_name = connection
        .unique_name()
        .cloned()
        .ok_or_else(|| anyhow!("D-Bus connection has no unique name"))?;
    let bus_name = BusName::try_from(well_known_name)
        .with_context(|| format!("invalid D-Bus name: {well_known_name}"))?;
    let proxy = DBusProxy::new(connection).await?;
    let has_owner = proxy.name_has_owner(bus_name.clone()).await?;

    if !has_owner {
        bail!(
            "connected to D-Bus as {} but '{}' has no owner after name request",
            unique_name,
            well_known_name
        );
    }

    let owner = proxy.get_name_owner(bus_name).await?;
    if owner != unique_name {
        bail!(
            "connected to D-Bus as {} but '{}' is owned by {}",
            unique_name,
            well_known_name,
            owner
        );
    }

    tracing::info!(
        name = well_known_name,
        owner = %owner,
        "D-Bus name claimed"
    );

    Ok(())
}
