# Task2 用户态线程与协程实验总结报告

##  实验路线

本实验围绕“用户态线程与协程”的执行流机制展开，分为两个子任务：

1. **四种执行流机制的动态跟踪分析对比与总结**  
   通过运行时打点和日志，分析绿色线程、Stack-less Coroutine、Futures Explained 200 Lines 和 Tokio Future 的状态变迁过程。
2. **优先级扩展与测试验证**  

四种执行流都可以抽象为：

```text
Created / Ready -> Running -> Suspended / Pending / Ready -> Running -> Finished
```

都是：**创建、运行、挂起、恢复、结束**

区别主要在于：**状态保存在哪里、谁负责调度、如何挂起、如何恢复、是否占有独立栈。**

---

# 子任务一：四种执行流机制动态跟踪分析

## 1. 绿色线程：Stackful Green Thread

绿色线程属于有栈执行流。每个绿色线程拥有独立用户态栈，运行时通过 `gt_switch` 保存和恢复寄存器上下文。

状态变迁：

```text
Ready -> Running -> Ready
Ready -> Running -> Finished
```

典型流程：

```text
create task -> Ready -> dispatch -> Running
Running -> yield_now() -> Ready
Running -> return -> Finished
```

观察结论：

1. 绿色线程通过用户态栈和寄存器保存执行现场。
2. `yield_now()` 会让当前线程从 `Running` 回到 `Ready`。
3. 恢复时不是重新执行函数，而是从上次切走的位置继续执行。
4. 原始调度策略是 round-robin，属于协作式调度。

---

## 2. Stack-less Coroutine under 100 LoC

Stack-less Coroutine 属于无栈执行流，不分配独立用户态栈。每个 coroutine 被保存为：

```rust
Pin<Box<dyn Future<Output = ()>>>
```

状态变迁：

```text
Created -> Ready Queue -> Running -> Pending -> Ready Queue -> Running -> Finished
```

观察结论：

1. 无栈协程的状态由 Future 状态机保存。
2. `.await` 是挂起点。
3. `Poll::Pending` 表示当前任务暂时不能继续。
4. executor 再次 `poll()` 时，状态机从上次 await 后继续推进。

---

## 3. Futures Explained 200 Lines

Futures Explained 200 Lines 展示的是更完整的异步运行时模型：

```text
Spawner -> Executor -> Reactor -> Waker
```

状态变迁：

```text
Spawned / Ready -> Running -> Pending -> Waiting Event -> Ready -> Running -> Finished
```

观察结论：

1. Future 必须由 executor `poll()` 推进。
2. `Pending` 不是阻塞线程，而是表示任务等待外部事件。
3. Reactor 负责等待事件，例如 timer 或 I/O。
4. Waker 负责把事件完成的任务重新放回 ready queue。

---

## 4. Tokio Future

Tokio Future 是生产级异步运行时中的 Future 状态机模型。实验中使用 Tokio current-thread runtime 和手写 `YieldOnce` Future 观察 `poll -> Pending -> wake -> poll -> Ready` 过程。

状态变迁：

```text
Spawned / Ready -> Running -> Pending -> Ready Queue -> Running -> Finished
```

观察结论：

1. Tokio task 内部保存 async Future 状态机。
2. `.await` 处如果返回 `Pending`，任务让出执行权。
3. Waker 将任务重新放回 Tokio ready queue。
4. Tokio 与 Futures Explained 的抽象一致，但封装更完整。

---

## 四种机制对比总结

| 模型 | 是否独立栈 | 状态保存位置 | 挂起方式 | 恢复方式 | 调度者 |
|---|---|---|---|---|---|
| 绿色线程 | 是 | 用户态栈 + 寄存器上下文 | `yield_now()` | `gt_switch` 恢复上下文 | 自写 Runtime |
| Stack-less Coroutine | 否 | Future 状态机 | `Poll::Pending` | executor 再次 `poll` | 自写 executor |
| Futures Explained | 否 | Future + Reactor + Waker | `Pending` + 等待事件 | Waker 唤醒后再 poll | executor + reactor |
| Tokio Future | 否 | Tokio Task / Future 状态机 | `.await` 返回 `Pending` | Tokio runtime 再 poll | Tokio runtime |

结论：

```text
绿色线程：保存栈和寄存器，靠上下文切换恢复。
无栈协程 / Future：保存状态机，靠 poll / Pending / wake / poll 推进。
```



---

## 思考问题：从栈资源角度理解执行流

可以进一步得到一个统一视角：**执行流切换的本质是保存当前执行状态，并在之后恢复另一个执行状态。**

绿色线程暂停时仍然占有独立用户态栈，因此恢复时需要恢复栈和寄存器上下文。无栈协程、Futures Explained 和 Tokio Future 暂停时不占独立栈，而是把执行进度保存到 Future 状态机中，恢复时通过再次 `poll()` 推进。

| 执行流类型 | 暂停时是否占有独立栈 | 是否切换地址空间 | 主要保存内容 |
|---|---|---|---|
| 无栈协程 / Future | 否 | 否 | Future 状态机、await 点、必要局部变量 |
| 绿色线程 / 有栈协程 | 是 | 否 | 用户态栈、寄存器上下文 |
| 操作系统线程 | 是 | 否 | 线程栈、寄存器、调度上下文 |
| 进程 | 是 | 是 | 地址空间、页表、栈、寄存器和进程资源 |

所以，协程、线程和进程不是完全割裂的概念，而是执行状态保存方式和资源占有程度不同的执行流机制。

---

## 思考问题：从全局就绪队列角度理解调度

调度器可以理解为从“可运行任务列表”中选择任务运行。  
如果每个 worker 都维护自己的本地队列，可能出现某个队列很慢、另一个队列空闲的问题。使用全局 ready queue 可以让所有任务先统一排队，哪个 worker 空闲就取下一个任务。

在本实验中的对应关系：

| 实验对象 | Ready Queue 对应关系 |
|---|---|
| 绿色线程 | Runtime 在所有 `Ready` 线程列表中选择 |
| Stack-less Coroutine | Executor 的 `VecDeque` |
| Futures Explained | Executor ready queue + Waker requeue |
| Tokio Future | Tokio runtime 内部 ready queue |

**优先级调度**可以理解为在 ready queue 基础上进一步加入“任务重要性”判断：不仅看谁先来，也看谁更重要。

---

## 实验总结

本实验完成了子任务一工作。

通过动态跟踪分析了四种执行流机制。绿色线程使用用户态栈和寄存器保存执行现场；无栈协程使用 Future 状态机保存执行进度；Futures Explained 引入 executor、reactor 和 waker；Tokio Future 是生产级 runtime 对同一抽象的封装。

最终结论：

```text
执行流机制的核心问题不是“是否能够并发”，而是：
状态保存在哪里；
由谁负责调度；
如何挂起；
如何恢复；
调度策略能否扩展。
```

绿色线程更适合观察底层执行现场切换；无栈协程更适合理解 Future 状态机；Futures Explained 展示了 executor、reactor 和 waker 的协作关系；Tokio Future 展示了生产级异步运行时的封装能力。优先级扩展进一步说明，用户态运行时也可以实现类似操作系统调度器的策略控制。

## **下周安排**

1.在绿色线程轮询协作式调度基础上，去扩展优先级调度，测试验证

2.开始学习任务三
