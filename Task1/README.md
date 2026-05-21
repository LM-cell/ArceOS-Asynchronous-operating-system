# ArceOS 用户态并发爬虫实践

本项目实现了 3 个用户态 Rust 爬虫，用于依据高校中文名和官网链接下载首页纯文本，并将结果保存到当前目录。输出文件名就是学校中文名称，例如 `清华大学`、`北京大学`。

石墨表格来源：<https://shimo.im/sheets/RKAWMxEO6WUG5zq8/8ihUY>

## 输入格式

默认读取当前目录的 `schools.csv`：

```csv
院校名称,官方网站
清华大学,https://www.tsinghua.edu.cn/
北京大学,https://www.pku.edu.cn/
```

如果石墨表格内容有更新，可以从石墨导出 CSV，覆盖 `schools.csv`，或通过 `--input <path>` 指定新文件。程序会自动识别常见表头，如 `院校名称`、`学校名称`、`官方网站`、`官网`、`URL`。

## 三种实现

- `process_crawler`：基于进程，每个学校一个子进程。
- `thread_crawler`：基于线程，每个学校一个线程。
- `coroutine_crawler`：基于协程/Future，每个学校一个 Tokio 异步任务。

## 运行方式

```bash
cargo run --bin process_crawler -- --input schools.csv --output-dir .
cargo run --bin thread_crawler -- --input schools.csv --output-dir .
cargo run --bin coroutine_crawler -- --input schools.csv --output-dir .
```

常用参数：

- `--input schools.csv`：学校名称和官网链接 CSV。
- `--output-dir .`：纯文本保存目录，默认当前目录。
- `--timeout-ms 15000`：单个首页请求超时。
- `--limit N`：只抓取前 N 个学校，便于快速实验。
- `--mock --requests N`：离线模拟请求，用于稳定比较调度开销。

程序输出指标：

```text
model=thread
total=10 success=10 failed=0
latency_ms p50=...
throughput_rps=...
bytes_saved=...
memory_kib=...
sample_peak_memory_kib=...
```

其中 `memory_kib` 是主进程结束统计时的工作集；进程模型还会额外输出 `sample_peak_memory_kib`，表示子进程完成单个抓取时上报的最大工作集。

## 性能特征对比

| 模型 | 延时分布 | 吞吐率 | 内存开销 | 适用场景 |
| --- | --- | --- | --- | --- |
| 进程 | p95/max 通常最高，子进程创建和进程调度会放大尾延时 | 小规模可用，请求数多时创建成本明显 | 最高，每个子进程都有独立地址空间 | 需要强隔离、失败互不影响 |
| 线程 | p50/p95 通常好于进程，但大量线程会受栈和调度影响 | 中等到较高，阻塞 I/O 编程直观 | 中等，线程栈数量随并发增长 | 并发量中等、实现简单优先 |
| 协程 | 尾延时更稳定，任务切换成本低 | 通常最高，尤其适合大量 I/O 等待 | 最低，任务比线程轻量 | 大量网络 I/O、高并发下载 |

建议先运行 `--mock` 做调度模型对比，再运行真实官网链接观察公网波动下的表现。真实爬取的延时会受到高校官网地域、CDN、TLS 握手、限流策略和网络质量影响。
