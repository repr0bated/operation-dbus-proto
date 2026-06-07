//! D-Bus execution proxy for plugin subprocesses (netclient, wgcf, xray).
//!
//! Exposes a new object path `/org/opdbus/execution` -> `org.opdbus.execution`
//! which allows the UI and `op-plugins` to run system commands natively over
//! D-Bus, without shelling out in the application layer.

use anyhow::Result;
use tracing::debug;
use zbus::interface;

pub struct PluginExecutionService;

impl PluginExecutionService {
    pub fn new() -> Self {
        Self
    }
}

#[interface(name = "org.opdbus.execution")]
impl PluginExecutionService {
    /// Run the `netclient` command with the provided JSON arguments.
    async fn run_netclient(&self, args_json: &str) -> String {
        debug!("execution.run_netclient args={}", args_json);
        run_command("netclient", args_json).await
    }

    /// Run the `wgcf` command with the provided JSON arguments.
    async fn run_wgcf(&self, args_json: &str) -> String {
        debug!("execution.run_wgcf args={}", args_json);
        run_command("wgcf", args_json).await
    }

    /// Run the `xray` command with the provided JSON arguments.
    async fn run_xray(&self, args_json: &str) -> String {
        debug!("execution.run_xray args={}", args_json);
        run_command("xray", args_json).await
    }
}

async fn run_command(binary: &str, args_json: &str) -> String {
    let extra_args: Vec<String> = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(e) => return json_error(&format!("invalid args JSON: {}", e)),
    };

    let mut cmd = tokio::process::Command::new(binary);
    for a in extra_args {
        cmd.arg(a);
    }

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            if output.status.success() {
                serde_json::json!({ "ok": true, "stdout": stdout.trim(), "stderr": stderr.trim() })
                    .to_string()
            } else {
                json_error(&format!("{} exited {}: {}", binary, code, stderr.trim()))
            }
        }
        Err(e) => json_error(&format!("failed to spawn {}: {}", binary, e)),
    }
}

fn json_error(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}
