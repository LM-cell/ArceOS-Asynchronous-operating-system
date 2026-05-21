# task1 用户态并发爬虫实验

本任务实现三种用户态爬虫程序，从学校官网首页下载网页内容，提取纯文本，并把结果保存到指定输出目录。输出文件名使用学校中文名称，例如 `清华大学`、`北京大学`。

## 目录结构

```text
task1/
├── Cargo.toml
├── Cargo.lock
├── data/
│   └── schools.csv
├── reports/
│   └── experiment_report.md
├── scripts/
│   └── run_benchmark.sh
├── src/
│   ├── lib.rs
│   └── bin/
│       ├── process_crawler.rs
│       ├── thread_crawler.rs
│       └── coroutine_crawler.rs
└── tests/
```

## 输入格式

默认读取 `data/schools.csv`：

```csv
院校名称,官方网站
北京大学,http://www.pku.edu.cn/
清华大学,http://www.tsinghua.edu.cn/
```

程序会识别常见表头，例如 `院校名称`、`学校名称`、`官方网站`、`官网`、`URL`。

## 三种实现

- `process_crawler`：基于进程，每个学校一个子进程。
- `thread_crawler`：基于线程，每个学校一个线程。
- `coroutine_crawler`：基于 Tokio 协程，每个学校一个异步任务。

## 运行

```bash
cd task1
cargo run --bin process_crawler -- --input data/schools.csv --output-dir outputs/process
cargo run --bin thread_crawler -- --input data/schools.csv --output-dir outputs/thread
cargo run --bin coroutine_crawler -- --input data/schools.csv --output-dir outputs/coroutine
```

常用参数：

- `--input <path>`：学校名称和官网 CSV，默认优先读取 `data/schools.csv`。
- `--output-dir <path>`：纯文本保存目录，默认是当前目录。
- `--timeout-ms <n>`：单个首页请求超时时间，默认 `15000`。
- `--limit <n>`：只抓取前 `n` 个学校，方便快速实验。
- `--mock --requests <n>`：离线模拟请求，用于比较调度开销。

## 基准测试

真实网络测试：

```bash
cd task1
bash scripts/run_benchmark.sh
```


## 测试

```bash
cd task1
cargo test
```
