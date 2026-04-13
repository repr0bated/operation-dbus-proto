# Zenflow Task List: Refactor `op-services` for Unified RCP Architecture

This task list is designed to guide Zenflow through the process of stripping legacy local state (`sqlite`/`sqlx`) out of `op-services` and converting it into a stateless reconciliation engine that communicates directly with the authoritative `op-dbus` RCP stores (OVSDB / NonNet) via gRPC or JSON-RPC.

## Task 1: Remove SQLite and `sqlx` Dependencies
1. Open `crates/op-services/Cargo.toml`.
2. Remove the `sqlx` dependency entirely.
3. Remove any other SQLite-specific dependencies if present.
4. Run `cargo check -p op-services` to identify all compilation errors caused by the removal of `sqlx`.

## Task 2: Delete Local Storage Logic
1. Delete the file `crates/op-services/src/store/mod.rs` completely, or gut it if it must be repurposed as a trait/interface boundary.
2. Remove any logic in `crates/op-services/src/bin/op-services.rs` (or `main.rs`) that initializes a SQLite database connection pool, creates database files (`state.db`), or runs SQL migrations.
3. Remove the custom `audit_log` insertion logic. The system now relies entirely on the central BTRFS `timing_subvol` blockchain for audit trails.

## Task 3: Implement Stateless RCP Client
1. Create a new module (e.g., `crates/op-services/src/client/mod.rs` or repurpose `store`) to act as the stateless client.
2. Use the existing `tonic` and `prost` dependencies to implement a gRPC client connecting to `op-dbus` at `10.200.0.2:50051` (or the configured `OP_DBUS_GRPC_ADDR` from the environment).
3. Implement methods to fetch the desired `ServiceDef` state from the central NonNet store via the `StateSyncServer` or equivalent gRPC endpoints.
4. Implement methods to submit state mutations (e.g., when a user requests a service restart) directly to the central `op-dbus` server instead of saving them locally.

## Task 4: Update Service Manager Core
1. Update `crates/op-services/src/manager/service_manager.rs` to use the new stateless RCP client instead of the old SQLite `Store`.
2. Ensure the reconciliation loop queries the central `op-dbus` state and applies it locally via `dinit` (using the existing `DinitProxy`).
3. Ensure that when `op-services` detects a state change from `dinit` (e.g., a service crashed), it reports that status change back to the central `op-dbus` server via gRPC, rather than updating a local database.

## Task 5: Verify and Test
1. Run `cargo clippy -p op-services` to ensure all type errors from the `sqlx` removal have been resolved.
2. Build the project: `cargo build -p op-services`.
3. Verify that `op-services` starts successfully without attempting to create or access a local `.db` file.
4. Ensure that `op-services` successfully connects to the `op-dbus` gRPC port (`50051`) on startup.