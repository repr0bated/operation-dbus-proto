# Security and Quality Audit: op-inspector

## 1. Runtime Environment Variable Reads (`std::env::var`)
A comprehensive search of the provided source code for `op-inspector` shows **zero (0)** direct runtime calls to `std::env::var` or `std::env::var_os`.

However, the crate modifies the environment of child processes by injecting configuration variables:
* **`crates/op-inspector/src/datadump.rs:163`**: Injecting `CLOUDSDK_CORE_DISABLE_PROMPTS` set to `"1"` during data command execution.
* **`crates/op-inspector/src/gcloud.rs:179`**: Injecting `CLOUDSDK_CORE_DISABLE_PROMPTS` set to `"1"` during help execution.

---

## 2. Environment Variables Missing Defaults / Error Handling
Because there are **no runtime environment variable reads** executed within the scanned source files of `op-inspector`, no unhandled environment variables or missing defaults exist in the audited code.

---

## 3. Cargo Features & Additive Behavior

### `crates/op-inspector/Cargo.toml`
The local package manifest `crates/op-inspector/Cargo.toml` does not define any local `[features]`.

### Workspace `Cargo.toml`
The root workspace configuration defines features for the `op-dbus` package (which pulls in `op-inspector` as a workspace dependency):
* **`default`**: `["grpc"]`
* **`grpc`**: `[]`

### Additive Behavior Analysis
Yes, Cargo features in this workspace are **additive**. Enabling the `default` feature transitively includes the `grpc` feature. Since no features are modeled as mutually exclusive, they adhere to standard additive design patterns.

---

## 4. Hardcoded Paths, Binaries, Ports, and Addresses

Executing external commands using hardcoded system binary names instead of fully qualified paths exposes the application to path traversal or binary hijacking if the system's `PATH` environment variable is compromised.

### Hardcoded System Binaries
* **`crates/op-inspector/src/gcloud.rs:139`**: Calls `gcloud` directly via `Command::new("gcloud")`.
* **`crates/op-inspector/src/gcloud.rs:152`**: Calls `gcloud` directly via `Command::new("gcloud")`.
* **`crates/op-inspector/src/gcloud.rs:175`**: Calls `gcloud` directly via `Command::new("gcloud")`.
* **`crates/op-inspector/src/introspective_gadget.rs:188`**: Calls `docker` directly via `Command::new("docker")`.
* **`crates/op-inspector/src/introspective_gadget.rs:213`**: Calls `docker` directly via `Command::new("docker")`.
* **`crates/op-inspector/src/introspective_gadget.rs:707`**: Calls `docker` directly via `Command::new("docker")`.

### Hardcoded Ports, IP Addresses, and Network Sockets
No hardcoded local TCP ports or IP addresses (e.g., `127.0.0.1` or `0.0.0.0`) are defined in the production code. 
* A mock Google Cloud endpoint URL is hardcoded solely inside the unit test suite:
  * **`crates/op-inspector/src/datadump.rs:260`**: `"https://compute.googleapis.com/compute/v1/projects/my-project/zones/us-central1-a/instances/my-vm"`

---

## 5. Schema-as-Code Compliance Review (Ad-Hoc Structs & Contracts)

This codebase violates the **Schema-as-Code** discipline. Instead of defining versioned data contracts utilizing Protocol Buffers (`.proto`) or standardized OSCAL representation, it models critical system introspection records, command execution schemas, and security targets as ad-hoc, serializable Rust structs.

### Ad-Hoc CLI Introspection Schemas
The following structs express data contracts as ad-hoc structures rather than versioned schemas:
* **`crates/op-inspector/src/cli.rs:33-43`**: `CliSchema` struct
* **`crates/op-inspector/src/cli.rs:46-60`**: `CliCommand` struct
* **`crates/op-inspector/src/cli.rs:75-91`**: `CliFlag` struct
* **`crates/op-inspector/src/cli.rs:94-103`**: `CliArg` struct
* **`crates/op-inspector/src/cli.rs:106-113`**: `CliStats` struct

### Ad-Hoc Google Cloud Discovery Contracts
* **`crates/op-inspector/src/gcloud.rs:37-50`**: `GCloudSchema` struct
* **`crates/op-inspector/src/gcloud.rs:53-60`**: `GCloudStats` struct
* **`crates/op-inspector/src/gcloud.rs:63-79`**: `GCloudCommand` struct
* **`crates/op-inspector/src/gcloud.rs:92-102`**: `GCloudFlag` struct
* **`crates/op-inspector/src/gcloud.rs:105-110`**: `GCloudArg` struct

### Ad-Hoc Data Dump & Database Import Schemas
* **`crates/op-inspector/src/datadump.rs:28-44`**: `DataDumpResult` struct
* **`crates/op-inspector/src/datadump.rs:47-51`**: `DataDumpError` struct
* **`crates/op-inspector/src/datadump.rs:54-68`**: `ImportedObject` struct

### Ad-Hoc Universal Object Inspection & Knowledge Base Contracts
* **`crates/op-inspector/src/introspective_gadget.rs:40-52`**: `SchemaDefinition` struct
* **`crates/op-inspector/src/introspective_gadget.rs:514-519`**: `InspectionInput` struct
* **`crates/op-inspector/src/introspective_gadget.rs:531-541`**: `InspectionResult` struct
* **`crates/op-inspector/src/introspective_gadget.rs:544-548`**: `ParsedObject` struct
* **`crates/op-inspector/src/introspective_gadget.rs:551-558`**: `ObjectSchema` struct
* **`crates/op-inspector/src/introspective_gadget.rs:598-607`**: `SchemaProperty` struct
* **`crates/op-inspector/src/introspective_gadget.rs:648-652`**: `ContainerInspectionWithKnowledge` struct
* **`crates/op-inspector/src/introspective_gadget.rs:655-668`**: `ContainerInspection` struct
* **`crates/op-inspector/src/introspective_gadget.rs:671-677`**: `ContainerMount` struct
* **`crates/op-inspector/src/introspective_gadget.rs:680-694`**: `ContainerProcess` struct
* **`crates/op-inspector/src/introspective_gadget.rs:697-706`**: `XmlInspection` struct
* **`crates/op-inspector/src/introspective_gadget.rs:709-713`**: `XmlElementInfo` struct
* **`crates/op-inspector/src/introspective_gadget.rs:716-727`**: `LegacyInspection` struct
* **`crates/op-inspector/src/introspective_gadget.rs:730-735`**: `BinaryPattern` struct

---

## 6. Actionable Security Findings

### High: Argument Injection via Unvalidated System Commands
* **Location**: `crates/op-inspector/src/introspective_gadget.rs:188`, `213`, `707`
* **Impact**: Although Rust's `Command` API mitigates direct shell-command injection (e.g., separating inputs with semicolons), passing unvalidated string variables (`container_name` or `name`) directly as arguments allows an attacker to perform **argument injection**. By injecting inputs starting with hyphens (e.g., `--config`, `--mount`), malicious actors can force the host's `docker` binary to override configurations, mount arbitrary directories, or leak localized environment states.
* **Remediation**: Implement strict validation on all dynamic command-line parameters. Validate that `container_name` / `name` strings match a strict regex format (e.g., `^[a-zA-Z0-9][a-zA-Z0-9_.-]+$`) and specifically ensure they do not start with a hyphen (`-`).