# Project Guidelines for Junie

## Project Overview

`operation-dbus-proto` is a Rust workspace project (`op-core`, `op-plugins`, `op-state-store`, `op-workflows`, `op-web`, `op-mcp`) paired with a React/TypeScript frontend. It focuses on high-performance D-Bus native architecture for platform integrations and gRPC for internal service-to-service RPC, specifically tailored for deployment on Chimera OS.

## Strict Development Constraints & Nuances

- **No OVS Commands (Only Native):** Strictly prohibit the use of shell commands for Open vSwitch (e.g., no `ovs-vsctl`, `ovs-ofctl`, etc.). You must use *only native* implementations, bindings, and APIs.
- **No Stubs or Placeholders:** All code generated must be fully functional, complete, and production-ready. Do not leave "TODOs", stubbed functions, or incomplete placeholders.
- **Chimera OS Nuances:** 
  - **Init System (No systemd):** Chimera OS strictly uses **dinit**, not `systemd`. Never write, generate, or suggest `.service` files or use `systemctl`. Instead, use `dinitctl` and dinit service definitions.
  - **C Standard Library (musl, not glibc/glib):** The system uses **musl** libc, not `glibc` or `glib`. Ensure that all Rust FFI, C/C++ dependencies, and build pipelines are fully compatible with `musl` and do not rely on `glibc`-specific extensions.

## Build & Test Instructions

- **Rust Backend:** Use `cargo build --workspace` and `cargo test --workspace`.
- **Frontend:** Use `./update-ui.sh` from the root to sync the `operation-dashboard-ui` submodule and build the embedded assets. This script ensures dependencies for Generative UI (`@json-render/react`) and React Flow are correctly wired into the crate.
