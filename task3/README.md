# task3: Rust 执行流模型栈与内存实验

本目录提供一套可运行的 Rust 实验模板，用于比较三种执行流模型的栈和内存占用：

- OS 线程：`std::thread::spawn`
- 有栈绿色线程：`may` coroutine
- 无栈协程：`async/await` + Tokio runtime

## 目录结构

```text
task3/
├── Cargo.toml
├── data/       # 实验输出 CSV/JSON
├── reports/    # 报告模板和图表
├── scripts/    # 批量运行与画图脚本
├── src/        # 实验代码
└── tests/      # 调度策略测试
```

## 快速运行

Linux 上推荐使用独立进程逐点采集，避免运行时缓存影响后续数据：

```bash
cargo test
bash scripts/run_all.sh
python scripts/plot_results.py data/results.csv reports/figures
```

`scripts/run_all.sh` runs all three models for `1k / 10k / 50k / 100k`.
If OS-thread data points fail, inspect `data/failures.log` and `data/logs/*.log`.
Kernel-stack validation notes are in `reports/kernel_stack_validation.md`.

如果云主机访问 crates.io 超时，先执行：

```bash
bash scripts/setup_cargo_mirror.sh
cargo fetch
```

单独运行一个模型：

```bash
cargo run --release -- \
  --models async-future \
  --tasks 100000 \
  --sleep-ms 10 \
  --csv data/results.csv \
  --json data/async-future-100000.json \
  --samples-csv data/samples.csv
```

报告模板见 `reports/experiment_report_template.md`，没有实验数据的部分已经预留待填写。
