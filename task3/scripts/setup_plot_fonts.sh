#!/usr/bin/env bash
set -euo pipefail

# Install CJK fonts needed by matplotlib to render Chinese titles/comments.

if ! command -v apt-get >/dev/null 2>&1; then
  echo "apt-get not found. Please install a CJK font manually, for example Noto Sans CJK." >&2
  exit 1
fi

sudo apt-get update
sudo apt-get install -y fonts-noto-cjk fonts-wqy-microhei

# Matplotlib caches font discovery; clear it so the new font is picked up.
rm -rf "$HOME/.cache/matplotlib"
fc-cache -fv >/dev/null

echo "CJK fonts installed. Re-run:"
echo "python3 scripts/plot_results.py data/results.csv reports/figures"
