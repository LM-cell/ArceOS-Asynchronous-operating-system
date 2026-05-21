# ArceOS Asynchronous Operating System Practices

本仓库按实践任务拆分为独立目录，每个任务都保留自己的代码、测试脚本、数据和实验报告。

```text
.
├── task1/
│   ├── data/
│   ├── reports/
│   ├── scripts/
│   ├── src/
│   └── tests/
├── task2/
│   ├── data/
│   ├── reports/
│   ├── scripts/
│   ├── src/
│   └── tests/
└── task3/
    ├── data/
    ├── reports/
    ├── scripts/
    ├── src/
    └── tests/
```

## 任务说明

- `task1`：用户态并发爬虫实验，包含进程、线程、协程三种实现和性能对比报告。
- `task2`：预留任务目录，结构与 `task1` 保持一致。
- `task3`：预留任务目录，结构与 `task1` 保持一致。

每个任务目录内部约定：

- `src/`：源代码。
- `tests/`：测试代码或测试样例。
- `scripts/`：运行、测试、基准测试脚本。
- `data/`：输入数据。
- `reports/`：实验报告、性能记录和结果分析。
