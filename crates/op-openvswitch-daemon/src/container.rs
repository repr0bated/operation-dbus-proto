//! Container instance name to host PID resolution via incus.

use anyhow::{Context, Result};

/// Resolve an incus instance name to its host-side PID.
///
/// Uses `incus info <name>` to get the init PID.
pub fn pid_from_instance_name(name: &str) -> Result<u32> {
    let output = std::process::Command::new("incus")
        .args(["info", name])
        .output()
        .context("spawn incus info")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("incus info {} failed: {}", name, stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("PID:") {
            let pid_str = trimmed.split(':').nth(1).unwrap_or("").trim();
            let pid: u32 = pid_str
                .parse()
                .with_context(|| format!("parse PID from '{}'", trimmed))?;
            if pid > 0 {
                return Ok(pid);
            }
        }
    }

    anyhow::bail!("no PID found in incus info for {}", name)
}
