use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use zbus::{Connection, Proxy};

#[derive(Debug, serde::Deserialize)]
struct QueryStateResponse {
    plugins: HashMap<String, Value>,
}

async fn proxy(connection: &Connection) -> Result<Proxy<'_>> {
    let proxy = Proxy::new(
        connection,
        "org.opdbus",
        "/org/opdbus/state",
        "org.opdbus.StateManager",
    )
    .await
    .context("create StateManager D-Bus proxy")?;
    Ok(proxy)
}

pub async fn query_plugin_state<T>(plugin_id: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let connection = Connection::system()
        .await
        .context("connect to system D-Bus for StateManager access")?;
    let proxy = proxy(&connection).await?;
    let mut state_json: String = proxy
        .call("QueryState", &())
        .await
        .context("query current state from StateManager")?;
    let query_state: QueryStateResponse = unsafe { simd_json::from_str(&mut state_json) }
        .context("parse StateManager query_state payload")?;

    if let Some(existing) = query_state.plugins.get(plugin_id) {
        let value = simd_json::serde::from_owned_value(existing.clone())
            .with_context(|| format!("parse {} plugin state", plugin_id))?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

pub async fn apply_plugin_state<T>(plugin_id: &str, value: &T) -> Result<()>
where
    T: Serialize,
{
    let connection = Connection::system()
        .await
        .context("connect to system D-Bus for StateManager access")?;
    let proxy = proxy(&connection).await?;
    let value = simd_json::serde::to_owned_value(value)
        .with_context(|| format!("serialize {} plugin state", plugin_id))?;
    let request = simd_json::json!({
        "plugin_id": plugin_id,
        "value": value,
    });
    let request_json = simd_json::to_string(&request)
        .with_context(|| format!("encode {} contract mutation", plugin_id))?;

    let _: String = proxy
        .call("ApplyContractMutation", &(request_json,))
        .await
        .with_context(|| format!("apply {} contract mutation", plugin_id))?;
    Ok(())
}
