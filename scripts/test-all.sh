#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

echo "== cargo fmt =="
cargo fmt --all -- --check
(cd rkvm-master && cargo fmt --all -- --check)

echo "== clippy =="
cargo clippy --workspace --all-targets --no-deps -- -D warnings

echo "== cargo test (includes e2e_ipc / e2e_peer_channel / e2e_edge_engine) =="
cargo test --workspace
(cd rkvm-master && cargo test --workspace)

echo "== frontend unit + e2e contracts =="
npm test
npm run format:check
npm run build

echo "all automated tests passed"
echo "physical two-machine matrix is still manual (docs/TESTING.md)"
