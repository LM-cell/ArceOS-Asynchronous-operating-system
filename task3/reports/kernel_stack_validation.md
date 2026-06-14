# 内核栈占用与线程状态验证说明

## 1. 为什么不能直接得到“真实内核栈实时使用字节数”

普通 Linux 用户态程序通常不能直接读取每个内核线程的内核栈实时深度。`/proc/self/status` 可以提供线程数、RSS、VmSize 等信息，但并不会告诉我们“每个内核栈当前用了多少字节”。

因此本实验采用可复现的代理指标来验证趋势：

- 内核线程数峰值：`peak_kernel_threads`
- 内核栈预留估算：`estimated_kernel_stack_reserved_bytes = peak_kernel_threads * kernel_stack_size`
- 每 1000 个任务对应的内核线程栈槽位：`kernel_stack_slots_per_1000_tasks`
- 线程状态分布：`running / sleeping / uninterruptible`
- 主要等待位置：`dominant_wait_channel`

这些指标不能精确表示内核栈已经使用了多少字节，但可以证明不同模型为了支撑同样数量任务，需要多少个内核线程栈槽位。

## 2. 如何解释新增指标

### OS 线程

OS 线程模型中，一个任务对应一个内核线程。

如果 1000 个任务成功运行，通常会看到：

```text
peak_kernel_threads ≈ 1000
kernel_stack_slots_per_1000_tasks ≈ 1000
```

这说明每 1000 个任务大约需要 1000 个内核线程栈槽位。

如果 10k、50k、100k 失败，需要查看：

```bash
cat data/failures.log
cat data/logs/os-thread-10000.log
cat data/logs/os-thread-50000.log
cat data/logs/os-thread-100000.log
```

失败本身就是实验结果，说明 OS 线程模型在该机器的系统限制下无法扩展到对应规模。

### 有栈绿色线程

有栈绿色线程中，任务数可以很多，但内核线程主要由 worker 数决定。

例如 10000 个绿色线程、4 个 worker 时，理论趋势是：

```text
peak_kernel_threads ≈ worker_count + sampler/main
kernel_stack_slots_per_1000_tasks 很小
```

这说明绿色线程没有为每个任务都创建内核线程栈，但每个绿色线程仍保留独立用户态栈。

### 无栈协程

无栈协程中，大量 Future 复用少量 Tokio worker。

例如 100000 个 async task、4 个 worker 时，通常会看到：

```text
peak_kernel_threads ≈ 4 + main/sampler
kernel_stack_slots_per_1000_tasks 接近 0
```

这说明大量挂起任务没有各自占用内核线程栈，这是验证“无栈协程高并发时内核栈槽位占用低”的关键证据。

## 3. 一次跑完整矩阵

运行：

```bash
cargo test
bash scripts/run_all.sh
python3 scripts/plot_results.py data/results.csv reports/figures
```

`scripts/run_all.sh` 会一次跑完整矩阵：

```text
models = os-thread, green-thread, async-future
tasks  = 1000, 10000, 50000, 100000
```

每个模型和任务数量都会单独启动一个进程，避免运行时缓存影响后续数据。

## 4. 输出文件

| 文件 | 作用 |
| --- | --- |
| `data/results.csv` | 每个成功数据点的汇总结果 |
| `data/samples.csv` | 每次采样的时序数据，包含线程状态 |
| `data/failures.log` | 失败数据点，例如 OS 线程创建失败 |
| `data/logs/*.log` | 每个数据点的完整运行日志 |
| `reports/figures/kernel_stack_slots_per_1000_tasks.png` | 每 1000 个任务对应的内核线程栈槽位 |
| `reports/figures/kernel_thread_states.png` | 峰值线程状态分布 |

## 5. 推荐写入报告的判断方式

可以按下面的逻辑写结论：

1. OS 线程在 1k 时 `kernel_stack_slots_per_1000_tasks` 接近 1000，说明每个任务基本都占用一个内核线程栈槽位。
2. OS 线程在更大规模下如果失败，结合 `data/failures.log` 说明该模型受系统线程数量限制。
3. green_thread 和 async_future 的 `kernel_stack_slots_per_1000_tasks` 随任务数增长快速下降，说明它们复用少量内核线程。
4. async_future 的用户态栈预留为 0，Future 状态机开销随任务数增长，但内核线程栈槽位数量基本稳定。
