#!/usr/bin/env bash
set -euo pipefail

# Optional helper for machines that cannot reach crates.io reliably.
# It writes a user-level Cargo config and redirects crates.io to rsproxy.

mkdir -p "$HOME/.cargo"

cat > "$HOME/.cargo/config.toml" <<'EOF'
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[registries.crates-io]
protocol = "sparse"

[net]
git-fetch-with-cli = true
retry = 5
timeout = 120
EOF

echo "Cargo mirror configured at $HOME/.cargo/config.toml"
echo "Run: cargo fetch"
