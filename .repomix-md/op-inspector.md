This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-inspector/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-inspector/
              src/
                cli.rs
                datadump.rs
                gcloud.rs
                introspective_gadget.rs
                lib.rs
              ADAPTER-WORKFLOW.md
              Cargo.toml
              compare-op-inspector.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/src/cli.rs">
//! Generic CLI Introspection Adapter
//!
//! Generalizes the gcloud introspection pattern into a reusable framework
//! for ANY CLI program (incus, docker, kubectl, helm, etc.).
//!
//! Handles both Go/cobra-style and Python/click-style help output formats,
//! as well as gcloud's ALL-CAPS section headers.
//!
//! # Usage
//!
//! ```rust,no_run
//! use op_inspector::cli::{CliParser, introspect_cli};
//!
//! // Quick introspection
//! let schema = introspect_cli("incus", 3).await?;
//!
//! // Custom help flag (e.g., for programs that use "-h" only)
//! let parser = CliParser::with_help_flag("mytool", "-h");
//! let schema = parser.introspect_full(2).await?;
//! ```

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Root schema for a CLI program's command hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSchema {
    /// The program name (e.g., "incus", "docker", "kubectl")
    pub program: String,
    /// Version string as reported by the program
    pub version: String,
    /// Root command hierarchy
    pub hierarchy: CliCommand,
    /// Introspection statistics
    pub statistics: CliStats,
}

/// A command or command group within the CLI hierarchy (recursive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// Command name (e.g., "list", "config", "admin")
    pub name: String,
    /// Full command path (e.g., "incus config edit")
    pub full_path: String,
    /// Human-readable description
    pub description: String,
    /// Whether this node is a group (has subcommands)
    pub is_group: bool,
    /// Available flags / options
    pub flags: Vec<CliFlag>,
    /// Positional arguments
    pub positional_args: Vec<CliArg>,
    /// Subcommands keyed by name
    pub subcommands: HashMap<String, CliCommand>,
}

impl Default for CliCommand {
    fn default() -> Self {
        Self {
            name: "root".to_string(),
            full_path: String::new(),
            description: String::new(),
            is_group: true,
            flags: vec![],
            positional_args: vec![],
            subcommands: HashMap::new(),
        }
    }
}

/// A CLI flag / option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliFlag {
    /// Long flag name including dashes (e.g., "--format")
    pub name: String,
    /// Optional short flag (e.g., "-f")
    pub short_name: Option<String>,
    /// Description text
    pub description: String,
    /// Whether the flag is required
    pub required: bool,
    /// Inferred value type ("string", "integer", "boolean")
    pub value_type: String,
    /// Default value if any
    pub default: Option<String>,
    /// Allowed choices if any
    pub choices: Vec<String>,
}

/// A positional argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArg {
    /// Argument name
    pub name: String,
    /// Description text
    pub description: String,
    /// Whether the argument is required
    pub required: bool,
}

/// Statistics gathered during introspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStats {
    pub total_groups: usize,
    pub total_commands: usize,
    pub total_flags: usize,
    pub introspection_time_ms: u128,
}

impl Default for CliStats {
    fn default() -> Self {
        Self {
            total_groups: 0,
            total_commands: 0,
            total_flags: 0,
            introspection_time_ms: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// CliParser
// ---------------------------------------------------------------------------

/// Generic CLI introspection parser.
///
/// Runs a program's help output, parses it to discover command groups,
/// subcommands, flags, and arguments, then builds a [`CliSchema`].
pub struct CliParser {
    /// The program binary name (e.g., "incus")
    program: String,
    /// In-memory cache of help text keyed by command path
    cache: Arc<Mutex<HashMap<String, String>>>,
    /// The flag used to request help (usually "--help")
    help_flag: String,
}

impl CliParser {
    /// Create a new parser for the given program with the default `--help` flag.
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            help_flag: "--help".to_string(),
        }
    }

    /// Create a new parser with a custom help flag (e.g., "-h").
    pub fn with_help_flag(program: &str, help_flag: &str) -> Self {
        Self {
            program: program.to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            help_flag: help_flag.to_string(),
        }
    }

    /// Attempt to retrieve the program version.
    ///
    /// Tries `program --version` first; if that fails or returns empty output
    /// it falls back to `program version`. Returns the first non-empty line.
    pub async fn get_version(&self) -> Result<String> {
        // Try --version first (most common)
        let output = tokio::process::Command::new(&self.program)
            .arg("--version")
            .output()
            .await
            .context(format!("Failed to run {} --version", self.program))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(first_line) = stdout.lines().find(|l| !l.trim().is_empty()) {
            return Ok(first_line.trim().to_string());
        }

        // Fallback: some programs use a "version" subcommand (e.g., docker)
        let output = tokio::process::Command::new(&self.program)
            .arg("version")
            .output()
            .await
            .context(format!("Failed to run {} version", self.program))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown");
        Ok(first_line.trim().to_string())
    }

    /// Run the help command for a given command path and cache the result.
    ///
    /// Executes: `program [path...] <help_flag>`
    /// Both stdout and stderr are captured and concatenated.
    async fn run_help(&self, command_path: &[String]) -> Result<String> {
        let cache_key = if command_path.is_empty() {
            "_root".to_string()
        } else {
            command_path.join(".")
        };

        // Check cache first
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(cached.clone());
            }
        }

        let mut cmd = tokio::process::Command::new(&self.program);
        for part in command_path {
            cmd.arg(part);
        }
        cmd.arg(&self.help_flag);

        debug!(
            "Running: {} {} {}",
            self.program,
            command_path.join(" "),
            self.help_flag
        );

        let output = cmd.output().await.context(format!(
            "Failed to run {} {} {}",
            self.program,
            command_path.join(" "),
            self.help_flag
        ))?;

        // Combine stdout and stderr — many CLIs print help to stderr
        let help_text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);

        // Cache the result
        {
            let mut cache = self.cache.lock().await;
            cache.insert(cache_key, help_text.clone());
        }

        Ok(help_text)
    }

    /// Full introspection entry point.
    ///
    /// Recursively discovers the entire command hierarchy up to `max_depth`.
    pub async fn introspect_full(&self, max_depth: usize) -> Result<CliSchema> {
        let start = std::time::Instant::now();

        info!(
            "Starting CLI introspection for '{}' (max_depth={})",
            self.program, max_depth
        );

        let version = self
            .get_version()
            .await
            .unwrap_or_else(|_| "unknown".to_string());

        info!("{} version: {}", self.program, version);

        let mut stats = CliStats::default();
        let hierarchy = self
            .introspect_command_inner(&[], 0, max_depth, &mut stats)
            .await?;

        stats.introspection_time_ms = start.elapsed().as_millis();

        info!(
            "Introspection complete for '{}': {} groups, {} commands, {} flags in {}ms",
            self.program,
            stats.total_groups,
            stats.total_commands,
            stats.total_flags,
            stats.introspection_time_ms
        );

        Ok(CliSchema {
            program: self.program.clone(),
            version,
            hierarchy,
            statistics: stats,
        })
    }

    /// Recursively introspect a single command node.
    ///
    /// Uses `Box::pin` to allow async recursion through the subcommand tree.
    async fn introspect_command_inner(
        &self,
        command_path: &[String],
        depth: usize,
        max_depth: usize,
        stats: &mut CliStats,
    ) -> Result<CliCommand> {
        if depth > max_depth {
            return Ok(CliCommand::default());
        }

        let full_path = if command_path.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, command_path.join(" "))
        };

        let name = command_path
            .last()
            .map(|s| s.as_str())
            .unwrap_or(&self.program);

        debug!("Introspecting: {}", full_path);

        let help = self.run_help(command_path).await?;

        // Parse groups (gcloud-style GROUPS section)
        let groups = self.parse_groups(&help);
        // Parse commands from all recognized section formats
        let commands = self.parse_commands_section(&help);
        let flags = self.parse_flags_section(&help);
        let description = self.parse_description(&help);

        let is_group = !groups.is_empty() || !commands.is_empty();

        stats.total_flags += flags.len();
        if is_group {
            stats.total_groups += 1;
        } else {
            stats.total_commands += 1;
        }

        let mut cmd = CliCommand {
            name: name.to_string(),
            full_path,
            description,
            is_group,
            flags,
            positional_args: vec![],
            subcommands: HashMap::new(),
        };

        // Recurse into subcommands if we haven't hit the depth limit
        if depth < max_depth {
            // Groups are subcommands that themselves contain more commands
            for (group_name, _desc) in &groups {
                let mut sub_path = command_path.to_vec();
                sub_path.push(group_name.clone());

                match Box::pin(self.introspect_command_inner(
                    &sub_path,
                    depth + 1,
                    max_depth,
                    stats,
                ))
                .await
                {
                    Ok(sub_cmd) => {
                        cmd.subcommands.insert(group_name.clone(), sub_cmd);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to introspect group '{} {}': {}",
                            self.program,
                            sub_path.join(" "),
                            e
                        );
                    }
                }
            }

            // Leaf commands (may still have further subcommands)
            for (cmd_name, _desc) in &commands {
                // Avoid duplicates if a name appeared in both GROUPS and COMMANDS
                if cmd.subcommands.contains_key(cmd_name) {
                    continue;
                }

                let mut sub_path = command_path.to_vec();
                sub_path.push(cmd_name.clone());

                match Box::pin(self.introspect_command_inner(
                    &sub_path,
                    depth + 1,
                    max_depth,
                    stats,
                ))
                .await
                {
                    Ok(sub_cmd) => {
                        cmd.subcommands.insert(cmd_name.clone(), sub_cmd);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to introspect command '{} {}': {}",
                            self.program,
                            sub_path.join(" "),
                            e
                        );
                    }
                }
            }
        }

        Ok(cmd)
    }

    // -----------------------------------------------------------------------
    // Parsing helpers
    // -----------------------------------------------------------------------

    /// Parse commands from help output.
    ///
    /// Recognizes multiple section header styles:
    /// - Cobra-style: "Available Commands:", "Additional Commands:"
    /// - Click-style: "Commands:"
    /// - Gcloud-style: "COMMANDS"
    /// - Misc: "Subcommands:", "SUBCOMMANDS"
    ///
    /// Returns `(name, description)` pairs.
    pub fn parse_commands_section(&self, help: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut in_section = false;

        let cmd_name_re = Regex::new(r"^\s{2,8}(\w[\w-]*)\s{2,}(.*)$").unwrap();
        // Fallback for names with no multi-space gap (single description word)
        let cmd_name_simple_re = Regex::new(r"^\s{2,8}(\w[\w-]*)$").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            // Detect section headers
            if Self::is_commands_header(trimmed) {
                in_section = true;
                continue;
            }

            if in_section {
                // A non-indented, non-empty line signals end of section
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    in_section = false;
                    continue;
                }

                // Skip blank lines within the section
                if trimmed.is_empty() {
                    continue;
                }

                // Try to extract command name + description
                if let Some(caps) = cmd_name_re.captures(line) {
                    let name = caps.get(1).unwrap().as_str().to_string();
                    let desc = caps.get(2).unwrap().as_str().trim().to_string();
                    results.push((name, desc));
                } else if let Some(caps) = cmd_name_simple_re.captures(line) {
                    let name = caps.get(1).unwrap().as_str().to_string();
                    results.push((name, String::new()));
                }
            }
        }

        results
    }

    /// Parse flags from help output.
    ///
    /// Recognizes section headers:
    /// - Cobra-style: "Flags:", "Global Flags:", "Persistent Flags:"
    /// - Gcloud-style: "FLAGS", "OPTIONAL FLAGS", "REQUIRED FLAGS", "GLOBAL FLAGS",
    ///   "GCLOUD WIDE FLAGS"
    /// - Click/argparse-style: "Options:", "Optional arguments:"
    ///
    /// Handles flag formats:
    /// - `--flag-name string   Description text` (cobra)
    /// - `-f, --flag-name      Description text` (cobra short+long)
    /// - `--flag-name=VALUE    Description text` (gcloud)
    /// - `--flag-name          Description text` (boolean flags)
    pub fn parse_flags_section(&self, help: &str) -> Vec<CliFlag> {
        let mut flags = Vec::new();
        let mut in_flags_section = false;
        let mut is_required_section = false;
        let mut current_flag: Option<CliFlag> = None;

        // Pattern: -f, --flag-name value   Description
        // Pattern:     --flag-name value    Description
        // Pattern:     --flag-name=VALUE    Description
        let long_flag_re =
            Regex::new(r"^\s+(?:(-\w),\s+)?(--[\w-]+)(?:[=\s]\s*(\w+))?\s{2,}(.*)$").unwrap();
        // Simpler: just the flag with no description on this line
        let long_flag_simple_re =
            Regex::new(r"^\s+(?:(-\w),\s+)?(--[\w-]+)(?:[=\s]\s*(\w+))?\s*$").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            // Detect flags section headers
            if Self::is_flags_header(trimmed) {
                // Flush any pending flag
                if let Some(flag) = current_flag.take() {
                    flags.push(flag);
                }
                in_flags_section = true;
                is_required_section = trimmed.contains("REQUIRED");
                continue;
            }

            if in_flags_section {
                // Non-indented, non-empty line ends the section
                if !line.starts_with(' ') && !trimmed.is_empty() && !trimmed.starts_with('-') {
                    in_flags_section = false;
                    if let Some(flag) = current_flag.take() {
                        flags.push(flag);
                    }
                    continue;
                }

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Try to match a flag definition line
                let matched = if let Some(caps) = long_flag_re.captures(line) {
                    Some((
                        caps.get(1).map(|m| m.as_str().to_string()),
                        caps.get(2).unwrap().as_str().to_string(),
                        caps.get(3).map(|m| m.as_str()),
                        caps.get(4).map(|m| m.as_str().trim().to_string()),
                    ))
                } else if let Some(caps) = long_flag_simple_re.captures(line) {
                    Some((
                        caps.get(1).map(|m| m.as_str().to_string()),
                        caps.get(2).unwrap().as_str().to_string(),
                        caps.get(3).map(|m| m.as_str()),
                        None,
                    ))
                } else {
                    None
                };

                if let Some((short, long, value_hint, desc)) = matched {
                    // Flush previous flag
                    if let Some(flag) = current_flag.take() {
                        flags.push(flag);
                    }

                    // Extract default value from description if present
                    let (description, default_val) = if let Some(ref d) = desc {
                        Self::extract_default(d)
                    } else {
                        (String::new(), None)
                    };

                    current_flag = Some(CliFlag {
                        name: long,
                        short_name: short,
                        description,
                        required: is_required_section,
                        value_type: self.infer_type(value_hint),
                        default: default_val,
                        choices: vec![],
                    });
                } else if let Some(ref mut flag) = current_flag {
                    // Continuation line for the current flag's description
                    if !flag.description.is_empty() {
                        flag.description.push(' ');
                    }
                    flag.description.push_str(trimmed);
                }
            }
        }

        // Flush last flag
        if let Some(flag) = current_flag {
            flags.push(flag);
        }

        flags
    }

    /// Parse the description from help output.
    ///
    /// Looks for "Description:" (title-case) or "DESCRIPTION" (gcloud-style)
    /// sections and returns up to the first 3 lines joined with spaces.
    /// Falls back to the first non-header, non-empty line if no section found.
    pub fn parse_description(&self, help: &str) -> String {
        let mut in_description = false;
        let mut desc_lines: Vec<String> = Vec::new();

        for line in help.lines() {
            let trimmed = line.trim();

            if trimmed == "DESCRIPTION"
                || trimmed == "Description:"
                || trimmed.starts_with("DESCRIPTION")
            {
                in_description = true;
                continue;
            }

            if in_description {
                // Non-indented, non-empty → new section
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    break;
                }
                if !trimmed.is_empty() {
                    desc_lines.push(trimmed.to_string());
                }
            }
        }

        desc_lines
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Parse GROUPS section (gcloud-style).
    ///
    /// Returns `(name, description)` pairs for each group.
    pub fn parse_groups(&self, help: &str) -> Vec<(String, String)> {
        let mut groups = Vec::new();
        let mut in_groups_section = false;

        let name_re = Regex::new(r"^\s{2,8}(\w[\w-]*)\s{2,}(.*)$").unwrap();
        let name_simple_re = Regex::new(r"^\s{2,8}(\w[\w-]*)$").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            if trimmed == "GROUPS" || trimmed.starts_with("GROUPS") {
                in_groups_section = true;
                continue;
            }

            if in_groups_section {
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    break;
                }
                if trimmed.is_empty() {
                    continue;
                }

                if let Some(caps) = name_re.captures(line) {
                    let name = caps.get(1).unwrap().as_str().to_string();
                    let desc = caps.get(2).unwrap().as_str().trim().to_string();
                    groups.push((name, desc));
                } else if let Some(caps) = name_simple_re.captures(line) {
                    let name = caps.get(1).unwrap().as_str().to_string();
                    groups.push((name, String::new()));
                }
            }
        }

        groups
    }

    /// Infer the value type from an optional hint string.
    ///
    /// - "int", "integer", "number" -> "integer"
    /// - "bool", "boolean" -> "boolean"
    /// - None (no value hint) -> "boolean" (bare flags are typically toggles)
    /// - Anything else -> "string"
    pub fn infer_type(&self, hint: Option<&str>) -> String {
        match hint.map(|s| s.to_lowercase()).as_deref() {
            Some("int") | Some("integer") | Some("number") => "integer".to_string(),
            Some("bool") | Some("boolean") => "boolean".to_string(),
            Some("list") | Some("array") => "array".to_string(),
            Some(_) => "string".to_string(),
            None => "boolean".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Check if a trimmed line is a commands-section header.
    fn is_commands_header(trimmed: &str) -> bool {
        trimmed == "COMMANDS"
            || trimmed.starts_with("COMMANDS")
            || trimmed == "Available Commands:"
            || trimmed == "Additional Commands:"
            || trimmed == "Commands:"
            || trimmed == "Subcommands:"
            || trimmed == "SUBCOMMANDS"
            || trimmed == "Management Commands:"
    }

    /// Check if a trimmed line is a flags-section header.
    fn is_flags_header(trimmed: &str) -> bool {
        trimmed == "Flags:"
            || trimmed == "FLAGS"
            || trimmed == "Options:"
            || trimmed == "Global Flags:"
            || trimmed == "Persistent Flags:"
            || trimmed.starts_with("OPTIONAL FLAGS")
            || trimmed.starts_with("REQUIRED FLAGS")
            || trimmed.starts_with("GLOBAL FLAGS")
            || trimmed.starts_with("GCLOUD WIDE FLAGS")
            || trimmed == "Optional arguments:"
    }

    /// Extract a default value from a description string.
    ///
    /// Looks for patterns like `(default "table")` or `(default: 10)`.
    fn extract_default(desc: &str) -> (String, Option<String>) {
        let default_re =
            Regex::new(r#"\(default[:\s]+["']?([^"')]+)["']?\)"#).unwrap();
        if let Some(caps) = default_re.captures(desc) {
            let default_val = caps.get(1).unwrap().as_str().trim().to_string();
            let cleaned = default_re.replace(desc, "").trim().to_string();
            (cleaned, Some(default_val))
        } else {
            (desc.to_string(), None)
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

/// Convenience function to introspect any CLI program.
///
/// Creates a [`CliParser`] with default settings and runs a full introspection
/// up to the given depth.
pub async fn introspect_cli(program: &str, max_depth: usize) -> Result<CliSchema> {
    let parser = CliParser::new(program);
    parser.introspect_full(max_depth).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_parser_creation() {
        let parser = CliParser::new("incus");
        assert!(parser.cache.lock().await.is_empty());
        assert_eq!(parser.program, "incus");
        assert_eq!(parser.help_flag, "--help");
    }

    #[test]
    fn test_parse_cobra_commands() {
        let parser = CliParser::new("incus");
        let help = r#"
Usage:
  incus [command]

Available Commands:
  admin       Manage incus daemon
  config      Manage instance and server configuration options
  list        List instances

Flags:
  -h, --help   Print help
"#;

        let commands = parser.parse_commands_section(help);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].0, "admin");
        assert_eq!(commands[0].1, "Manage incus daemon");
        assert_eq!(commands[1].0, "config");
        assert_eq!(
            commands[1].1,
            "Manage instance and server configuration options"
        );
        assert_eq!(commands[2].0, "list");
        assert_eq!(commands[2].1, "List instances");
    }

    #[test]
    fn test_parse_cobra_flags() {
        let parser = CliParser::new("incus");
        let help = r#"
Usage:
  incus [command]

Flags:
      --debug          Show all debug messages
  -h, --help           Print help
  -q, --quiet          Don't show progress information
      --format string  Output format (default "table")
"#;

        let flags = parser.parse_flags_section(help);
        assert_eq!(flags.len(), 4);

        // --debug: boolean (no value hint)
        let debug_flag = flags.iter().find(|f| f.name == "--debug").unwrap();
        assert_eq!(debug_flag.value_type, "boolean");
        assert!(debug_flag.short_name.is_none());
        assert!(debug_flag.description.contains("debug"));

        // -h, --help
        let help_flag = flags.iter().find(|f| f.name == "--help").unwrap();
        assert_eq!(help_flag.short_name.as_deref(), Some("-h"));
        assert_eq!(help_flag.value_type, "boolean");

        // -q, --quiet
        let quiet_flag = flags.iter().find(|f| f.name == "--quiet").unwrap();
        assert_eq!(quiet_flag.short_name.as_deref(), Some("-q"));

        // --format string (default "table")
        let format_flag = flags.iter().find(|f| f.name == "--format").unwrap();
        assert_eq!(format_flag.value_type, "string");
        assert_eq!(format_flag.default.as_deref(), Some("table"));
    }

    #[test]
    fn test_parse_gcloud_style() {
        let parser = CliParser::new("gcloud");
        let help = r#"
NAME
    gcloud - manage Google Cloud resources

GROUPS
    compute       Create and manage Compute Engine resources
    storage       Create and manage Cloud Storage resources
    container     Deploy and manage containers

COMMANDS
    init          Initialize gcloud
    version       Print version information
    help          Display detailed help

FLAGS
    --project=PROJECT
        The Google Cloud project ID.

    --quiet
        Disable interactive prompts.

DESCRIPTION
    The Google Cloud CLI manages authentication, local configuration,
    developer workflow, and interactions with the Cloud Platform APIs.
"#;

        // Groups
        let groups = parser.parse_groups(help);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, "compute");
        assert!(groups[0].1.contains("Compute Engine"));
        assert_eq!(groups[1].0, "storage");
        assert_eq!(groups[2].0, "container");

        // Commands
        let commands = parser.parse_commands_section(help);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].0, "init");
        assert_eq!(commands[1].0, "version");
        assert_eq!(commands[2].0, "help");

        // Flags
        let flags = parser.parse_flags_section(help);
        assert!(!flags.is_empty());
        assert!(flags.iter().any(|f| f.name == "--project"));
        assert!(flags.iter().any(|f| f.name == "--quiet"));

        // Description
        let desc = parser.parse_description(help);
        assert!(desc.contains("Google Cloud CLI"));
    }

    #[test]
    fn test_parse_description_title_case() {
        let parser = CliParser::new("mytool");
        let help = r#"
Description:
    This is a great tool that does many things.
    It supports multiple formats.
    Very useful indeed.
    This fourth line should be ignored.

Usage:
    mytool [flags]
"#;

        let desc = parser.parse_description(help);
        assert!(desc.contains("great tool"));
        assert!(desc.contains("multiple formats"));
        assert!(desc.contains("Very useful"));
        // Only first 3 lines
        assert!(!desc.contains("fourth line"));
    }

    #[test]
    fn test_infer_type() {
        let parser = CliParser::new("test");
        assert_eq!(parser.infer_type(Some("int")), "integer");
        assert_eq!(parser.infer_type(Some("integer")), "integer");
        assert_eq!(parser.infer_type(Some("number")), "integer");
        assert_eq!(parser.infer_type(Some("bool")), "boolean");
        assert_eq!(parser.infer_type(Some("boolean")), "boolean");
        assert_eq!(parser.infer_type(Some("list")), "array");
        assert_eq!(parser.infer_type(Some("string")), "string");
        assert_eq!(parser.infer_type(Some("PATH")), "string");
        assert_eq!(parser.infer_type(None), "boolean");
    }

    #[test]
    fn test_extract_default() {
        let (desc, default) = CliParser::extract_default(r#"Output format (default "table")"#);
        assert_eq!(default.as_deref(), Some("table"));
        assert!(desc.contains("Output format"));
        assert!(!desc.contains("default"));

        let (desc, default) = CliParser::extract_default("Simple description");
        assert_eq!(desc, "Simple description");
        assert!(default.is_none());
    }

    #[test]
    fn test_with_help_flag() {
        let parser = CliParser::with_help_flag("mytool", "-h");
        assert_eq!(parser.help_flag, "-h");
        assert_eq!(parser.program, "mytool");
    }

    #[test]
    fn test_parse_required_flags_section() {
        let parser = CliParser::new("gcloud");
        let help = r#"
REQUIRED FLAGS
    --zone=ZONE
        The zone of the resource.

OPTIONAL FLAGS
    --quiet
        Disable prompts.
"#;

        let flags = parser.parse_flags_section(help);
        let zone = flags.iter().find(|f| f.name == "--zone").unwrap();
        assert!(zone.required);

        let quiet = flags.iter().find(|f| f.name == "--quiet").unwrap();
        assert!(!quiet.required);
    }

    #[test]
    fn test_default_cli_command() {
        let cmd = CliCommand::default();
        assert_eq!(cmd.name, "root");
        assert!(cmd.is_group);
        assert!(cmd.flags.is_empty());
        assert!(cmd.positional_args.is_empty());
        assert!(cmd.subcommands.is_empty());
    }

    #[test]
    fn test_default_cli_stats() {
        let stats = CliStats::default();
        assert_eq!(stats.total_groups, 0);
        assert_eq!(stats.total_commands, 0);
        assert_eq!(stats.total_flags, 0);
        assert_eq!(stats.introspection_time_ms, 0);
    }

    #[test]
    fn test_parse_empty_help() {
        let parser = CliParser::new("empty");
        let help = "";
        assert!(parser.parse_commands_section(help).is_empty());
        assert!(parser.parse_flags_section(help).is_empty());
        assert!(parser.parse_groups(help).is_empty());
        assert!(parser.parse_description(help).is_empty());
    }

    #[test]
    fn test_parse_docker_style_management_commands() {
        let parser = CliParser::new("docker");
        let help = r#"
Management Commands:
  container   Manage containers
  image       Manage images
  network     Manage networks

Commands:
  run         Create and run a new container
  ps          List containers
"#;

        // Management Commands are treated as commands too
        let mgmt = parser.parse_commands_section(help);
        // Should capture both Management Commands and Commands sections
        assert!(mgmt.len() >= 5);
        assert!(mgmt.iter().any(|c| c.0 == "container"));
        assert!(mgmt.iter().any(|c| c.0 == "run"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/src/datadump.rs">
//! Universal Data Dump - Import data from introspected schemas into database
//!
//! This module executes commands discovered during introspection and imports
//! their output into the op-dbus database.
//!
//! # Workflow
//! 1. Read schema from discover phase
//! 2. Identify data-producing commands (list, describe, get, etc.)
//! 3. Execute commands with JSON output format
//! 4. Import results into database

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::gcloud::{GCloudCommand, GCloudSchema};

/// Result of a data dump operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDumpResult {
    /// Source (e.g., "gcloud")
    pub source: String,
    /// Commands that were executed
    pub commands_executed: Vec<String>,
    /// Total objects imported
    pub total_objects: usize,
    /// Objects by type (e.g., "compute.instances" -> 5)
    pub objects_by_type: HashMap<String, usize>,
    /// Errors encountered
    pub errors: Vec<DataDumpError>,
    /// Time taken in milliseconds
    pub duration_ms: u128,
}

/// Error during data dump
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDumpError {
    pub command: String,
    pub error: String,
}

/// Imported object from external system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedObject {
    /// Object type (e.g., "gcloud.compute.instances")
    pub object_type: String,
    /// Object ID (extracted from object data)
    pub object_id: String,
    /// Full command path that produced this object
    pub source_command: String,
    /// Raw JSON data from the command
    pub data: Value,
    /// Timestamp of import
    pub imported_at: String,
}

/// Data dump executor
pub struct DataDumper {
    /// Dry run mode - don't actually execute commands
    dry_run: bool,
    /// Filter: only dump these resource types (empty = all)
    resource_filter: Vec<String>,
}

impl DataDumper {
    pub fn new() -> Self {
        Self {
            dry_run: false,
            resource_filter: Vec::new(),
        }
    }

    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }

    pub fn filter_resources(mut self, resources: Vec<String>) -> Self {
        self.resource_filter = resources;
        self
    }

    /// Find all data-producing commands in a schema
    fn find_data_commands(&self, cmd: &GCloudCommand, prefix: &str) -> Vec<DataCommand> {
        let mut results = Vec::new();
        let current_path = if prefix.is_empty() {
            cmd.name.clone()
        } else {
            format!("{}.{}", prefix, cmd.name)
        };

        // Check if this command produces data
        if is_data_producing_command(&cmd.name) {
            // Check resource filter
            if self.resource_filter.is_empty()
                || self.resource_filter.iter().any(|f| current_path.contains(f))
            {
                results.push(DataCommand {
                    path: current_path.clone(),
                    full_command: cmd.full_path.clone(),
                    command_type: classify_command(&cmd.name),
                });
            }
        }

        // Recurse into subcommands
        for sub_cmd in cmd.subcommands.values() {
            results.extend(self.find_data_commands(sub_cmd, &current_path));
        }

        results
    }

    /// Execute a data-producing command and return its output
    async fn execute_command(&self, cmd: &DataCommand) -> Result<Vec<ImportedObject>> {
        if self.dry_run {
            info!("[DRY RUN] Would execute: {} --format=json", cmd.full_command);
            return Ok(Vec::new());
        }

        debug!("Executing: {} --format=json", cmd.full_command);

        // Parse the command into parts
        let parts: Vec<&str> = cmd.full_command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(Vec::new());
        }

        let mut command = Command::new(parts[0]);
        for part in &parts[1..] {
            command.arg(part);
        }
        command.arg("--format=json");
        command.env("CLOUDSDK_CORE_DISABLE_PROMPTS", "1");
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let output = command.output().await.context("Failed to execute command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Some commands fail legitimately (no resources, permission denied)
            if stderr.contains("Listed 0 items")
                || stderr.contains("PERMISSION_DENIED")
                || stderr.contains("API has not been used")
            {
                debug!("Command {} returned no data or access denied", cmd.full_command);
                return Ok(Vec::new());
            }
            warn!("Command failed: {} - {}", cmd.full_command, stderr);
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() || stdout.trim() == "[]" {
            debug!("Command {} returned empty result", cmd.full_command);
            return Ok(Vec::new());
        }

        // Parse JSON output
        let json: Value = simd_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse JSON from {}", cmd.full_command))?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut objects = Vec::new();

        match json {
            Value::Array(items) => {
                for item in items {
                    let object_id = extract_object_id(&item);
                    objects.push(ImportedObject {
                        object_type: cmd.path.clone(),
                        object_id,
                        source_command: cmd.full_command.clone(),
                        data: item,
                        imported_at: now.clone(),
                    });
                }
            }
            Value::Object(_) => {
                let object_id = extract_object_id(&json);
                objects.push(ImportedObject {
                    object_type: cmd.path.clone(),
                    object_id,
                    source_command: cmd.full_command.clone(),
                    data: json,
                    imported_at: now,
                });
            }
            _ => {
                debug!("Unexpected JSON type from {}", cmd.full_command);
            }
        }

        info!(
            "Imported {} objects from {}",
            objects.len(),
            cmd.full_command
        );
        Ok(objects)
    }

    /// Dump data from a gcloud schema
    pub async fn dump_gcloud(&self, schema: &GCloudSchema) -> Result<(DataDumpResult, Vec<ImportedObject>)> {
        let start = std::time::Instant::now();

        info!("Starting data dump from gcloud schema");

        // Find all data-producing commands
        let data_commands = self.find_data_commands(&schema.hierarchy, "");
        info!("Found {} data-producing commands", data_commands.len());

        let mut result = DataDumpResult {
            source: "gcloud".to_string(),
            commands_executed: Vec::new(),
            total_objects: 0,
            objects_by_type: HashMap::new(),
            errors: Vec::new(),
            duration_ms: 0,
        };

        let mut all_objects = Vec::new();

        for cmd in &data_commands {
            result.commands_executed.push(cmd.full_command.clone());

            match self.execute_command(cmd).await {
                Ok(objects) => {
                    let count = objects.len();
                    if count > 0 {
                        *result.objects_by_type.entry(cmd.path.clone()).or_insert(0) += count;
                        result.total_objects += count;
                        all_objects.extend(objects);
                    }
                }
                Err(e) => {
                    result.errors.push(DataDumpError {
                        command: cmd.full_command.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis();

        info!(
            "Data dump complete: {} objects from {} commands in {}ms",
            result.total_objects,
            result.commands_executed.len(),
            result.duration_ms
        );

        Ok((result, all_objects))
    }
}

impl Default for DataDumper {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal representation of a data-producing command
#[derive(Debug, Clone)]
struct DataCommand {
    /// Dot-separated path (e.g., "compute.instances.list")
    path: String,
    /// Full command (e.g., "gcloud compute instances list")
    full_command: String,
    /// Type of command
    command_type: CommandType,
}

#[derive(Debug, Clone, Copy)]
enum CommandType {
    List,
    Describe,
    Get,
    Other,
}

/// Check if a command name produces data
fn is_data_producing_command(name: &str) -> bool {
    matches!(
        name,
        "list" | "describe" | "get" | "get-value" | "show" | "info"
    )
}

/// Classify a command by type
fn classify_command(name: &str) -> CommandType {
    match name {
        "list" => CommandType::List,
        "describe" => CommandType::Describe,
        "get" | "get-value" => CommandType::Get,
        _ => CommandType::Other,
    }
}

/// Extract an ID from an object
fn extract_object_id(obj: &Value) -> String {
    // Try common ID fields
    for field in &["id", "name", "selfLink", "resource_id", "uid", "ID", "Name"] {
        if let Some(id) = obj.get(field) {
            if let Some(s) = id.as_str() {
                return s.to_string();
            }
            if let Some(n) = id.as_u64() {
                return n.to_string();
            }
        }
    }

    // For selfLink, extract the last part
    if let Some(link) = obj.get("selfLink").and_then(|v| v.as_str()) {
        if let Some(last) = link.rsplit('/').next() {
            return last.to_string();
        }
    }

    // Fallback to hash of object
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    obj.to_string().hash(&mut hasher);
    format!("obj-{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_data_producing_command() {
        assert!(is_data_producing_command("list"));
        assert!(is_data_producing_command("describe"));
        assert!(is_data_producing_command("get"));
        assert!(!is_data_producing_command("create"));
        assert!(!is_data_producing_command("delete"));
    }

    #[test]
    fn test_extract_object_id() {
        let obj = simd_json::json!({"id": "12345", "name": "my-vm"});
        assert_eq!(extract_object_id(&obj), "12345");

        let obj = simd_json::json!({"name": "my-bucket"});
        assert_eq!(extract_object_id(&obj), "my-bucket");

        let obj = simd_json::json!({"selfLink": "https://compute.googleapis.com/compute/v1/projects/my-project/zones/us-central1-a/instances/my-vm"});
        assert_eq!(extract_object_id(&obj), "my-vm");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/src/gcloud.rs">
//! GCloud CLI Introspection Adapter
//!
//! Introspects the complete gcloud command hierarchy, discovering:
//! - Command groups and subcommands
//! - Flags and arguments for each command
//! - Command descriptions
//!
//! # Usage
//!
//! ```rust,no_run
//! use op_inspector::{IntrospectiveGadget, InspectionInput, InspectionSource};
//!
//! let gadget = IntrospectiveGadget::new();
//! gadget.register_parser("gcloud", Arc::new(GCloudParser::new()));
//!
//! let input = InspectionInput {
//!     source: InspectionSource::GCloud {
//!         command_path: vec![],  // Start from root
//!         max_depth: 3,
//!     },
//!     data: None,
//!     metadata: Default::default(),
//! };
//!
//! let result = gadget.inspect_object(input).await?;
//! ```

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// GCloud command hierarchy schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudSchema {
    /// Schema version
    pub schema_version: String,
    /// GCloud SDK version
    pub gcloud_version: String,
    /// Target account (if authenticated)
    pub account: Option<String>,
    /// Root command hierarchy
    pub hierarchy: GCloudCommand,
    /// Statistics
    pub statistics: GCloudStats,
}

/// Statistics about the introspection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GCloudStats {
    pub total_groups: usize,
    pub total_commands: usize,
    pub total_flags: usize,
    pub introspection_time_ms: u128,
}

/// Represents a gcloud command or command group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudCommand {
    /// Command name (e.g., "compute", "instances", "list")
    pub name: String,
    /// Full command path (e.g., "gcloud compute instances list")
    pub full_path: String,
    /// Command description
    pub description: String,
    /// Whether this is a command group (has subcommands)
    pub is_group: bool,
    /// Available flags
    pub flags: Vec<GCloudFlag>,
    /// Positional arguments
    pub positional_args: Vec<GCloudArg>,
    /// Subcommands (if this is a group)
    pub subcommands: HashMap<String, GCloudCommand>,
}

impl Default for GCloudCommand {
    fn default() -> Self {
        Self {
            name: "gcloud".to_string(),
            full_path: "gcloud".to_string(),
            description: String::new(),
            is_group: true,
            flags: vec![],
            positional_args: vec![],
            subcommands: HashMap::new(),
        }
    }
}

/// GCloud command flag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudFlag {
    pub name: String,
    pub short_name: Option<String>,
    pub description: String,
    pub required: bool,
    pub value_type: String,
    pub default: Option<String>,
    pub choices: Vec<String>,
}

/// GCloud positional argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudArg {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// GCloud CLI introspection parser
pub struct GCloudParser {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl Default for GCloudParser {
    fn default() -> Self {
        Self::new()
    }
}

impl GCloudParser {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the current gcloud version
    pub async fn get_version(&self) -> Result<String> {
        let output = tokio::process::Command::new("gcloud")
            .arg("--version")
            .output()
            .await
            .context("Failed to run gcloud --version")?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        Ok(version_str.lines().next().unwrap_or("unknown").to_string())
    }

    /// Get the current authenticated account
    pub async fn get_account(&self) -> Result<Option<String>> {
        let output = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "account"])
            .output()
            .await
            .context("Failed to get gcloud account")?;

        let account = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if account.is_empty() || account == "(unset)" {
            Ok(None)
        } else {
            Ok(Some(account))
        }
    }

    /// Run gcloud help for a command path
    async fn run_help(&self, command_path: &[String]) -> Result<String> {
        let cache_key = command_path.join(".");

        // Check cache
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(cached.clone());
            }
        }

        let mut cmd = tokio::process::Command::new("gcloud");
        for part in command_path {
            cmd.arg(part);
        }
        cmd.arg("--help");
        cmd.env("CLOUDSDK_CORE_DISABLE_PROMPTS", "1");

        let output = cmd.output().await.context("Failed to run gcloud help")?;

        let help_text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);

        // Cache the result
        {
            let mut cache = self.cache.lock().await;
            cache.insert(cache_key, help_text.clone());
        }

        Ok(help_text)
    }

    /// Parse command groups from help output
    fn parse_groups(&self, help: &str) -> Vec<String> {
        let mut groups = Vec::new();
        let mut in_groups_section = false;
        let group_regex = Regex::new(r"^\s{4,8}(\w[\w-]*)\s").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            if trimmed == "GROUPS" || trimmed.starts_with("GROUPS") {
                in_groups_section = true;
                continue;
            }

            if in_groups_section {
                // New section starts
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    break;
                }

                // Parse group name (indented, followed by description)
                if let Some(caps) = group_regex.captures(line) {
                    if let Some(name) = caps.get(1) {
                        groups.push(name.as_str().to_string());
                    }
                }
            }
        }

        groups
    }

    /// Parse commands from help output
    fn parse_commands(&self, help: &str) -> Vec<String> {
        let mut commands = Vec::new();
        let mut in_commands_section = false;
        let cmd_regex = Regex::new(r"^\s{4,8}(\w[\w-]*)\s").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            if trimmed == "COMMANDS" || trimmed.starts_with("COMMANDS") {
                in_commands_section = true;
                continue;
            }

            if in_commands_section {
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    break;
                }

                if let Some(caps) = cmd_regex.captures(line) {
                    if let Some(name) = caps.get(1) {
                        commands.push(name.as_str().to_string());
                    }
                }
            }
        }

        commands
    }

    /// Parse flags from help output
    fn parse_flags(&self, help: &str) -> Vec<GCloudFlag> {
        let mut flags = Vec::new();
        let mut in_flags_section = false;
        let mut current_flag: Option<GCloudFlag> = None;

        let flag_regex = Regex::new(r"^\s+(--[\w-]+)(?:=(\w+))?(?:,\s+(-\w))?").unwrap();

        for line in help.lines() {
            let trimmed = line.trim();

            // Detect flags sections
            if trimmed.contains("FLAGS")
                && (trimmed == "FLAGS"
                    || trimmed.starts_with("OPTIONAL FLAGS")
                    || trimmed.starts_with("REQUIRED FLAGS")
                    || trimmed.starts_with("GLOBAL FLAGS")
                    || trimmed.starts_with("GCLOUD WIDE FLAGS"))
            {
                in_flags_section = true;
                continue;
            }

            if in_flags_section {
                // New section
                if !line.starts_with(' ') && !trimmed.is_empty() && !trimmed.starts_with("--") {
                    in_flags_section = false;
                    if let Some(flag) = current_flag.take() {
                        flags.push(flag);
                    }
                    continue;
                }

                // Flag definition
                if let Some(caps) = flag_regex.captures(line) {
                    if let Some(flag) = current_flag.take() {
                        flags.push(flag);
                    }

                    let flag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let value_hint = caps.get(2).map(|m| m.as_str());
                    let short_name = caps.get(3).map(|m| m.as_str().to_string());

                    current_flag = Some(GCloudFlag {
                        name: flag_name.to_string(),
                        short_name,
                        description: String::new(),
                        required: false,
                        value_type: self.infer_type(value_hint),
                        default: None,
                        choices: vec![],
                    });
                } else if let Some(ref mut flag) = current_flag {
                    // Description continuation
                    if !trimmed.is_empty() {
                        if !flag.description.is_empty() {
                            flag.description.push(' ');
                        }
                        flag.description.push_str(trimmed);
                    }
                }
            }
        }

        if let Some(flag) = current_flag {
            flags.push(flag);
        }

        flags
    }

    /// Parse description from help output
    fn parse_description(&self, help: &str) -> String {
        let mut in_description = false;
        let mut description_lines = Vec::new();

        for line in help.lines() {
            let trimmed = line.trim();

            if trimmed == "DESCRIPTION" {
                in_description = true;
                continue;
            }

            if in_description {
                if !line.starts_with(' ') && !trimmed.is_empty() {
                    break;
                }
                if !trimmed.is_empty() {
                    description_lines.push(trimmed.to_string());
                }
            }
        }

        description_lines
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Infer type from hint
    fn infer_type(&self, hint: Option<&str>) -> String {
        match hint.map(|s| s.to_lowercase()).as_deref() {
            Some("int") | Some("integer") | Some("number") => "integer".to_string(),
            Some("bool") | Some("boolean") => "boolean".to_string(),
            Some("list") | Some("array") => "array".to_string(),
            Some(_) => "string".to_string(),
            None => "boolean".to_string(),
        }
    }

    /// Recursively introspect a command (non-recursive entry point)
    async fn introspect_command(
        &self,
        command_path: &[String],
        depth: usize,
        max_depth: usize,
    ) -> Result<(GCloudCommand, GCloudStats)> {
        let mut stats = GCloudStats::default();
        let cmd = self
            .introspect_command_inner(command_path, depth, max_depth, &mut stats)
            .await?;
        Ok((cmd, stats))
    }

    /// Recursively introspect a command (uses iteration to avoid async recursion)
    async fn introspect_command_inner(
        &self,
        command_path: &[String],
        depth: usize,
        max_depth: usize,
        stats: &mut GCloudStats,
    ) -> Result<GCloudCommand> {
        if depth > max_depth {
            return Ok(GCloudCommand::default());
        }

        let full_path = if command_path.is_empty() {
            "gcloud".to_string()
        } else {
            format!("gcloud {}", command_path.join(" "))
        };

        let name = command_path.last().map(|s| s.as_str()).unwrap_or("gcloud");

        debug!("Introspecting: {}", full_path);

        let help = self.run_help(command_path).await?;

        let groups = self.parse_groups(&help);
        let commands = self.parse_commands(&help);
        let flags = self.parse_flags(&help);
        let description = self.parse_description(&help);

        let is_group = !groups.is_empty() || !commands.is_empty();

        stats.total_flags += flags.len();
        if is_group {
            stats.total_groups += 1;
        } else {
            stats.total_commands += 1;
        }

        let mut cmd = GCloudCommand {
            name: name.to_string(),
            full_path,
            description,
            is_group,
            flags,
            positional_args: vec![],
            subcommands: HashMap::new(),
        };

        // Introspect subcommands (one level at a time to avoid deep recursion)
        if depth < max_depth {
            for group in groups {
                let mut sub_path = command_path.to_vec();
                sub_path.push(group.clone());

                match Box::pin(self.introspect_command_inner(
                    &sub_path,
                    depth + 1,
                    max_depth,
                    stats,
                ))
                .await
                {
                    Ok(sub_cmd) => {
                        cmd.subcommands.insert(group, sub_cmd);
                    }
                    Err(e) => {
                        warn!("Failed to introspect {}: {}", sub_path.join(" "), e);
                    }
                }
            }

            for command in commands {
                let mut sub_path = command_path.to_vec();
                sub_path.push(command.clone());

                match Box::pin(self.introspect_command_inner(
                    &sub_path,
                    depth + 1,
                    max_depth,
                    stats,
                ))
                .await
                {
                    Ok(sub_cmd) => {
                        cmd.subcommands.insert(command, sub_cmd);
                    }
                    Err(e) => {
                        warn!("Failed to introspect {}: {}", sub_path.join(" "), e);
                    }
                }
            }
        }

        Ok(cmd)
    }

    /// Full introspection of gcloud CLI
    pub async fn introspect_full(&self, max_depth: usize) -> Result<GCloudSchema> {
        let start = std::time::Instant::now();

        info!(
            "Starting gcloud CLI introspection (max_depth={})",
            max_depth
        );

        let version = self
            .get_version()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        let account = self.get_account().await.unwrap_or(None);

        info!("GCloud version: {}", version);
        if let Some(ref acc) = account {
            info!("Authenticated account: {}", acc);
        }

        let (hierarchy, mut stats) = self.introspect_command(&[], 0, max_depth).await?;

        stats.introspection_time_ms = start.elapsed().as_millis();

        info!(
            "Introspection complete: {} groups, {} commands, {} flags in {}ms",
            stats.total_groups,
            stats.total_commands,
            stats.total_flags,
            stats.introspection_time_ms
        );

        Ok(GCloudSchema {
            schema_version: "1.0.0".to_string(),
            gcloud_version: version,
            account,
            hierarchy,
            statistics: stats,
        })
    }
}

/// Convenience function to introspect gcloud and return schema
pub async fn introspect_gcloud(max_depth: usize) -> Result<GCloudSchema> {
    let parser = GCloudParser::new();
    parser.introspect_full(max_depth).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gcloud_parser_creation() {
        let parser = GCloudParser::new();
        assert!(parser.cache.lock().await.is_empty());
    }

    #[test]
    fn test_parse_groups() {
        let parser = GCloudParser::new();
        let help = r#"
NAME
    gcloud - manage Google Cloud resources

GROUPS
    compute       Create and manage Compute Engine resources
    storage       Create and manage Cloud Storage resources
    container     Deploy and manage containers

COMMANDS
    init          Initialize gcloud
"#;

        let groups = parser.parse_groups(help);
        assert_eq!(groups, vec!["compute", "storage", "container"]);
    }

    #[test]
    fn test_parse_commands() {
        let parser = GCloudParser::new();
        let help = r#"
COMMANDS
    init          Initialize gcloud
    version       Print version information
    help          Display help
"#;

        let commands = parser.parse_commands(help);
        assert_eq!(commands, vec!["init", "version", "help"]);
    }

    #[test]
    fn test_parse_flags() {
        let parser = GCloudParser::new();
        let help = r#"
FLAGS
    --project=PROJECT
        The Google Cloud project ID.

    --zone=ZONE, -z
        The zone of the resource.

    --quiet
        Disable interactive prompts.
"#;

        let flags = parser.parse_flags(help);
        assert!(!flags.is_empty());
        assert!(flags.iter().any(|f| f.name == "--project"));
        assert!(flags.iter().any(|f| f.name == "--zone"));
        assert!(flags.iter().any(|f| f.name == "--quiet"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/src/introspective_gadget.rs">
//! Introspective Gadget - Universal Object Inspector
//!
//! Like Inspector Gadget, but for data structures! 🕵️‍♂️
//!
//! This is the universal object inspector that can analyze ANY data structure
//! and add it to the knowledge base for schema generation and understanding.
//!
//! Examples:
//! - Docker containers: Inspect running containers, extract configurations
//! - XML data: Parse unknown XML structures, generate schemas
//! - JSON objects: Analyze complex nested structures
//! - Binary data: Attempt to reverse engineer structures
//! - Legacy formats: Handle old/obscure data formats (like Apple Lisa disks)
//!
//! The gadget can:
//! 1. Accept any input source (file, URL, data stream, etc.)
//! 2. Attempt multiple parsing strategies
//! 3. Extract structural information
//! 4. Generate validation schemas
//! 5. Add to knowledge base for future use
//! 6. Create templates for similar objects

use anyhow::{Context, Result};
use base64::engine::general_purpose;
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::Path;

// Stub types for KnowledgeBase and SchemaDefinition until op-mcp is built
#[derive(Debug, Clone, Default)]
pub struct KnowledgeBase {
    pub schemas: HashMap<String, SchemaDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub name: String,
    pub object_type: String,
    pub source_type: String,
    pub source_data: Option<String>,
    pub schema: Value,
    pub generated_schemas: HashMap<String, String>,
    pub validation_rules: Vec<String>,
    pub examples: Vec<Value>,
    pub metadata: HashMap<String, String>,
}

// ============================================================================
// INTROSPECTIVE GADGET - THE OBJECT INSPECTOR
// ============================================================================

/// The main Introspective Gadget - universal object inspector
#[derive(Clone)]
pub struct IntrospectiveGadget {
    knowledge_base: std::sync::Arc<tokio::sync::RwLock<KnowledgeBase>>,
    parsers: std::sync::Arc<
        std::sync::RwLock<HashMap<String, std::sync::Arc<dyn ObjectParser + Send + Sync>>>,
    >,
}

impl IntrospectiveGadget {
    /// Create a new Introspective Gadget
    pub async fn new(
        knowledge_base: std::sync::Arc<tokio::sync::RwLock<KnowledgeBase>>,
    ) -> Result<Self> {
        let mut parsers: HashMap<String, std::sync::Arc<dyn ObjectParser + Send + Sync>> =
            HashMap::new();

        // Register all built-in parsers
        parsers.insert("json".to_string(), std::sync::Arc::new(JsonParser));
        parsers.insert("xml".to_string(), std::sync::Arc::new(XmlParser));
        parsers.insert("yaml".to_string(), std::sync::Arc::new(YamlParser));
        parsers.insert("docker".to_string(), std::sync::Arc::new(DockerParser));
        parsers.insert("binary".to_string(), std::sync::Arc::new(BinaryParser));
        parsers.insert("text".to_string(), std::sync::Arc::new(TextParser));
        parsers.insert("auto".to_string(), std::sync::Arc::new(AutoParser));

        Ok(Self {
            knowledge_base,
            parsers: std::sync::Arc::new(std::sync::RwLock::new(parsers)),
        })
    }

    /// Inspect any object and add to knowledge base
    ///
    /// This is the main "Go-Go-Gadget" method that can handle anything!
    pub async fn inspect_object(&self, input: InspectionInput) -> Result<InspectionResult> {
        let start_time = std::time::Instant::now();

        // Attempt to determine the format
        let detected_format = self.detect_format(&input).await?;

        // Try multiple parsing strategies
        let mut results = Vec::new();
        let mut errors = Vec::new();

        // Try the detected format first
        let parser_opt = self.parsers.read().unwrap().get(&detected_format).cloned();
        if let Some(parser) = parser_opt {
            match parser.parse(&input).await {
                Ok(result) => results.push(result),
                Err(e) => errors.push(format!("{} parser failed: {}", detected_format, e)),
            }
        }

        // If that didn't work, try auto-detection
        if results.is_empty() {
            let auto_parser_opt = self.parsers.read().unwrap().get("auto").cloned();
            if let Some(auto_parser) = auto_parser_opt {
                match auto_parser.parse(&input).await {
                    Ok(result) => results.push(result),
                    Err(e) => errors.push(format!("Auto parser failed: {}", e)),
                }
            }
        }

        // Try all parsers if still no results
        if results.is_empty() {
            let all_parsers: Vec<(String, std::sync::Arc<dyn ObjectParser + Send + Sync>)> = self
                .parsers
                .read()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            for (format_name, parser) in all_parsers {
                if format_name != detected_format && format_name != "auto" {
                    if let Ok(result) = parser.parse(&input).await {
                        results.push(result);
                    }
                }
            }
        }

        if results.is_empty() {
            return Err(anyhow::anyhow!(
                "Could not parse object with any available parser. Errors: {:?}",
                errors
            ));
        }

        // Use the best result (most complete schema)
        let best_result = results
            .into_iter()
            .max_by_key(|r| r.schema.complexity_score())
            .unwrap();

        // Generate knowledge base entry
        let kb_entry = self
            .generate_knowledge_base_entry(&best_result, &input)
            .await?;

        // Add to knowledge base
        {
            let mut kb = self.knowledge_base.write().await;
            kb.schemas.insert(kb_entry.name.clone(), kb_entry.clone());
        }

        let inspection_time = start_time.elapsed().as_millis();

        Ok(InspectionResult {
            input_info: input,
            detected_format,
            parsed_data: best_result.data,
            schema: best_result.schema,
            knowledge_base_entry: kb_entry.name,
            inspection_time_ms: inspection_time,
            parsing_errors: errors,
        })
    }

    /// Inspect a Docker container (specialized method)
    pub async fn inspect_docker_container(
        &self,
        container_name: &str,
    ) -> Result<ContainerInspectionWithKnowledge> {
        // Get container info
        let inspect_output = tokio::process::Command::new("docker")
            .args(["inspect", container_name])
            .output()
            .await
            .context("Failed to run docker inspect")?;

        let mut inspect_json = String::from_utf8_lossy(&inspect_output.stdout).to_string();

        // Parse the JSON
        let container_data: Value = unsafe { simd_json::from_str(&mut inspect_json) }
            .context("Failed to parse docker inspect JSON")?;

        // Extract key information
        let config = container_data[0]["Config"].clone();
        let network_settings = container_data[0]["NetworkSettings"].clone();
        let mounts = container_data[0]["Mounts"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|m| ContainerMount {
                source: m["Source"].as_str().unwrap_or("").to_string(),
                destination: m["Destination"].as_str().unwrap_or("").to_string(),
                mode: m["Mode"].as_str().unwrap_or("").to_string(),
                rw: m["RW"].as_bool().unwrap_or(false),
            })
            .collect();

        // Get running processes
        let top_output = tokio::process::Command::new("docker")
            .args(["top", container_name])
            .output()
            .await;

        let processes = if let Ok(output) = top_output {
            let top_text = String::from_utf8_lossy(&output.stdout);
            self.parse_docker_top(&top_text)
        } else {
            vec![]
        };

        let inspection = ContainerInspection {
            name: container_name.to_string(),
            id: container_data[0]["Id"].as_str().unwrap_or("").to_string(),
            image: container_data[0]["Config"]["Image"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            status: container_data[0]["State"]["Status"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            config: config.clone(),
            network_settings: network_settings.clone(),
            mounts,
            processes,
            ports: self.extract_container_ports(&network_settings),
            environment: self.extract_container_env(&config),
            labels: self.extract_container_labels(&config),
        };

        // Add to knowledge base
        let input = InspectionInput {
            source: InspectionSource::DockerContainer(container_name.to_string()),
            data: Some(inspect_json.to_string()),
            metadata: HashMap::new(),
        };

        let result = self.inspect_object(input).await?;
        let kb_entry_name = result.knowledge_base_entry;

        Ok(ContainerInspectionWithKnowledge {
            inspection,
            knowledge_base_entry: kb_entry_name,
        })
    }

    /// Inspect random XML data (as mentioned)
    pub async fn inspect_xml_data(
        &self,
        xml_data: &str,
        source_description: &str,
    ) -> Result<XmlInspection> {
        let input = InspectionInput {
            source: InspectionSource::RawData {
                format_hint: Some("xml".to_string()),
                description: source_description.to_string(),
            },
            data: Some(xml_data.to_string()),
            metadata: HashMap::new(),
        };

        let result = self.inspect_object(input).await?;

        // Try to understand the XML structure
        let root_element = self.extract_xml_root(xml_data);
        let namespaces = self.extract_xml_namespaces(xml_data);
        let elements = self.analyze_xml_elements(xml_data);

        Ok(XmlInspection {
            source_description: source_description.to_string(),
            root_element,
            namespaces,
            elements,
            schema_generated: result.schema,
            knowledge_base_entry: result.knowledge_base_entry,
        })
    }

    /// Inspect legacy/binary data (like Apple Lisa disks)
    pub async fn inspect_legacy_data(
        &self,
        data: &[u8],
        description: &str,
    ) -> Result<LegacyInspection> {
        let input = InspectionInput {
            source: InspectionSource::RawData {
                format_hint: Some("binary".to_string()),
                description: description.to_string(),
            },
            data: Some(String::from_utf8_lossy(data).to_string()),
            metadata: HashMap::from([
                ("original_size".to_string(), data.len().to_string()),
                (
                    "entropy".to_string(),
                    self.calculate_entropy(data).to_string(),
                ),
            ]),
        };

        let result = self.inspect_object(input).await?;

        // Analyze binary structure
        let file_header = if data.len() >= 16 {
            Some(data[0..16].to_vec())
        } else {
            None
        };

        let strings_found = self.extract_strings_from_binary(data);
        let patterns = self.analyze_binary_patterns(data);

        Ok(LegacyInspection {
            description: description.to_string(),
            file_size: data.len(),
            file_header,
            strings_found,
            patterns,
            entropy: self.calculate_entropy(data),
            schema_generated: result.schema,
            knowledge_base_entry: result.knowledge_base_entry,
        })
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    async fn detect_format(&self, input: &InspectionInput) -> Result<String> {
        match &input.source {
            InspectionSource::File(path) => {
                if let Some(ext) = Path::new(path).extension() {
                    match ext.to_str().unwrap_or("") {
                        "json" => Ok("json".to_string()),
                        "xml" => Ok("xml".to_string()),
                        "yaml" | "yml" => Ok("yaml".to_string()),
                        _ => Ok("auto".to_string()),
                    }
                } else {
                    Ok("auto".to_string())
                }
            }
            InspectionSource::DockerContainer(_) => Ok("docker".to_string()),
            InspectionSource::RawData { format_hint, .. } => {
                Ok(format_hint.clone().unwrap_or_else(|| "auto".to_string()))
            }
            _ => Ok("auto".to_string()),
        }
    }

    async fn generate_knowledge_base_entry(
        &self,
        result: &ParsedObject,
        input: &InspectionInput,
    ) -> Result<SchemaDefinition> {
        let name = match &input.source {
            InspectionSource::File(path) => format!(
                "file_{}",
                Path::new(path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
            InspectionSource::DockerContainer(name) => format!("docker_container_{}", name),
            InspectionSource::RawData { description, .. } => {
                format!("raw_data_{}", description.replace(" ", "_"))
            }
            InspectionSource::Url(url) => {
                format!("url_{}", url.replace("/", "_").replace(":", "_"))
            }
        };

        let source_type = match &input.source {
            InspectionSource::File(_) => "file".to_string(),
            InspectionSource::DockerContainer(_) => "docker".to_string(),
            InspectionSource::RawData { .. } => "raw_data".to_string(),
            InspectionSource::Url(_) => "url".to_string(),
        };

        Ok(SchemaDefinition {
            name: name.clone(),
            object_type: result.schema.schema_type.clone(),
            source_type,
            source_data: Some(simd_json::to_string(&result.data)?),
            schema: result.schema.to_value(),
            generated_schemas: HashMap::new(),
            validation_rules: result.schema.generate_validation_rules(),
            examples: vec![result.data.clone()],
            metadata: HashMap::new(),
        })
    }

    fn extract_xml_root(&self, xml: &str) -> Option<String> {
        let re = Regex::new(r#"<\s*([^\s>]+)"#).ok()?;
        re.captures(xml)?.get(1).map(|m| m.as_str().to_string())
    }

    fn extract_xml_namespaces(&self, xml: &str) -> HashMap<String, String> {
        let mut namespaces = HashMap::new();
        let re = Regex::new(r#"xmlns(?::([^\s=]+))?\s*=\s*["']([^"']+)["']"#).unwrap();

        for cap in re.captures_iter(xml) {
            let prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("default");
            let uri = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            namespaces.insert(prefix.to_string(), uri.to_string());
        }

        namespaces
    }

    fn analyze_xml_elements(&self, xml: &str) -> Vec<XmlElementInfo> {
        let mut elements = Vec::new();
        let re = Regex::new(r#"<([^\s>/]+)([^>]*)>"#).unwrap();

        for cap in re.captures_iter(xml) {
            let name = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let attrs = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            let attributes = self.parse_xml_attributes(attrs);
            elements.push(XmlElementInfo { name, attributes });
        }

        elements
    }

    fn parse_xml_attributes(&self, attrs: &str) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        let re = Regex::new(r#"(\w+)\s*=\s*["']([^"']*)["']"#).unwrap();

        for cap in re.captures_iter(attrs) {
            if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                attributes.insert(key.as_str().to_string(), value.as_str().to_string());
            }
        }

        attributes
    }

    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        let mut counts = [0u64; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    fn extract_strings_from_binary(&self, data: &[u8]) -> Vec<String> {
        let mut strings = Vec::new();
        let mut current_string = Vec::new();

        for &byte in data {
            if byte.is_ascii_alphanumeric() || byte.is_ascii_punctuation() || byte == b' ' {
                current_string.push(byte);
            } else {
                if current_string.len() >= 4 {
                    if let Ok(s) = String::from_utf8(current_string.clone()) {
                        strings.push(s);
                    }
                }
                current_string.clear();
            }
        }

        strings
    }

    fn analyze_binary_patterns(&self, data: &[u8]) -> Vec<BinaryPattern> {
        let mut patterns = Vec::new();

        // Look for repeating patterns
        if data.len() >= 8 {
            for i in 0..data.len().saturating_sub(8) {
                let pattern = &data[i..i + 8];
                let mut count = 0;
                let mut pos = 0;

                while let Some(found) = data[pos..].windows(8).position(|w| w == pattern) {
                    count += 1;
                    pos += found + 8;
                    if pos >= data.len() - 8 {
                        break;
                    }
                }

                if count > 1 {
                    patterns.push(BinaryPattern {
                        pattern: pattern.to_vec(),
                        count,
                        offset: i,
                    });
                }
            }
        }

        patterns.sort_by(|a, b| b.count.cmp(&a.count));
        patterns.truncate(10); // Top 10 patterns

        patterns
    }

    fn parse_docker_top(&self, top_output: &str) -> Vec<ContainerProcess> {
        let mut processes = Vec::new();
        let lines: Vec<&str> = top_output.lines().collect();

        if lines.len() < 2 {
            return processes;
        }

        for line in &lines[1..] {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 8 {
                processes.push(ContainerProcess {
                    user: parts[0].to_string(),
                    pid: parts[1].parse().unwrap_or(0),
                    ppid: parts[2].parse().unwrap_or(0),
                    cpu: parts[3].to_string(),
                    memory: parts[4].to_string(),
                    vsz: parts[5].parse().unwrap_or(0),
                    rss: parts[6].parse().unwrap_or(0),
                    tty: parts[7].to_string(),
                    stat: parts.get(8).map_or("", |v| v).to_string(),
                    start: parts.get(9).map_or("", |v| v).to_string(),
                    time: parts.get(10).map_or("", |v| v).to_string(),
                    command: parts[11..].join(" "),
                });
            }
        }

        processes
    }

    fn extract_container_ports(&self, network_settings: &Value) -> HashMap<String, Vec<String>> {
        let mut ports = HashMap::new();

        if let Some(ports_obj) = network_settings["Ports"].as_object() {
            for (container_port, host_bindings) in ports_obj {
                if let Some(bindings) = host_bindings.as_array() {
                    let hosts = bindings
                        .iter()
                        .filter_map(|b| {
                            if let (Some(host_ip), Some(host_port)) =
                                (b["HostIp"].as_str(), b["HostPort"].as_str())
                            {
                                Some(format!("{}:{}", host_ip, host_port))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    if !hosts.is_empty() {
                        ports.insert(container_port.clone(), hosts);
                    }
                }
            }
        }

        ports
    }

    fn extract_container_env(&self, config: &Value) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(env_array) = config["Env"].as_array() {
            for env_var in env_array {
                if let Some(env_str) = env_var.as_str() {
                    if let Some(eq_pos) = env_str.find('=') {
                        let key = &env_str[..eq_pos];
                        let value = &env_str[eq_pos + 1..];
                        env.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        env
    }

    fn extract_container_labels(&self, config: &Value) -> HashMap<String, String> {
        let mut labels = HashMap::new();

        if let Some(labels_obj) = config["Labels"].as_object() {
            for (key, value) in labels_obj {
                if let Some(val_str) = value.as_str() {
                    labels.insert(key.clone(), val_str.to_string());
                }
            }
        }

        labels
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Input for inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionInput {
    pub source: InspectionSource,
    pub data: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Source of the data to inspect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectionSource {
    File(String),
    Url(String),
    DockerContainer(String),
    RawData {
        format_hint: Option<String>,
        description: String,
    },
}

/// Result of an inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    pub input_info: InspectionInput,
    pub detected_format: String,
    pub parsed_data: Value,
    pub schema: ObjectSchema,
    pub knowledge_base_entry: String,
    pub inspection_time_ms: u128,
    pub parsing_errors: Vec<String>,
}

/// Parsed object result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedObject {
    pub data: Value,
    pub schema: ObjectSchema,
}

/// Object schema extracted from inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchema {
    pub schema_type: String,
    pub properties: HashMap<String, SchemaProperty>,
    pub required: Vec<String>,
    pub array_items: Option<Box<ObjectSchema>>,
    pub object_patterns: Vec<String>,
}

impl ObjectSchema {
    fn complexity_score(&self) -> usize {
        self.properties.len() * 10 + self.required.len() * 5 + self.object_patterns.len() * 3
    }

    fn to_value(&self) -> Value {
        json!({
            "type": self.schema_type,
            "properties": self.properties.iter().map(|(k, v)| (k.clone(), v.to_value())).collect::<HashMap<_, _>>(),
            "required": self.required,
            "array_items": self.array_items.as_ref().map(|s| s.to_value()),
            "object_patterns": self.object_patterns
        })
    }

    fn generate_validation_rules(&self) -> Vec<String> {
        let mut rules = Vec::new();

        for (prop_name, prop) in &self.properties {
            match prop.data_type.as_str() {
                "string" => {
                    if prop.pattern.is_some() {
                        rules.push(format!("{}_format", prop_name));
                    }
                }
                "number" => {
                    if let Some(min) = prop.minimum {
                        rules.push(format!("{}_min_{}", prop_name, min));
                    }
                    if let Some(max) = prop.maximum {
                        rules.push(format!("{}_max_{}", prop_name, max));
                    }
                }
                _ => {}
            }
        }

        rules
    }
}

/// Schema property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProperty {
    pub data_type: String,
    pub description: Option<String>,
    pub pattern: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub enum_values: Option<Vec<Value>>,
    pub nested_schema: Option<Box<ObjectSchema>>,
}

impl SchemaProperty {
    fn to_value(&self) -> Value {
        let mut obj = json!({
            "type": self.data_type
        });

        if let Some(desc) = &self.description {
            obj["description"] = json!(desc);
        }
        if let Some(pattern) = &self.pattern {
            obj["pattern"] = json!(pattern);
        }
        if let Some(min) = self.minimum {
            obj["minimum"] = json!(min);
        }
        if let Some(max) = self.maximum {
            obj["maximum"] = json!(max);
        }
        if let Some(enum_vals) = &self.enum_values {
            obj["enum"] = json!(enum_vals);
        }
        if let Some(nested) = &self.nested_schema {
            obj["properties"] = nested.to_value();
        }

        obj
    }
}

// ============================================================================
// SPECIALIZED INSPECTION RESULTS
// ============================================================================

/// Docker container inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInspectionWithKnowledge {
    pub inspection: ContainerInspection,
    pub knowledge_base_entry: String,
}

/// Docker container inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInspection {
    pub name: String,
    pub id: String,
    pub image: String,
    pub status: String,
    pub config: Value,
    pub network_settings: Value,
    pub mounts: Vec<ContainerMount>,
    pub processes: Vec<ContainerProcess>,
    pub ports: HashMap<String, Vec<String>>,
    pub environment: HashMap<String, String>,
    pub labels: HashMap<String, String>,
}

/// Container mount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerMount {
    pub source: String,
    pub destination: String,
    pub mode: String,
    pub rw: bool,
}

/// Container process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProcess {
    pub user: String,
    pub pid: u32,
    pub ppid: u32,
    pub cpu: String,
    pub memory: String,
    pub vsz: u64,
    pub rss: u64,
    pub tty: String,
    pub stat: String,
    pub start: String,
    pub time: String,
    pub command: String,
}

/// XML inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlInspection {
    pub source_description: String,
    pub root_element: Option<String>,
    pub namespaces: HashMap<String, String>,
    pub elements: Vec<XmlElementInfo>,
    pub schema_generated: ObjectSchema,
    pub knowledge_base_entry: String,
}

/// XML element information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlElementInfo {
    pub name: String,
    pub attributes: HashMap<String, String>,
}

/// Legacy/binary inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyInspection {
    pub description: String,
    pub file_size: usize,
    pub file_header: Option<Vec<u8>>,
    pub strings_found: Vec<String>,
    pub patterns: Vec<BinaryPattern>,
    pub entropy: f64,
    pub schema_generated: ObjectSchema,
    pub knowledge_base_entry: String,
}

/// Binary pattern found in data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryPattern {
    pub pattern: Vec<u8>,
    pub count: usize,
    pub offset: usize,
}

// ============================================================================
// PARSERS
// ============================================================================

/// Trait for object parsers
#[async_trait::async_trait]
trait ObjectParser: Send + Sync {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject>;
}

/// JSON parser
struct JsonParser;

#[async_trait::async_trait]
impl ObjectParser for JsonParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for JSON parsing"))?;

        let mut data_mut = data.clone();
        let parsed: Value = unsafe { simd_json::from_str(&mut data_mut)? };
        let schema = self.analyze_json_schema(&parsed);

        Ok(ParsedObject {
            data: parsed,
            schema,
        })
    }
}

impl JsonParser {
    fn analyze_json_schema(&self, value: &Value) -> ObjectSchema {
        if let Some(obj) = value.as_object() {
            let mut properties = HashMap::new();
            let mut required = Vec::new();

            for (key, val) in obj {
                properties.insert(key.clone(), self.analyze_json_value(val));
                required.push(key.clone());
            }

            ObjectSchema {
                schema_type: "object".to_string(),
                properties,
                required,
                array_items: None,
                object_patterns: vec![],
            }
        } else if let Some(arr) = value.as_array() {
            let item_schema = arr
                .first()
                .map(|first| Box::new(self.analyze_json_schema(first)));

            ObjectSchema {
                schema_type: "array".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: item_schema,
                object_patterns: vec![],
            }
        } else {
            ObjectSchema {
                schema_type: self.json_value_type(value),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec![],
            }
        }
    }

    fn analyze_json_value(&self, value: &Value) -> SchemaProperty {
        SchemaProperty {
            data_type: self.json_value_type(value),
            description: None,
            pattern: None,
            minimum: None,
            maximum: None,
            enum_values: None,
            nested_schema: None,
        }
    }

    fn json_value_type(&self, value: &Value) -> String {
        if value.is_str() {
            "string".to_string()
        } else if value.is_i64() || value.is_u64() || value.is_f64() {
            "number".to_string()
        } else if value.is_bool() {
            "boolean".to_string()
        } else if value.is_object() {
            "object".to_string()
        } else if value.is_array() {
            "array".to_string()
        } else {
            "null".to_string()
        }
    }
}

/// XML parser
struct XmlParser;

#[async_trait::async_trait]
impl ObjectParser for XmlParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for XML parsing"))?;

        // Simple XML parsing - extract structure
        let properties = HashMap::from([(
            "xml_content".to_string(),
            SchemaProperty {
                data_type: "string".to_string(),
                description: Some("Raw XML content".to_string()),
                pattern: Some(r#"^<.*>$"#.to_string()),
                minimum: None,
                maximum: None,
                enum_values: None,
                nested_schema: None,
            },
        )]);

        Ok(ParsedObject {
            data: json!({ "xml": data }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties,
                required: vec!["xml_content".to_string()],
                array_items: None,
                object_patterns: vec!["xml_structure".to_string()],
            },
        })
    }
}

/// Docker parser
struct DockerParser;

#[async_trait::async_trait]
impl ObjectParser for DockerParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        if let InspectionSource::DockerContainer(name) = &input.source {
            // Run docker inspect
            let output = tokio::process::Command::new("docker")
                .args(["inspect", name])
                .output()
                .await?;

            let mut json_str = String::from_utf8_lossy(&output.stdout).to_string();
            let parsed: Value = unsafe { simd_json::from_str(&mut json_str)? };

            Ok(ParsedObject {
                data: parsed,
                schema: ObjectSchema {
                    schema_type: "object".to_string(),
                    properties: HashMap::new(), // Would analyze Docker schema
                    required: vec![],
                    array_items: None,
                    object_patterns: vec!["docker_container".to_string()],
                },
            })
        } else {
            Err(anyhow::anyhow!(
                "Docker parser requires DockerContainer source"
            ))
        }
    }
}

/// Binary parser for unknown data
struct BinaryParser;

#[async_trait::async_trait]
impl ObjectParser for BinaryParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for binary parsing"))?;

        let bytes = data.as_bytes();

        Ok(ParsedObject {
            data: json!({
                "binary_data": general_purpose::STANDARD.encode(bytes),
                "size": bytes.len(),
                "entropy": calculate_entropy(bytes),
            }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec!["binary_blob".to_string()],
            },
        })
    }
}

/// YAML parser
struct YamlParser;

#[async_trait::async_trait]
impl ObjectParser for YamlParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for YAML parsing"))?;

        let parsed: Value = serde_yaml::from_str(data)?;
        let schema = JsonParser.analyze_json_schema(&parsed); // Reuse JSON analyzer

        Ok(ParsedObject {
            data: parsed,
            schema,
        })
    }
}

/// Text parser for plain text
struct TextParser;

#[async_trait::async_trait]
impl ObjectParser for TextParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for text parsing"))?;

        Ok(ParsedObject {
            data: json!({ "text": data }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec!["plain_text".to_string()],
            },
        })
    }
}

/// Auto-detecting parser
struct AutoParser;

#[async_trait::async_trait]
impl ObjectParser for AutoParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let _data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for auto parsing"))?;

        // Try JSON first
        if let Ok(result) = JsonParser.parse(input).await {
            return Ok(result);
        }

        // Try XML
        if let Ok(result) = XmlParser.parse(input).await {
            return Ok(result);
        }

        // Try YAML
        if let Ok(result) = YamlParser.parse(input).await {
            return Ok(result);
        }

        // Fall back to binary
        BinaryParser.parse(input).await
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

fn calculate_entropy(data: &[u8]) -> f64 {
    let mut counts = [0u64; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/src/lib.rs">
//! op-inspector: Inspector Gadget - Universal Object Inspector
//!
//! Features:
//! - Inspect ANY data structure (JSON, XML, binary, Docker, DBus, Proxmox)
//! - AI-powered gap filling for incomplete introspections
//! - Schema generation and validation
//! - Knowledge base integration
//! - Proxmox LXC template introspection (4500+ editable elements)
//! - GCloud CLI introspection (100+ command groups, all flags/args)

pub mod gcloud;
mod introspective_gadget;

// Re-export main types
pub use gcloud::{
    introspect_gcloud, GCloudArg, GCloudCommand, GCloudFlag, GCloudParser, GCloudSchema,
    GCloudStats,
};
pub use introspective_gadget::*;

use op_introspection::IntrospectionService;
use std::sync::Arc;

/// Simplified Inspector Gadget wrapper
pub struct InspectorGadget {
    introspection: Arc<IntrospectionService>,
}

impl InspectorGadget {
    pub fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }

    pub fn introspection(&self) -> Arc<IntrospectionService> {
        Arc::clone(&self.introspection)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/ADAPTER-WORKFLOW.md">
# Adapter Workflow: From Documentation to Integrated Tool

This document describes the workflow for creating an introspection adapter for any external system (gcloud, Active Directory, Docker, etc.) and integrating it into the op-dbus tool system.

## Overview

The workflow that was used for the gcloud adapter:

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. FEED DOCUMENTATION                                           │
│    Provide Claude with the external system's documentation,     │
│    reference material, or access to introspect the system       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. DISCOVER HIGH-LEVEL SURFACES                                 │
│    Enumerate all introspectable objects/entry points:           │
│    - gcloud: top-level command groups (compute, storage, ...)   │
│    - D-Bus: list all services on the bus                        │
│    - LDAP: query rootDSE, list naming contexts                  │
│    - Docker: list containers, images, networks, volumes         │
│    This gives the "table of contents" for full introspection    │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. DESIGN SCHEMA STRUCTURES                                     │
│    Create Rust structs that represent the system's surface:     │
│    - Hierarchy/tree structure                                   │
│    - Commands/methods/operations                                │
│    - Parameters/flags/arguments                                 │
│    - Properties/attributes                                      │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. IMPLEMENT PARSER                                             │
│    Create a parser that can introspect the external system:     │
│    - Implement ObjectParser trait                               │
│    - Parse help output, API responses, or documentation         │
│    - Build the schema structures                                │
│    - Cache results for efficiency                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. CREATE TOOLS                                                 │
│    Wrap the adapter in tools for the ToolRegistry:              │
│    - List/search operations                                     │
│    - Introspect specific items                                  │
│    - Execute commands (if applicable)                           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 6. REGISTER TOOLS                                               │
│    Wire tools into register_all_builtin_tools()                 │
└─────────────────────────────────────────────────────────────────┘
```

## Step 1: Feed Documentation

Provide Claude with comprehensive documentation about the external system:

- **CLI tools**: Help output, man pages, reference documentation
- **APIs**: OpenAPI specs, SDK documentation, protocol specs
- **Services**: D-Bus introspection XML, LDAP schemas, etc.

For gcloud, Claude was given access to run `gcloud --help` recursively to discover the entire command hierarchy.

## Step 2: Discover High-Level Surfaces

Before deep introspection, enumerate all introspectable entry points. This is the "table of contents" that tells you what exists to introspect.

### Discovery Methods by System

| System | Discovery Command | What It Returns |
|--------|-------------------|-----------------|
| gcloud | `gcloud --help` | Top-level groups: compute, storage, container, iam, ... |
| D-Bus | `busctl list` | All services: org.freedesktop.UDisks2, org.freedesktop.login1, ... |
| LDAP | Query rootDSE | Naming contexts, supported controls, schema location |
| Docker | `docker info`, `docker ps -a` | Containers, images, networks, volumes |
| Kubernetes | `kubectl api-resources` | All resource types: pods, services, deployments, ... |
| Active Directory | LDAP rootDSE + `CN=Schema,CN=Configuration` | Domain info, all object classes, attributes |

### gcloud Discovery Example

```bash
$ gcloud --help
# GROUPS section lists all top-level surfaces:
#   access-approval, access-context-manager, active-directory,
#   ai, ai-platform, alloydb, anthos, api-gateway, apigee,
#   app, artifacts, asset, assured, auth, batch, bigtable,
#   billing, bms, builds, certificate-manager, cloud-shell,
#   composer, compute, config, container, data-catalog,
#   database-migration, dataflow, dataplex, dataproc,
#   datastore, datastream, deploy, deployment-manager,
#   dns, domains, edge-cache, edge-cloud, emulators,
#   endpoints, essential-contacts, eventarc, filestore,
#   firebase, firestore, functions, healthcare, iam,
#   identity, ids, immersive-stream, infra-manager,
#   kms, logging, looker, memcache, metastore, ml,
#   ml-engine, monitoring, netapp, network-connectivity,
#   network-management, network-security, network-services,
#   notebooks, org-policies, organizations, pam, policy-intelligence,
#   policy-troubleshoot, privateca, projects, publicca,
#   pubsub, recaptcha, recommender, redis, resource-manager,
#   resource-settings, run, scc, scheduler, secrets,
#   service-directory, services, source, spanner, sql,
#   storage, tasks, telco-automation, topic, transcoder,
#   transfer, vmware, workbench, workflows, workspace-add-ons, ...
```

This discovery step identifies ~100+ top-level groups, each of which will be recursively introspected in step 4.

### D-Bus Discovery Example

```bash
$ busctl --system list
# Returns all services on the system bus:
org.freedesktop.Accounts
org.freedesktop.DBus
org.freedesktop.UDisks2
org.freedesktop.login1
org.freedesktop.NetworkManager
org.freedesktop.PolicyKit1
org.freedesktop.systemd1
...
```

Each service is then introspected to discover its object paths, interfaces, methods, properties, and signals.

### Why Discovery Matters

1. **Scoping**: Know the full surface area before diving deep
2. **Incremental introspection**: Can introspect one group at a time
3. **Caching strategy**: Cache at the right granularity
4. **Progress tracking**: "Introspected 45/100 command groups"
5. **Schema design**: Informs what structures are needed

## Step 3: Design Schema Structures

Create Rust structs that capture the system's surface. Key patterns:

### Root Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudSchema {
    pub schema_version: String,
    pub gcloud_version: String,
    pub account: Option<String>,
    pub hierarchy: GCloudCommand,      // The tree structure
    pub statistics: GCloudStats,       // Introspection metadata
}
```

### Hierarchical Items
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudCommand {
    pub name: String,
    pub full_path: String,
    pub description: String,
    pub is_group: bool,                          // Has children?
    pub flags: Vec<GCloudFlag>,                  // Parameters
    pub positional_args: Vec<GCloudArg>,         // Required args
    pub subcommands: HashMap<String, GCloudCommand>,  // Children
}
```

### Parameters/Flags
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCloudFlag {
    pub name: String,
    pub short_name: Option<String>,
    pub description: String,
    pub required: bool,
    pub value_type: String,
    pub default: Option<String>,
    pub choices: Vec<String>,
}
```

## Step 4: Implement Parser

Create a parser that implements `ObjectParser` trait:

```rust
pub struct GCloudParser {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

#[async_trait]
impl ObjectParser for GCloudParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        // 1. Extract parameters from input
        // 2. Run introspection (e.g., gcloud --help)
        // 3. Parse output into schema structures
        // 4. Return ParsedObject with data and schema
    }
}
```

### Introspection Strategy

For gcloud, the parser:
1. Runs `gcloud [command_path] --help`
2. Parses the help text with regex to extract:
   - GROUPS section → subcommand groups
   - COMMANDS section → leaf commands
   - FLAGS section → available flags
   - DESCRIPTION section → command description
3. Recursively introspects subcommands up to max_depth
4. Caches results to avoid redundant calls

```rust
async fn introspect_command(
    &self,
    command_path: &[String],
    depth: usize,
    max_depth: usize,
) -> Result<GCloudCommand> {
    let help = self.run_help(command_path).await?;

    let groups = self.parse_groups(&help);
    let commands = self.parse_commands(&help);
    let flags = self.parse_flags(&help);
    let description = self.parse_description(&help);

    // Recursively introspect children
    for group in groups {
        let sub_path = [command_path, &[group]].concat();
        let sub_cmd = self.introspect_command(&sub_path, depth + 1, max_depth).await?;
        cmd.subcommands.insert(group, sub_cmd);
    }

    Ok(cmd)
}
```

## Step 5: Create Tools (Integration Gap)

This is where gcloud integration is incomplete. Need to create tools in `op-tools/src/builtin/gcloud_tools.rs`:

```rust
pub async fn register_gcloud_tools(registry: &ToolRegistry) -> Result<()> {
    let parser = Arc::new(GCloudParser::new());

    registry.register_tool(Arc::new(GCloudIntrospectTool::new(parser.clone()))).await?;
    registry.register_tool(Arc::new(GCloudSearchTool::new(parser.clone()))).await?;
    registry.register_tool(Arc::new(GCloudGetCommandTool::new(parser.clone()))).await?;

    Ok(())
}

struct GCloudIntrospectTool {
    parser: Arc<GCloudParser>,
}

#[async_trait]
impl Tool for GCloudIntrospectTool {
    fn name(&self) -> &str { "gcloud_introspect" }

    fn description(&self) -> &str {
        "Introspect gcloud CLI command hierarchy"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command_path": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command path to introspect (e.g., ['compute', 'instances'])"
                },
                "max_depth": {
                    "type": "integer",
                    "default": 3
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let schema = self.parser.introspect_full(max_depth).await?;
        Ok(serde_json::to_value(schema)?)
    }
}
```

## Step 6: Register Tools

Add to `op-tools/src/builtin/mod.rs`:

```rust
pub mod gcloud_tools;

pub async fn register_all_builtin_tools(registry: &ToolRegistry) -> Result<()> {
    // ... existing registrations ...

    // Register gcloud tools
    gcloud_tools::register_gcloud_tools(registry).await?;

    Ok(())
}
```

## Applying to Other Systems

### Active Directory / LDAP

```rust
pub struct LdapSchema {
    pub schema_version: String,
    pub base_dn: String,
    pub object_classes: HashMap<String, LdapObjectClass>,
    pub attribute_types: HashMap<String, LdapAttribute>,
}

pub struct LdapParser {
    // Connect to LDAP, query schema
    // Parse objectClass and attributeType definitions
}
```

### Docker

```rust
pub struct DockerSchema {
    pub containers: Vec<ContainerInfo>,
    pub images: Vec<ImageInfo>,
    pub networks: Vec<NetworkInfo>,
    pub volumes: Vec<VolumeInfo>,
}

pub struct DockerParser {
    // Run docker inspect, docker ps, etc.
    // Parse JSON output
}
```

### D-Bus (Already Done)

The D-Bus adapter in `op-introspection` follows this same pattern:
- `IntrospectionService` - the parser
- `ServiceScanner` - runs introspection
- `dbus_introspection.rs` - the tools (12 tools registered)

## File Locations

```
crates/
├── op-inspector/
│   └── src/
│       ├── lib.rs                    # Export adapters
│       ├── gcloud.rs                 # GCloud adapter (complete)
│       ├── ldap.rs                   # LDAP adapter (future)
│       └── introspective_gadget.rs   # Generic inspection framework
│
├── op-introspection/
│   └── src/
│       ├── lib.rs                    # D-Bus introspection service
│       └── scanner.rs                # D-Bus scanner
│
└── op-tools/
    └── src/
        └── builtin/
            ├── mod.rs                # Register all tools
            ├── dbus_introspection.rs # D-Bus tools (complete)
            └── gcloud_tools.rs       # GCloud tools (TODO)
```

## Summary

| Step | gcloud Status | D-Bus Status |
|------|---------------|--------------|
| 1. Documentation | Fed to Claude | Built-in introspection |
| 2. Discover surfaces | `gcloud --help` → 100+ groups | `busctl list` → all services |
| 3. Schema structs | `GCloudSchema`, `GCloudCommand`, etc. | `ObjectInfo`, `InterfaceInfo`, etc. |
| 4. Parser | `GCloudParser` | `IntrospectionService` |
| 5. Tools | **Missing** | 12 tools in `dbus_introspection.rs` |
| 6. Registration | **Missing** | In `register_all_builtin_tools()` |

The gcloud adapter is complete through step 4. Steps 5-6 need implementation to make it available to agents through the tool system.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/Cargo.toml">
[package]
name = "op-inspector"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Inspector Gadget - Universal object inspector with AI gap-filling and Proxmox introspection"

[dependencies]
op-core = { workspace = true }
op-introspection = { path = "../op-introspection" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
regex = { workspace = true }
quick-xml = { workspace = true }
sha2 = { workspace = true }
base64 = { workspace = true }
serde_yaml = { workspace = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/compare-op-inspector.md">
# compare-op-inspector

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md, ADAPTER-WORKFLOW.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 5 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 2 |
| Partial artifacts | 0 |
| Spec-listed source files | 4 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- Inspector Gadget - Universal object inspector with AI gap-filling and Proxmox introspection
- Internal crate integrations: op-core, op-introspection.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/introspective_gadget.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/introspective_gadget.rs |
| `src/gcloud.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gcloud.rs |
| `src/datadump.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/datadump.rs |
| `root` | ✅ Present | root source group | src/cli.rs, src/datadump.rs, src/gcloud.rs, src/introspective_gadget.rs, src/lib.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| introspective_gadget | ✅ Implemented | src/introspective_gadget.rs | SPEC main module |
| gcloud | ✅ Implemented | src/gcloud.rs | SPEC main module |
| datadump | ✅ Implemented | src/datadump.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-introspection` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `uuid` - documented in SPEC
- `chrono` - documented in SPEC
- `regex` - documented in SPEC
- `quick-xml` - documented in SPEC
- `sha2` - documented in SPEC
- `base64` - documented in SPEC
- `serde_yaml` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: ADAPTER-WORKFLOW.md, SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: gcloud, introspective_gadget.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-inspector/SPEC.md">
# op-inspector - Specification

## Overview
**Crate**: `op-inspector`  
**Location**: `crates/op-inspector`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-inspector"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-inspector/src/lib.rs
op-inspector/src/introspective_gadget.rs
op-inspector/src/gcloud.rs
op-inspector/src/datadump.rs
```

### Key Dependencies
```toml
op-core = { workspace = true }
op-introspection = { path = "../op-introspection" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
regex = { workspace = true }
quick-xml = { workspace = true }
sha2 = { workspace = true }
base64 = { workspace = true }
serde_yaml = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files
ADAPTER-WORKFLOW.md

## Module Structure
       4 Rust source files

### Main Modules
introspective_gadget
gcloud
datadump

## Purpose
Inspector Gadget - Universal object inspector with AI gap-filling and Proxmox introspection

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-introspection

---
*Generated from crate analysis*
</file>

</files>
