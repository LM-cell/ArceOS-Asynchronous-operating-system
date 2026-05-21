#!/usr/bin/env bash
set -euo pipefail

MODE="real"
INPUT_FILE="${1:-data/schools.csv}"
REQUESTS="${REQUESTS:-30}"
OUTPUT_ROOT="${OUTPUT_ROOT:-outputs/benchmark}"

if [[ "${1:-}" == "--mock" ]]; then
  MODE="mock"
  REQUESTS="${2:-$REQUESTS}"
fi

if [[ "$MODE" == "real" && ! -f "$INPUT_FILE" ]]; then
  echo "error: input file not found: $INPUT_FILE"
  exit 1
fi

mkdir -p "$OUTPUT_ROOT"

extract_key() {
  local key=$1
  awk -F= -v key="$key" '$1 == key { print $2; exit }'
}

extract_latency() {
  local key=$1
  awk -v key="$key" '
    /latency_ms/ {
      for (i = 1; i <= NF; i++) {
        split($i, pair, "=")
        if (pair[1] == key) {
          print pair[2]
          exit
        }
      }
    }
  '
}

run_crawler() {
  local bin_name=$1
  local output_dir="$OUTPUT_ROOT/$bin_name"
  local output

  rm -rf "$output_dir"
  mkdir -p "$output_dir"

  if [[ "$MODE" == "mock" ]]; then
    output=$(timeout 120 cargo run --release --bin "$bin_name" -- \
      --mock --requests "$REQUESTS" --output-dir "$output_dir" 2>&1) || true
  else
    output=$(timeout 120 cargo run --release --bin "$bin_name" -- \
      --input "$INPUT_FILE" --output-dir "$output_dir" 2>&1) || true
  fi

  printf "%s\n" "$output" > "$output_dir/metrics.txt"

  local model throughput p50 p95 max mem bytes
  model=$(printf "%s\n" "$output" | extract_key "model")
  throughput=$(printf "%s\n" "$output" | extract_key "throughput_rps")
  bytes=$(printf "%s\n" "$output" | extract_key "bytes_saved")
  mem=$(printf "%s\n" "$output" | extract_key "memory_kib")
  p50=$(printf "%s\n" "$output" | extract_latency "p50")
  p95=$(printf "%s\n" "$output" | extract_latency "p95")
  max=$(printf "%s\n" "$output" | extract_latency "max")

  printf "%-10s | RPS: %-8s | P50: %-6s ms | P95: %-6s ms | MAX: %-6s ms | MEM: %-8s KiB | BYTES: %s\n" \
    "${model:-$bin_name}" "${throughput:-n/a}" "${p50:-n/a}" "${p95:-n/a}" "${max:-n/a}" "${mem:-n/a}" "${bytes:-n/a}"
}

echo "mode=$MODE"
if [[ "$MODE" == "real" ]]; then
  echo "input=$INPUT_FILE"
else
  echo "requests=$REQUESTS"
fi
echo "output_root=$OUTPUT_ROOT"
echo "----------------------------------------------------------------------------------------------------"

run_crawler process_crawler
run_crawler thread_crawler
run_crawler coroutine_crawler

echo "----------------------------------------------------------------------------------------------------"
echo "Each crawler output directory contains downloaded text files and metrics.txt."
