# ArceOS-Asynchronous-operating-system
基于 Rust 语言的异步机制 future 对设备驱动、调度器、系统调用和 IPC 等内核模块进行异步改造核心问题。

## 任务一交付（进程、线程、协程）

本仓库新增了 3 个用户态 Rust 爬虫程序，用于实践并对比不同并发模型：

- `process_crawler`：基于**进程**（每个请求一个子进程）
- `thread_crawler`：基于**线程**（每个请求一个线程）
- `coroutine_crawler`：基于**协程/Future**（Tokio 异步任务）

### 运行方式

```bash
cargo run --bin process_crawler -- --requests 9 --mock
cargo run --bin thread_crawler -- --requests 9 --mock
cargo run --bin coroutine_crawler -- --requests 9 --mock
```

说明：

- `--mock`：使用内置模拟 URL（离线可复现）
- 不加 `--mock`：访问默认真实 URL 列表

输出指标包含：

- 延时分布：`p50 / p95 / max`
- 吞吐率：`throughput_rps`
- 内存开销：`memory_kib`（读取 `/proc/self/status` 中 `VmRSS`）

## 性能特征对比结论（在线开发文档）

在同等请求数与目标 URL 集下，三种模型通常呈现如下特征：

1. **进程模型**
   - 优点：隔离性最好
   - 缺点：进程创建和上下文切换成本高，尾延迟（p95/max）通常较高，内存开销最大
2. **线程模型**
   - 优点：实现直观，吞吐率通常高于进程模型
   - 缺点：线程栈占用明显，请求数增大时内存压力上升
3. **协程模型**
   - 优点：任务切换轻量，资源利用率高，通常可获得更稳定的延时分布和更低内存开销
   - 缺点：需要异步生态与运行时支持，调试复杂度相对更高

> 建议：将该对比方法用于后续内核模块（驱动、调度器、系统调用、IPC）异步化验证，统一使用“延时分布 + 吞吐率 + 内存”三维指标评估收益与代价。
