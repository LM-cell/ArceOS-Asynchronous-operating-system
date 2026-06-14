#!/usr/bin/env bash
set -euo pipefail

# Recommended on Linux. Each data point runs in a fresh process so runtime
# caches from may/Tokio do not bias later RSS samples.

MODELS=("os-thread" "green-thread" "async-future")
TASKS=(1000 10000 50000 100000)

CSV_PATH="data/results.csv"
SAMPLES_PATH="data/samples.csv"
FAILURE_LOG="data/failures.log"
LOG_DIR="data/logs"
RUN_TIMEOUT="${RUN_TIMEOUT:-300s}"
OS_THREAD_TIMEOUT="${OS_THREAD_TIMEOUT:-30s}"

stop_requested=false
current_child=""

cleanup_on_interrupt() {
  stop_requested=true
  echo
  echo "Interrupted. Stopping experiment matrix."
  if [[ -n "${current_child}" ]] && kill -0 "${current_child}" 2>/dev/null; then
    kill -TERM "-${current_child}" 2>/dev/null || kill -TERM "${current_child}" 2>/dev/null || true
  fi
  exit 130
}

trap cleanup_on_interrupt INT TERM

rm -f "$CSV_PATH" "$SAMPLES_PATH" "$FAILURE_LOG"
rm -rf "$LOG_DIR"
mkdir -p "$LOG_DIR"

echo "Running models: ${MODELS[*]}"
echo "Running task counts: ${TASKS[*]}"
echo "Each model/task-count pair runs in a fresh process."
echo "Per-run timeout: ${RUN_TIMEOUT}"
echo "OS-thread timeout: ${OS_THREAD_TIMEOUT}"

for model in "${MODELS[@]}"; do
  for tasks in "${TASKS[@]}"; do
    if [[ "${stop_requested}" == "true" ]]; then
      exit 130
    fi

    run_log="${LOG_DIR}/${model}-${tasks}.log"
    timeout_value="$RUN_TIMEOUT"
    if [[ "$model" == "os-thread" ]]; then
      timeout_value="$OS_THREAD_TIMEOUT"
    fi

    echo "==> model=${model} tasks=${tasks}"
    setsid timeout "$timeout_value" cargo run --release -- \
      --models "$model" \
      --tasks "$tasks" \
      --sleep-ms 10 \
      --os-stack-kib 64 \
      --green-stack-kib 64 \
      --touch-stack-kib 8 \
      --kernel-stack-kib 16 \
      --sample-interval-ms 5 \
      --csv "$CSV_PATH" \
      --json "data/${model}-${tasks}.json" \
      --samples-csv "$SAMPLES_PATH" \
      --append-csv >"$run_log" 2>&1 &
    current_child=$!

    if wait "$current_child"; then
      current_child=""
      echo "ok model=${model} tasks=${tasks}"
    else
      status=$?
      current_child=""
      if [[ "$status" -eq 130 ]] || [[ "$status" -eq 143 ]]; then
        exit "$status"
      fi
      echo "failed model=${model} tasks=${tasks} log=${run_log}" | tee -a "$FAILURE_LOG"
    fi
  done
done

echo "summary: ${CSV_PATH}"
echo "samples: ${SAMPLES_PATH}"
echo "failures: ${FAILURE_LOG}"
echo "per-run logs: ${LOG_DIR}"
