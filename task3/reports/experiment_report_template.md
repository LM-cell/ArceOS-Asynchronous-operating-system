# Rust 执行流模型栈与内存占用实验报告

## 1. 实验目标

本实验比较三种执行流模型在高并发场景下的资源占用差异：

- OS 线程：每个任务对应一个系统线程，拥有独立用户态线程栈和内核线程栈。
- 有栈绿色线程：每个任务对应一个用户态 coroutine，拥有独立用户态栈，多个 coroutine 复用少量内核线程。
- 无栈协程：每个任务对应一个 Future 状态机，挂起时不保留独立用户态栈，多个 Future 复用少量内核线程。

重点观察：

- 用户态栈预留量随任务数增长的变化。
- 内核态栈预留量随内核线程数增长的变化。
- 进程整体 RSS / VmSize 随并发增长的变化。
- 无栈协程在大量任务挂起时，内核线程数保持较低，内核栈大部分处于空闲/未增长状态的现象。

## 2. 实验环境

待填写：

| 项目 | 内容 |
| --- | --- |
| CPU |  |
| 内存 |  |
| 操作系统 |  |
| Linux 内核版本 |  |
| Rust 版本 |  |
| Tokio 版本 |  |
| may 版本 |  |
| 编译模式 | release |

## 3. 实验方法

### 3.1 执行流模型

| 模型 | 用户态上下文保存方式 | 内核线程数量 | 调度方式 |
| --- | --- | --- | --- |
| OS 线程 | 独立 OS 线程栈 | 约等于任务数 | 内核调度 |
| 有栈绿色线程 | 独立用户态 coroutine 栈 | 约等于 worker 数 | 用户态调度 |
| 无栈协程 | Future 状态机 | 约等于 worker 数 | 用户态调度 |

### 3.2 任务负载

每个任务执行以下操作：

1. 在进入等待前触碰固定大小的用户态栈，用于制造可观测栈压力。
2. sleep 10 ms，模拟 I/O 等待或阻塞点。
3. 返回 checksum，防止编译器消除栈触碰逻辑。

默认参数：

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--tasks` | 1000 / 10000 / 50000 / 100000 | 并发任务数量 |
| `--sleep-ms` | 10 | 每个任务等待时间 |
| `--os-stack-kib` | 64 | OS 线程栈配置 |
| `--green-stack-kib` | 64 | may coroutine 栈配置 |
| `--touch-stack-kib` | 8 | 每个任务实际触碰的栈空间 |
| `--kernel-stack-kib` | 16 | 单个内核线程栈估算值 |

### 3.3 测量指标

| 指标 | 来源 | 说明 |
| --- | --- | --- |
| 用户态栈预留量 | `task_count * stack_size` | OS 线程和有栈绿色线程使用配置值估算 |
| Future 状态机大小 | `std::mem::size_of_val` | 无栈协程单个 Future 的近似上下文大小 |
| 内核线程数峰值 | `/proc/self/status` 的 `Threads` | Linux 下采样得到 |
| 内核栈预留估算 | `peak_threads * kernel_stack_size` | 用于比较趋势，不代表实时内核栈深度 |
| RSS 峰值 | `/proc/self/status` 的 `VmRSS` | 进程实际驻留内存 |
| VmSize 峰值 | `/proc/self/status` 的 `VmSize` | 进程虚拟地址空间 |

说明：普通用户态程序无法直接读取每个内核线程的实时内核栈深度。本实验使用“内核线程数 × 单线程内核栈大小”估算内核栈预留量，并结合高并发 sleep 场景说明 async Future 挂起时不需要为每个任务保留内核栈。

## 4. 运行方式

正式采集建议在 Linux 上运行：

```bash
cargo test
bash scripts/run_all.sh
python scripts/plot_results.py data/results.csv reports/figures
```

单独运行某个数据点：

```bash
cargo run --release -- \
  --models async-future \
  --tasks 100000 \
  --sleep-ms 10 \
  --os-stack-kib 64 \
  --green-stack-kib 64 \
  --touch-stack-kib 8 \
  --kernel-stack-kib 16 \
  --csv data/results.csv \
  --json data/async-future-100000.json \
  --samples-csv data/samples.csv
```

## 5. 实验结果

待填写：将 `data/results.csv` 的核心数据填入下表。

| 模型 | 任务数 | 用户态栈预留 | Future 状态机估算 | 峰值内核线程数 | 内核栈预留估算 | RSS 峰值 | VmSize 峰值 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| os_thread | 1,000 |  |  |  |  |  |  |
| os_thread | 10,000 |  |  |  |  |  |  |
| green_thread | 1,000 |  |  |  |  |  |  |
| green_thread | 10,000 |  |  |  |  |  |  |
| async_future | 1,000 |  |  |  |  |  |  |
| async_future | 10,000 |  |  |  |  |  |  |
| async_future | 100,000 |  |  |  |  |  |  |

图表位置待填写：

- 用户态栈预留曲线：`reports/figures/user_stack_reserved.png`
- 内核栈预留估算曲线：`reports/figures/kernel_stack_reserved.png`
- RSS 峰值曲线：`reports/figures/peak_rss.png`

## 6. 结果分析

待填写实验数据后补充：

1. OS 线程的用户态栈预留量和内核线程数均随任务数线性增长，因此在高并发下最容易遇到线程创建失败、虚拟内存膨胀或调度开销过高。
2. 有栈绿色线程的内核线程数量由 worker 数决定，不随任务数线性增长；但每个 coroutine 仍有独立用户态栈，因此用户态栈预留量仍随任务数线性增长。
3. 无栈协程的任务上下文主要保存为 Future 状态机，挂起后不保留独立用户态栈；内核线程数量接近 runtime worker 数，因此内核栈预留估算基本稳定。
4. 当任务主要处于 sleep/I/O 等待状态时，无栈协程可以承载更高并发，RSS 增长主要来自 Future、Tokio task 分配、调度队列和计时器结构，而不是线程栈。

## 7. 优先级调度扩展验证

代码中提供了两个简化优先级调度器：

- 协作式优先级调度：高优先级队列先运行，低优先级队列后运行。
- 时间片抢占式优先级调度：每个 tick 重新选择最高优先级 ready 任务，高优先级任务到达后可以抢占尚未完成的低优先级任务。

测试命令：

```bash
cargo test priority
```

预期结果：

- 协作式：高优先级任务 `high-1`、`high-2` 在低优先级任务 `low-1`、`low-2` 前完成。
- 抢占式：`low-long` 在 tick 0 先运行，`high-short` 在 tick 1 到达后抢占执行，并先于 `low-long` 完成。

待填写测试输出：

```text

```

## 8. 结论

待填写：

- 在本机环境下，OS 线程达到的最大稳定并发数为：。
- 有栈绿色线程达到的最大稳定并发数为：。
- 无栈协程达到的最大稳定并发数为：。
- 综合 RSS、VmSize、内核线程数和任务完成时间，无栈协程在 I/O 密集高并发场景下的资源占用最低。
