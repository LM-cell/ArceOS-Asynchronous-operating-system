#!/usr/bin/env bash
set -e

INPUT_FILE=${1:-schools.csv}
if [[ ! -f "$INPUT_FILE" ]]; then
  echo "错误: 找不到输入文件 $INPUT_FILE"
  exit 1
fi

echo "正在执行真实网络基准测试: $INPUT_FILE"
echo "--------------------------------------------------"

run_crawler() {
    local bin_name=$1
    local output
    # 增加超时保护，防止单个请求卡死 30s+
    output=$(timeout 60 cargo run --release --bin "$bin_name" -- --input "$INPUT_FILE" 2>&1) || true
    
    local model throughput p50 p95 mem
    model=$(echo "$output" | grep "^model=" | cut -d= -f2)
    throughput=$(echo "$output" | grep "^throughput_rps=" | cut -d= -f2)
    p50=$(echo "$output" | grep "latency_ms" | grep -oP 'p50=\K\d+')
    p95=$(echo "$output" | grep "latency_ms" | grep -oP 'p95=\K\d+')
    mem=$(echo "$output" | grep "^memory_kib=" | cut -d= -f2)
    
    printf "%-12s | RPS: %-8s | P50: %-6s ms | P95: %-6s ms | MEM: %-8s KB\n" \
           "$model" "$throughput" "$p50" "$p95" "$mem"
}

run_crawler process_crawler
run_crawler thread_crawler
run_crawler coroutine_crawler

echo "--------------------------------------------------"
echo "真实网络测试完成。建议重复 3 次取平均值以消除网络抖动。"