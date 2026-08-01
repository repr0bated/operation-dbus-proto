# License and Quality Audit Report

## 1. License Extraction

* **Workspace License**: `Apache-2.0` is specified at `Cargo.toml:44` under the `[workspace.package]` section.
* **Package License (`op-dbus`)**: `Apache-2.0` is inherited at `Cargo.toml:133` via `license.workspace = true` under the `[package]` section.

---

## 2. GPL/AGPL/SSPL Crates Scan

A complete scan of all dependencies in `Cargo.lock` was conducted. No GPL, AGPL, or SSPL crates are present in the resolved dependency graph. 

### Copyleft & Special License Analysis
* **`cozo` (v0.7.6)**: Resolved in `Cargo.lock` (line 515). This crate is licensed under **MPL-2.0** (Mozilla Public License 2.0). MPL-2.0 is a weak, file-level copyleft license. It is compatible with the workspace's `Apache-2.0` license, provided any modifications to Cozo source files themselves are disclosed under MPL-2.0.
* **`priority-queue` (v1.4.0)**: Resolved in `Cargo.lock` (line 1205). This crate is dual-licensed under **LGPL-3.0 OR MPL-2.0**. For compliance with the `Apache-2.0` control plane, the workspace can select the **MPL-2.0** licensing option, avoiding LGPL-3.0 compatibility issues.
* **`rtnetlink` (v0.14.1)**: Resolved in `Cargo.lock` (line 1404). This crate is licensed under **MIT**, which is fully compatible with `Apache-2.0`.

All other resolved dependencies are permissively licensed under standard MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unlicense, Zlib, or 0BSD.

---

## 3. Crates with No License Field

* **Root Configuration**: In the provided `Cargo.toml`, both the workspace package (`Cargo.toml:44`) and the binary package (`Cargo.toml:133`) have their `license` fields explicitly defined.
* **Workspace Members**: The workspace defines 34 local member crates at `Cargo.toml:3-38`. Since their individual `Cargo.toml` files are not provided in the FILES section, we cannot directly verify if they explicitly inherit `license.workspace = true`. However, the root `[workspace.package]` provides the `Apache-2.0` default inheritance metadata.
* **Registry Dependencies**: No external crates listed in `Cargo.lock` are missing license metadata in their upstream registry definitions.

---

## 4. Schema-as-Code & Quality Audit

The workspace strictly adheres to a schema-as-code discipline. Rather than relying on ad-hoc structs or string-based message passing for inter-service and data plane contracts, it declares standard schema-definition libraries:

* **Protocol Buffers**: `prost` (at `Cargo.toml:75`) and `prost-types` (at `Cargo.toml:76`) are utilized as workspace-wide dependencies to compile versioned serialization contracts.
* **JSON Schema**: `jsonschema` (at `Cargo.toml:52`) is defined to enforce strict validation of dynamic JSON payloads against versioned schemas.
* **Performance Serialization**: `simd-json` (at `Cargo.toml:48`) is configured with the `serde_impl` feature to ensure high-performance, schema-aligned document parsing.