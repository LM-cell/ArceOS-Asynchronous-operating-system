#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
TASK_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_DIR="${TASK_DIR}/logs"
LOG_FILE="${LOG_DIR}/tokio_future_latest.log"

mkdir -p "${LOG_DIR}"

cd "${TASK_DIR}"
cargo run --release -- tokio-future | tee "${LOG_FILE}"

echo
echo "tokio future log saved to: ${LOG_FILE}"
