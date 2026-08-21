#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

echo "[1/3] ledger evidence test"
cargo test -p op-cognitive-mcp development_ledger::tests::tracks_capability_and_verification_history --lib

echo "[2/3] sealed schema contract test"
cargo test -p op-plugins state_plugins::cognitive_mcp::tests::development_methods_are_sealed_with_expected_contracts --lib

echo "[3/3] canonical bridge compilation"
cargo check -p op-grpc-bridge

echo "cognitive development verification passed"
