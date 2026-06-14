# 实验数据分析总结：有栈执行流与无栈执行流的栈资源占用对比

## 1. 数据来源与有效实验范围

本次分析基于以下数据文件：

- `data/results.csv`：每组实验的峰值统计结果。
- `data/samples.csv`：实验运行过程中的实时采样数据。
- `data/`：由脚本生成的可视化图表。

当前 CSV 中包含 7 条有效实验记录：

| 模型 | 有效任务规模 |
| --- | --- |
| `os_thread` | 1k |
| `green_thread` | 1k、10k |
| `async_future` | 1k、10k、50k、100k |

注意：当前结果中没有 `os_thread` 在 10k、50k、100k 下的有效记录，也没有 `green_thread` 在 50k、100k 下的有效记录。因此这些点不应画成 0，而应在图表和报告中标记为缺失、失败或未完成。

## 2. 指标说明

本实验重点比较用户栈、内核栈和整体内存随任务数量增长的变化。

| 指标 | 含义 |
| --- | --- |
| **估算的用户栈预留内存** | 估算的用户态栈预留量。OS thread 和 green thread 按 `任务数量 × 每个任务配置的用户态栈大小` 计算。 |
| **内核栈预留的内存** | 估算的内核栈预留量。当前按 ` 峰值内核线程数 × 单个内核线程栈大小估算值` 近似计算。 |
| `kernel_threads_per_task` | 单位任务内核线程数，计算公式为 `peak_kernel_threads / task_count`。 |
| `user_stack_reserved_bytes_per_task` | 单位任务用户栈预留量。平均每个任务需要“准备”多少栈资源 |
| `kernel_stack_reserved_bytes_per_task` | 单位任务内核栈预留量。平均每个任务需要“准备”多少栈资源 |
| `future_state_bytes_per_task` | 单个 Rust `Future` 状态机大小，本实验中为 168 bytes。 |
| `peak_rss_bytes` | 进程峰值 RSS，表示实际驻留物理内存，不等于栈的虚拟地址空间预留量。 |

## 3. 核心结果表

| 模型 | 任务数 | 峰值内核线程数 | 单位任务内核线程数 | 用户栈预留 MiB | 内核栈预留 MiB | Future 状态机 MiB | 峰值 RSS MiB | 单位任务总栈预留 KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `os_thread` | 1,000 | 984 | 0.984000 | 62.50 | 15.38 | 0.00 | 19.13 | 79.744 |
| `green_thread` | 1,000 | 7 | 0.007000 | 62.50 | 0.11 | 0.00 | 19.08 | 64.112 |
| `green_thread` | 10,000 | 7 | 0.000700 | 625.00 | 0.11 | 0.00 | 159.12 | 64.011 |
| `async_future` | 1,000 | 6 | 0.006000 | 0.00 | 0.09 | 0.16 | 4.04 | 0.096 |
| `async_future` | 10,000 | 6 | 0.000600 | 0.00 | 0.09 | 1.60 | 9.13 | 0.010 |
| `async_future` | 50,000 | 6 | 0.000120 | 0.00 | 0.09 | 8.01 | 31.02 | 0.002 |
| `async_future` | 100,000 | 6 | 0.000060 | 0.00 | 0.09 | 16.02 | 58.82 | 0.001 |

## 4. 单位任务资源占用对比

| 模型 | 任务数 | 用户栈 KiB/task | 内核栈 KiB/task | Future 状态机 bytes/task | 内核栈槽位/1000 tasks |
| --- | ---: | ---: | ---: | ---: | ---: |
| `os_thread` | 1,000 | 64.000 | 15.744 | 0 | 984.00 |
| `green_thread` | 1,000 | 64.000 | 0.112 | 0 | 7.00 |
| `green_thread` | 10,000 | 64.000 | 0.011 | 0 | 0.70 |
| `async_future` | 1,000 | 0.000 | 0.096 | 168 | 6.00 |
| `async_future` | 10,000 | 0.000 | 0.010 | 168 | 0.60 |
| `async_future` | 50,000 | 0.000 | 0.002 | 168 | 0.12 |
| `async_future` | 100,000 | 0.000 | 0.001 | 168 | 0.06 |

这个表最能体现实验重点：

- `os_thread` 在 1k 任务时已经接近“一任务一 kernel thread”，单位任务内核线程数为 0.984。
- `green_thread` 复用少量 OS worker，因此内核栈槽位不随任务数线性增长，但每个任务仍保留 64 KiB 用户栈。
- `async_future` 不为每个任务分配独立用户栈，只保存 168 bytes 的 `Future` 状态机；100k 任务时内核线程仍只有 6 个。

## 5. OS 线程模型分析

`os_thread` 在 1k 任务下的峰值内核线程数为 984，`kernel_threads_per_task = 0.984`，说明该模型基本符合：

```text
任务数 N -> N 个 OS thread -> N 份用户栈 + N 份 kernel stack
```

本次 1k 数据中：

- 用户栈预留约 62.50 MiB。
- 内核栈预留约 15.38 MiB。
- 单位任务总栈预留约 79.744 KiB。
- 峰值 RSS 约 19.13 MiB。
- 采样中大量线程处于 sleeping 或 blocked 状态，主要 wait channel 包括 `futex_wait_queue_me` 和 `hrtimer_nanosleep`。

结论：OS thread 模型的栈资源和内核调度实体数量随任务数近似线性增长。它适合少量并行任务，但在 10k 以上任务规模下很容易受系统线程数、PID、虚拟内存、调度开销等限制影响。

## 6. 有栈绿色线程模型分析

`green_thread` 使用少量 OS worker 承载大量用户态 coroutine。当前数据中，1k 和 10k 任务的峰值内核线程数都为 7。

本次数据中：

| 任务数 | 用户栈预留 | 内核栈预留 | 峰值 RSS | 单位任务内核线程数 |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 62.50 MiB | 0.11 MiB | 19.08 MiB | 0.007000 |
| 10,000 | 625.00 MiB | 0.11 MiB | 159.12 MiB | 0.000700 |

可以看到，green thread 的内核栈压力很小，因为 OS worker 数量基本固定；但是用户栈预留仍然随任务数线性增长。10k 任务时，按每任务 64 KiB 计算，用户栈预留已经达到 625 MiB。

结论：有栈绿色线程解决了“一任务一 kernel thread”的问题，**但没有解决“一任务一用户栈”的问题。它的主要成本从 kernel stack 转移到了 user stack。**

## 7. 无栈 async/Future 模型分析

`async_future` 的结果最符合无栈协程实验预期。任务数从 1k 增加到 100k 时，峰值内核线程数始终为 6。

| 任务数 | 用户栈预留 | 内核栈预留 | Future 状态机总量 | 峰值 RSS | 单位任务内核线程数 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 0.00 MiB | 0.09 MiB | 0.16 MiB | 4.04 MiB | 0.006000 |
| 10,000 | 0.00 MiB | 0.09 MiB | 1.60 MiB | 9.13 MiB | 0.000600 |
| 50,000 | 0.00 MiB | 0.09 MiB | 8.01 MiB | 31.02 MiB | 0.000120 |
| 100,000 | 0.00 MiB | 0.09 MiB | 16.02 MiB | 58.82 MiB | 0.000060 |

这说明：

- async task 没有独立用户栈。
- 挂起上下文主要保存在 `Future` 状态机中。
- 内核线程数由 Tokio runtime worker、主线程和采样线程决定，而不是由任务数决定。
- 100k 任务时，单位任务内核栈预留约 0.001 KiB，单位任务内核线程数只有 0.000060。

结论：无栈协程不是让大量“独立内核栈”处于空闲，而是避免为每个任务创建独立 OS thread 和 kernel stack。大量挂起 Future 共享少量 runtime worker 的内核线程栈槽位，因此高并发 I/O/sleep 场景下栈资源占用显著更低。

## 8. 内核栈实时利用率的解释

本实验中的“内核栈利用率”更准确地说是“单位任务对应的 kernel stack slot 数量”或“单位任务内核栈预留压力”。

从数据看：

- `os_thread/1000` 的 `kernel_stack_slots_per_1000_tasks = 984.0`，说明每 1000 个任务大约对应 984 个 kernel stack slot。
- `green_thread/10000` 的该指标为 0.7，说明每 1000 个任务只对应 0.7 个 kernel stack slot。
- `async_future/100000` 的该指标为 0.06，说明每 1000 个任务只对应 0.06 个 kernel stack slot。

这能够证明：无栈 Future 在高并发任务下不会制造大量 kernel stack slot，内核态栈资源不会随任务数量线性增长。



当前实验已经能完成“内核栈槽位数量不随 async task 数增长”的验证目标。

## 9. 总体结论

本次实验数据支持以下结论：

1. `os_thread` 的 user stack 和 kernel stack 槽位都随任务数近似线性增长，1k 任务时峰值内核线程数已经达到 984。
2. `green_thread` 显著降低 kernel thread/kernel stack 数量，1k 到 10k 任务都只需要 7 个内核线程；但每个任务仍有 64 KiB 用户栈，因此 user stack 预留从 62.50 MiB 增长到 625.00 MiB。
3. `async_future` 没有独立用户栈，100k 任务时 Future 状态机总量约 16.02 MiB，峰值 RSS 约 58.82 MiB，内核线程仍只有 6 个。
4. 从单位任务指标看，`async_future/100000` 的单位任务内核线程数为 0.000060，单位任务内核栈预留约 0.001 KiB，明显低于 OS thread 和 green thread。
5. 因此，在 I/O 或 sleep 型高并发场景下，无栈协程的优势主要体现在：不为每个任务创建独立 user stack，也不为每个任务创建独立 kernel thread/kernel stack。

## 10. 图表引用建议

报告中建议优先引用以下图表：

- `data/user_stack_reserved.png`：展示 user stack 总预留量。
- `data/kernel_threads_per_task.png`：展示单位任务内核线程数。
- `data/kernel_stack_slots_per_1000_tasks.png`：展示每 1000 个任务对应的 kernel stack slot 数量。
- `data/stack_reserved_per_task_breakdown.png`：展示单位任务 user stack/kernel stack 构成。
- `data/future_state_total.png`：展示 Future 状态机总量随任务数增长。
- `data/peak_rss.png`：展示整体 RSS 内存变化。

