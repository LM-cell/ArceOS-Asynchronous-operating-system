# Task2：绿色线程 / Stackful Coroutine

本目录选择 `green thread / stackful coroutine` 作为 task2 实践对象。

- 阶段 1：跑通原始绿色线程，观察 round-robin 调度日志。
- 阶段 2：在绿色线程中扩展优先级调度，并用测试验证正确性。
- Tokio Future：通过 `poll / Pending / wake / Ready` 日志分析 async 执行流。
- Stack-less Coroutine：用 Rust stable `async/await` 和标准库实现 100 行以内的安全无栈协程库，并动态跟踪 Future 状态机。
- Futures Explained 200 Lines：用接近 200 行的标准库 executor/reactor/waker 示例解释 Future 的设计和并发替代方案。

## 运行

在 Linux x86_64 环境中执行：

```bash
cd /home/ArceOS/ArceOS-Asynchronous-operating-system/task2
cargo test -- --nocapture
bash ./scripts/run_stage2.sh
bash ./scripts/run_stackless_coroutine.sh
bash ./scripts/run_futures_200.sh
bash ./scripts/run_tokio_future.sh
bash ./scripts/run_benchmark.sh
```

如果希望直接用 `./scripts/run_benchmark.sh` 这种方式执行，先添加执行权限：

```bash
chmod +x ./scripts/*.sh
./scripts/run_benchmark.sh
```

如果想直接运行某个 demo：

```bash
cargo run --release -- stage1
cargo run --release -- stage2
cargo run --release -- stackless-coroutine
cargo run --release -- futures-200
cargo run --release -- tokio-future
```

阶段 2 脚本会把日志保存到：

```text
logs/stage2_priority_latest.log
```

阶段 1 的原始脚本仍保留：

```bash
bash ./scripts/run_stage1.sh
```

它会把日志保存到：

```text
logs/stage1_original_latest.log
```

仓库中也保留了一份阶段 1 参考输出：

```text
logs/stage1_original_sample.log
```

## 实现范围

### 模块结构

```text
src/main.rs                 # 只做 demo 选择与统一入口
src/green_thread/
  context.rs                # x86_64 用户态上下文切换汇编
  runtime.rs                # GreenThread / Runtime / priority scheduler
  demo.rs                   # 阶段 1 round-robin 与阶段 2 priority 示例
src/stackless_coroutine/
  tiny.rs                   # 标准库-only、无 unsafe、无独立栈的极小 coroutine executor
  trace.rs                  # Stack-less trace 日志编号
  demo.rs                   # heap-pinned Future coroutine 示例
src/futures_explained_200/
  runtime.rs                # 约 200 行 executor / reactor / waker / Delay Future
  trace.rs                  # Futures Explained trace 日志编号
  demo.rs                   # 阻塞、线程、Future 三种并发方案对照
src/tokio_future/
  trace.rs                  # Tokio trace 日志编号
  traced_future.rs          # 手写 Future，跟踪 poll/Pending/wake/Ready
  demo.rs                   # Tokio current-thread runtime 示例
```

### Green Thread

- 每个绿色线程拥有独立用户态栈。
- 调度器保存和恢复 `rsp/rbp/rbx/r12-r15` 等 x86_64 callee-saved 寄存器。
- 用户态线程通过 `yield_now()` 主动让出 CPU。
- 阶段 1 实现 round-robin 协作式调度。
- 阶段 2 增加 `priority` 字段与 `spawn_with_priority(name, priority, entry)`。
- 阶段 2 调度策略：先选 `Ready` 中最大 `priority`，同优先级继续 round-robin。
- 动态跟踪通过运行时打点输出 `create / dispatch / yield / return / switch` 日志，并包含 `priority`。

### Tokio Future

- 使用 Tokio current-thread runtime 运行多个 async task。
- 用手写 `YieldOnce` Future 记录 `poll -> Pending -> wake -> poll -> Ready`。
- 动态跟踪输出 `spawn / enter / before-await / poll / pending / ready / after-await / return`。
- 用于对比 stackful green thread 和 stack-less Future 状态机的执行流差异。

### Stack-less Coroutine

- 使用 `Pin<Box<dyn Future<Output = ()>>>` 保存 coroutine 状态机。
- 使用标准库 `Wake` trait 构造安全 `Waker`，不手写 `RawWaker`。
- executor 只维护 ready queue：`poll` 得到 `Pending` 时重新入队，得到 `Ready` 时结束。
- `tiny.rs` 核心库保持 100 行以内，不包含汇编、不分配独立用户态栈。
- 动态跟踪输出 `create / poll / pending / requeue / finish`，用于对比 green thread 的 `switch` 日志。

### Futures Explained 200 Lines

- 用 `Spawner / Executor / Task / Delay` 展示 Future runtime 的核心部件。
- executor 只负责 poll，timer reactor 负责等待事件，`Waker` 负责把 task 放回 ready queue。
- 动态跟踪输出 `spawn / poll / Pending / reactor register / wake / Ready / finish`。
- 同一 demo 中对照阻塞顺序执行、OS 线程并发和 Future 事件驱动三种方案。
- 用于解释 Rust Future 为什么将计算推进、等待事件和重新调度拆成不同抽象。

## 文档

阶段文档：

```text
docs/stage1_green_thread_trace.md
docs/stage2_priority_scheduler.md
docs/stackless_coroutine_trace.md
docs/futures_explained_200_trace.md
docs/tokio_future_trace.md
```

总结报告：

```text
reports/experiment_summary.md
```
