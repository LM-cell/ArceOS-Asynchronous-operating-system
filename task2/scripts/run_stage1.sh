#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
TASK_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_DIR="${TASK_DIR}/logs"
LOG_FILE="${LOG_DIR}/stage1_original_latest.log"

mkdir -p "${LOG_DIR}"

cd "${TASK_DIR}"
cargo run --release -- stage1 | tee "${LOG_FILE}"

echo
echo "stage1 log saved to: ${LOG_FILE}"
