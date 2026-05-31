#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
bash "${SCRIPT_DIR}/run_stage2.sh"
bash "${SCRIPT_DIR}/run_stackless_coroutine.sh"
bash "${SCRIPT_DIR}/run_futures_200.sh"
bash "${SCRIPT_DIR}/run_tokio_future.sh"
