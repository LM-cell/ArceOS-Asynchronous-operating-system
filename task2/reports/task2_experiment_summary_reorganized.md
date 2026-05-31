# Task2 总结报告

## 1. 实验路线

本实验围绕“用户态线程与协程”的执行流机制展开，目标是理解不同执行流模型在创建、运行、挂起、恢复和结束过程中的状态变迁，并在此基础上对其中一种执行流机制进行调度策略扩展。

结合当前实验工作，报告路线分为两个子任务：

1. **子任务一：四种执行流机制的动态跟踪分析对比与总结**  
   通过运行时打点和日志分析，观察 Tokio Future、200 行绿色线程、100 行无栈协程、Futures Explained 200 Lines 四种执行流机制的状态变迁过程。

2. **子任务二：绿色线程优先级扩展与测试验证总结**  
   在原始绿色线程 round-robin 协作式调度基础上，扩展优先级调度支持，并通过单元测试验证优先级机制的正确性。

---

# 子任务一：四种执行流机制的动态跟踪分析对比与总结

## 2. 子任务一目标

子任务一围绕四种执行流机制展开动态跟踪分析：

| 序号 | 执行流 | 类型 | 观察重点 |
|---|---|---|---|
| 1 | 绿色线程 | Stackful green thread | 用户态栈切换、寄存器上下文保存、协作式调度 |
| 2 | Stack-less Rust coroutine under 100 LoC | Stack-less coroutine | Future 状态机、`poll -> Pending -> requeue` |
| 3 | Futures Explained 200 Lines | Executor + Reactor + Waker | 外部事件、`Waker` 唤醒、任务重新调度 |
| 4 | Tokio Future | 生产级 async runtime | `spawn / poll / Pending / wake / Ready` 状态推进 |

四种机制都可以抽象为：

```text
Created / Ready -> Running -> Suspended / Pending / Ready -> Running -> Finished
```

但是它们的区别在于：**执行状态保存在哪里、谁负责调度、挂起由什么触发、恢复由什么触发、是否依赖独立用户态栈。**

---

## 3. 200 行绿色线程动态跟踪分析

绿色线程属于 **stackful coroutine / 有栈协程**。每个绿色线程拥有独立用户态栈，运行时通过保存和恢复寄存器上下文完成执行流切换。

核心组件包括：

| 组件 | 作用 |
|---|---|
| `GreenThread` | 保存线程 id、名称、状态、优先级、独立栈、上下文和入口函数 |
| `Context` | 保存 `rsp/rbp/rbx/r12-r15` 等寄存器 |
| `Runtime` | 管理所有绿色线程、当前运行线程和主线程上下文 |
| `gt_switch` | 使用 x86_64 汇编保存和恢复上下文 |
| `yield_now()` | 当前绿色线程主动让出执行权 |
| `thread_bootstrap()` | 绿色线程第一次被调度时进入的启动函数 |

绿色线程状态变迁如下：

```text
Ready -> Running -> Ready
Ready -> Running -> Finished
```

典型动态跟踪过程：

```text
task:create id=0 name=alpha state=Ready stack=65536B
task:create id=1 name=beta state=Ready stack=65536B
task:create id=2 name=gamma state=Ready stack=65536B
runtime:run begin
state:task:0 alpha Ready->Running reason=dispatch
switch:main -> task:0
app:alpha step=0
state:task:0 alpha Running->Ready reason=yield
state:task:1 beta Ready->Running reason=dispatch
switch:task:0 -> task:1
```

当绿色线程入口函数返回时，线程进入 `Finished` 状态，不再参与调度：

```text
task:2 return from entry
state:task:2 gamma Running->Finished reason=return
```

观察结论：

1. 绿色线程恢复执行时不是重新 poll，而是从上一次被切走的栈位置继续执行。
2. 执行现场由用户态栈和寄存器上下文共同保存。
3. 原始调度策略是 round-robin，只选择下一个 `Ready` 线程。
4. 当前模型是协作式调度，只有任务主动 `yield_now()` 或函数返回时才会发生切换。

---

## 4. Stack-less Coroutine under 100 LoC 动态跟踪分析

该阶段实现一个只依赖 Rust stable `async/await` 和标准库 `Future` 的极小无栈协程运行时。它不使用独立用户栈，也不保存和恢复寄存器上下文，而是把每个 coroutine 保存为：

```rust
Pin<Box<dyn Future<Output = ()>>>
```

状态变迁如下：

```text
Created -> Ready queue -> Running -> Suspended -> Ready queue -> Running -> Finished
```

其中 `Suspended` 对应 `yield_now().await` 返回 `Poll::Pending`。

典型动态跟踪过程：

```text
coroutine:create name=alpha state=Created storage=heap-pinned-future
runtime:run begin executor=std-only-stackless
coroutine:poll name=alpha state=Running
app:alpha step=0 before-yield
coroutine:pending name=alpha state=Suspended action=requeue
coroutine:poll name=alpha state=Running
app:alpha step=0 after-yield
coroutine:finish name=alpha state=Finished
```

观察结论：

1. 无栈协程没有独立用户态栈。
2. 执行状态由编译器生成的 Future 状态机保存。
3. `.await` 是可挂起点。
4. `Poll::Pending` 表示当前不能继续执行。
5. executor 再次 poll 该 Future 时，状态机从上次 await 后继续推进。

---

## 5. Futures Explained 200 Lines 动态跟踪分析

Futures Explained 200 Lines 阶段实现了一个接近真实异步运行时结构的教学模型，重点解释 Rust Future 为什么采用 `poll / Pending / Waker` 接口。

该模型包含三层：

```text
Spawner  -> 创建 Task，放入 ready queue
Executor -> 取出 Task，构造 Waker，poll Future
Reactor  -> 等待外部事件，事件就绪后调用 Waker::wake()
```

状态变迁如下：

```text
Spawned / Ready -> Running -> Suspended -> Waiting event -> Ready -> Running -> Finished
```

典型动态跟踪过程：

```text
task:spawn id=0 name=alpha state=Ready storage=Future
executor:poll id=0 name=alpha state=Running
future:create task=alpha step=0 kind=Delay duration_ms=12
future:poll task=alpha step=0 kind=Delay
reactor:register task=alpha step=0 source=timer
future:pending task=alpha step=0 kind=Delay
executor:pending id=0 name=alpha state=Suspended
reactor:event-ready task=alpha step=0 source=timer
reactor:wake task=alpha step=0
waker:schedule id=0 name=alpha reason=wake executor_state=Ready
executor:poll id=0 name=alpha state=Running
future:ready task=alpha step=0 kind=Delay
```

观察结论：

1. Future 是惰性的，只有 executor poll 时才向前推进。
2. `Pending` 不是阻塞线程，而是表示任务暂时无法完成。
3. Reactor 负责等待外部事件。
4. Waker 是 reactor 与 executor 之间的通知桥梁。
5. task 被唤醒后重新进入 ready queue，等待 executor 再次 poll。

---

## 6. Tokio Future 动态跟踪分析

Tokio Future 阶段使用 Tokio current-thread runtime 运行多个 async task，并通过手写 `YieldOnce` Future 跟踪 `poll -> Pending -> wake -> poll -> Ready` 的状态变化。

状态变迁如下：

```text
Spawned / Ready -> Running -> Pending -> Ready queue -> Running -> Finished
```

典型动态跟踪过程：

```text
runtime:create kind=tokio-current-thread
task:spawn name=alpha state=Ready executor=tokio
task:enter name=alpha state=Running
app:alpha step=0 before-await
future:create task=alpha await_point=0 state=Created
future:poll task=alpha await_point=0 state=Running yielded=false
future:pending task=alpha await_point=0 state=Pending
future:wake task=alpha await_point=0 executor_state=Ready
future:poll task=alpha await_point=0 state=Running yielded=true
future:ready task=alpha await_point=0 state=Ready
app:alpha step=0 after-await
task:return name=alpha state=Finished
```

观察结论：

1. Tokio task 内部保存 async Future 状态机。
2. `.await` 处如果 Future 返回 `Pending`，任务会让出执行权。
3. Waker 将任务重新放回 Tokio ready queue。
4. Tokio 再次 poll 该 task 后，从上次 await 点继续推进。
5. Tokio 与 Futures Explained 的抽象一致，但 Tokio 是生产级运行时，隐藏了 ready queue、task header、waker 等底层细节。

---

## 7. 子任务一统一对比总结

| 模型 | 是否独立栈 | 状态保存位置 | 挂起触发 | 恢复触发 | 调度者 |
|---|---|---|---|---|---|
| 绿色线程 | 是 | 用户态栈 + 寄存器上下文 | `yield_now()` 或函数返回 | `gt_switch` 恢复上下文 | 自写 `Runtime` |
| 100 行无栈协程 | 否 | heap-pinned Future 状态机 | `yield_now().await` 返回 `Pending` | executor 重新 `poll` | 自写 executor |
| Futures Explained 200 Lines | 否 | Task Future + reactor state + Waker | `Delay.await` 返回 `Pending` | reactor 调用 `wake()` | executor + reactor |
| Tokio Future | 否 | Tokio task 内部 Future 状态机 | `.await` 内部 Future 返回 `Pending` | Tokio Waker 放回 ready queue | Tokio runtime |

统一结论：

1. 四种模型都能表达多个执行流之间的交错执行。
2. 绿色线程通过“保存栈和寄存器”恢复执行现场。
3. 无栈协程通过“Future 状态机 + poll”恢复执行。
4. Futures Explained 增加 reactor 和 waker，说明异步任务如何等待外部事件。
5. Tokio Future 是同一抽象的生产级实现。
6. 从执行流状态角度看，本质都是“可运行 -> 运行中 -> 挂起 -> 恢复 -> 完成”。

---

# 子任务二：绿色线程优先级扩展与测试验证总结

## 8. 子任务二目标

原始绿色线程采用 round-robin 协作式调度，只能按照线程创建顺序和当前位置进行轮转，无法表达任务重要性。因此子任务二选择绿色线程作为扩展对象，目标是：

1. 在 `GreenThread` 中增加优先级字段；
2. 新增带优先级的创建接口；
3. 修改调度器选择逻辑；
4. 保持同优先级任务 round-robin 公平性；
5. 编写测例验证优先级机制正确性。

---

## 9. 原始 round-robin 调度问题

Stage1 的原始调度策略为：

```text
从 current + 1 开始
        ↓
环形扫描所有线程
        ↓
选择第一个 Ready 线程
```

该策略优点是简单、公平，适合同优先级任务；但它无法表达“高优先级任务应优先运行”的需求。

---

## 10. 优先级机制设计

### 10.1 数据结构扩展

在 `GreenThread` 中新增字段：

```rust
priority: usize
```

约定：

```text
priority 数值越大，优先级越高。
```

### 10.2 创建接口扩展

新增接口：

```rust
spawn_with_priority(name, priority, entry)
```

保留原有接口：

```rust
spawn(name, entry)
```

其中 `spawn()` 默认使用：

```text
priority = 0
```

这样既能保持 Stage1 兼容，又能让 Stage2 显式创建不同优先级线程。

### 10.3 调度器修改

Stage2 修改 `pick_next()`，调度策略为：

```text
Ready 线程集合
        ↓
找出最大 priority
        ↓
从 current + 1 开始环形扫描
        ↓
选择第一个 priority 等于最大值的 Ready 线程
```

该策略满足两个目标：

1. 不同优先级：高优先级先运行；
2. 相同优先级：继续使用 round-robin。

---

## 11. 优先级调度动态跟踪分析

Stage2 demo 中创建了四个线程：

```text
low     priority=1
high-a  priority=3
high-b  priority=3
mid     priority=2
```

虽然 `low` 最先创建，但第一次调度选择的是 `priority=3` 的 `high-a`：

```text
task:create id=0 name=low priority=1 state=Ready
task:create id=1 name=high-a priority=3 state=Ready
task:create id=2 name=high-b priority=3 state=Ready
task:create id=3 name=mid priority=2 state=Ready
state:task:1 high-a priority=3 Ready->Running reason=dispatch
```

这说明调度器已经不是简单选择第一个 Ready 线程，而是优先选择最高优先级任务。

当 `high-a` 和 `high-b` 同为最高优先级时，它们在 yield 后交替执行：

```text
high-a priority=3 yield
dispatch high-b priority=3
high-b priority=3 yield
dispatch high-a priority=3
```

这说明同优先级任务仍然保持 round-robin 语义。



## 11. 扩展思考：从“栈资源”角度统一理解进程、线程与协程

通过本次实验可以进一步得到一个统一视角：**执行流切换的本质，是保存当前执行状态，并在之后恢复另一个执行状态。不同执行流机制的差异，不只是调度策略不同，更关键的是它们保存执行状态的方式不同。**

在 Stage1 和 Stage2 的绿色线程实验中，每个绿色线程都拥有独立用户态栈。任务调用 `yield_now()` 后虽然让出了执行权，但它的用户态栈、调用帧、局部变量和寄存器上下文仍然被保留。后续再次调度该绿色线程时，运行时通过 `gt_switch` 恢复寄存器和栈指针，使它从上一次让出的位置继续执行。因此，绿色线程更接近“有栈执行流”。

与之不同，stack-less coroutine、Futures Explained 200 Lines 和 Tokio Future 都属于无栈模型。它们在暂停时不保存完整调用栈，也不为每个任务长期占有独立栈资源，而是把**执行进度保存在 Future 状态机中**。任务执行到 `.await` 或返回 `Poll::Pending` 时，当前任务暂时挂起；当事件就绪后，再通过 executor、reactor 或 waker 重新调度，并通过再次 `poll()` 推进状态机。

因此，可以把“栈”看成一种调度资源：如果一个执行流暂停时仍然占有自己的栈，那么它更接近线程或有栈协程；如果一个执行流暂停时不占有独立栈，而是只保存必要状态机信息，那么它更接近无栈协程。

沿着这个思路，还可以进一步把进程、线程和协程统一到“执行流上下文切换”框架下理解：

| 执行流类型          | 暂停时是否占有独立栈 | 是否切换地址空间 | 主要保存内容                             |
| ------------------- | -------------------- | ---------------- | ---------------------------------------- |
| 无栈协程 / Future   | 否                   | 否               | Future 状态机、await 点、必要局部变量    |
| 绿色线程 / 有栈协程 | 是                   | 否               | 用户态栈、寄存器上下文                   |
| 操作系统线程        | 是                   | 否               | 内核调度上下文、线程栈、寄存器           |
| 进程                | 是                   | 是               | 地址空间、页表、寄存器、栈和其他进程资源 |

从这个角度看，协程、线程和进程并不是完全割裂的概念，而是执行状态保存粒度和资源占有程度不同的执行流机制。无栈协程保存的信息最少，切换成本最低；绿色线程和操作系统线程需要保存栈和上下文；进程还需要切换地址空间，因此切换成本更高。

这也为后续实验提供了一个方向：可以在当前绿色线程优先级调度的基础上，继续抽象统一的执行流描述结构，把“是否拥有独立栈”“是否需要地址空间切换”“是否具有优先级”“是否可抢占”等属性纳入调度器设计中，从而进一步探索协程、线程和进程调度的一体化模型。

---

## 12. 测试验证

为了验证优先级机制正确性，实验设计了三个单元测试。

### 12.1 高优先级任务优先运行

测试名称：

```text
high_priority_ready_thread_runs_first
```

测试逻辑：

```text
创建 low(priority=1)
创建 high(priority=10)
创建 mid(priority=5)
调用 pick_next(None)
期望返回 high
```

验证点：Ready 集合中 priority 最大的线程会被优先选择。

### 12.2 同优先级任务保持 round-robin

测试名称：

```text
same_priority_uses_round_robin_order
```

测试逻辑：

```text
创建 a(priority=7)
创建 b(priority=7)
创建 c(priority=7)
current = a 时，下一次选择 b
current = b 时，下一次选择 c
current = c 时，下一次选择 a
```

验证点：同优先级线程仍保留 Stage1 的轮转公平性。

### 12.3 yield 后重新参与优先级调度

测试名称：

```text
yielded_thread_rejoins_priority_scheduling
```

测试逻辑：

```text
创建 high(priority=10)
创建 low(priority=1)
high Running -> Ready
pick_next(Some(high)) == low
low Running -> Ready
pick_next(Some(low)) == high
```

验证点：yield 后线程重新进入 Ready 状态，并继续按照自身优先级参与后续调度。

---

## 

绿色线程优先级扩展证明了用户态运行时可以在不依赖操作系统内核调度器的情况下实现自定义调度策略。

实现效果如下：

1. `GreenThread` 支持优先级字段；
2. `spawn_with_priority()` 可以显式创建不同优先级线程；
3. `pick_next()` 实现“高优先级优先，同优先级轮转”；
4. 动态跟踪日志能够显示 priority 对调度结果的影响；
5. 三个测试分别验证了高优先级优先、同优先级轮转和 yield 后重新参与调度。

当前限制：

1. 线程不主动 yield 时无法被抢占；
2. 高优先级任务持续就绪时，低优先级任务可能被延后；
3. 没有 aging 老化机制；
4. 没有时间片；
5. 不是完整抢占式实时调度器。

##  扩展思考：从全局就绪队列角度理解执行流调度

本实验还可以从“就绪队列”的角度进一步理解执行流调度。假设每个 worker 或执行单元都有自己的本地队列，就类似多个窗口各自排队：某个窗口前面的任务处理较慢时，该队列后面的任务会被长期阻塞；而其他窗口可能已经空闲。此时如果允许任务换队，又会引入迁移成本、顺序维护和公平性问题。

一种更简单的设计是在入口处维护一个全局就绪队列。所有新任务先进入全局队列，哪个 worker 空闲，就从全局队列取出下一个任务执行。这样任务按照进入顺序统一排队，不需要考虑任务在多个本地队列之间迁移的问题，也能在一定程度上提高负载均衡性。

这个思想和本实验中的 executor 模型直接相关。在 stack-less coroutine 中，executor 维护一个 ready queue，Future 返回 Pending 后可以重新入队；在 Futures Explained 200 Lines 中，Waker 会把等待事件完成的 task 重新放回 ready queue；在 Tokio Future 中，runtime 内部也会维护可运行任务队列，只是具体实现更加复杂。绿色线程 Stage2 的 `pick_next()` 虽然没有显式使用全局队列，但本质上也是在所有 Ready 线程集合中选择下一个可运行执行流。

因此，可以把执行流调度理解为：任务创建后进入就绪集合，运行时从就绪集合中选择任务运行；任务遇到等待条件时挂起，条件满足后再回到就绪集合。全局就绪队列提供了一种简单、公平、易理解的调度方式，而优先级调度则是在这个基础上进一步引入任务重要性，使调度器不仅考虑“谁先来”，还考虑“谁更重要”。

## 14. 实验总总结

本实验按照两个子任务完成。

第一，基于动态跟踪分析了四种执行流机制。绿色线程使用独立用户态栈和寄存器上下文保存执行现场；100 行无栈协程使用 Future 状态机和 executor 重新 poll；Futures Explained 200 Lines 在此基础上引入 reactor 与 waker，解释了外部事件如何唤醒任务；Tokio Future 则体现了生产级异步运行时对同一抽象的封装。

第二，在绿色线程中扩展了优先级调度机制。通过新增 priority 字段、spawn_with_priority 接口和优先级优先的 pick_next 逻辑，实现了用户态调度器对任务执行顺序的控制。测试结果证明：高优先级任务能够优先运行，同优先级任务保持 round-robin，yield 后任务仍能按照优先级重新参与调度。

最终可以得到结论：

```text
执行流机制的核心问题不是“是否能够并发”，而是：
状态保存在哪里；
由谁负责调度；
如何挂起；
如何恢复；
调度策略能否扩展。
```

绿色线程更适合观察底层执行现场切换；无栈协程更适合理解 Rust Future 状态机；Futures Explained 展示了 executor、reactor 和 waker 的协作；Tokio Future 体现了生产级 runtime 的封装能力。优先级扩展进一步说明，用户态运行时不仅能管理执行流，还可以实现类似操作系统调度器的策略控制。

---

## 15. 后续改进方向

1. 为绿色线程加入 aging 机制，避免低优先级任务长期饥饿；
2. 尝试加入时间片或信号驱动的抢占式调度；
3. 为 stack-less coroutine 增加更明确的 ready/pending 队列；
4. 将 Futures Explained 的 timer reactor 替换为更接近真实 I/O 多路复用的事件源；
5. 对 Tokio current-thread 和 multi-thread runtime 做对比，观察多 worker 场景下的任务调度差异；
6. 将四种机制的状态变迁画成统一流程图，便于后续汇报展示。
