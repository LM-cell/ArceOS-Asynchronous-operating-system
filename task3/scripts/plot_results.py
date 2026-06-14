#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""根据 data/results.csv 生成中文对比图。

用法：
    python3 scripts/plot_results.py data/results.csv reports/figures
"""

from __future__ import annotations

import csv
import os
import sys
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.ticker import FuncFormatter, MaxNLocator


MODEL_ORDER = ["os_thread", "green_thread", "async_future"]
MODEL_LABELS = {
    "os_thread": "OS thread",
    "green_thread": "stackful green thread",
    "async_future": "stackless async Future",
}
MODEL_SHORT_LABELS = {
    "os_thread": "OS",
    "green_thread": "Green",
    "async_future": "Async",
}
MODEL_COLORS = {
    "os_thread": "#4C78A8",
    "green_thread": "#F58518",
    "async_future": "#54A24B",
}

STATE_COLORS = {
    "peak_running_kernel_threads": "#4C78A8",
    "peak_sleeping_kernel_threads": "#72B7B2",
    "peak_uninterruptible_kernel_threads": "#E45756",
}
STATE_LABELS = {
    "peak_running_kernel_threads": "Running (R)",
    "peak_sleeping_kernel_threads": "Sleeping (S)",
    "peak_uninterruptible_kernel_threads": "Uninterruptible (D)",
}

STACK_COMPONENTS = [
    ("user_stack_reserved_bytes_per_task", "user stack", "#4C78A8"),
    ("kernel_stack_reserved_bytes_per_task", "kernel stack", "#F58518"),
]

CJK_FONT_NAMES = [
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Source Han Sans SC",
    "WenQuanYi Micro Hei",
    "Microsoft YaHei",
    "SimHei",
    "Arial Unicode MS",
]

CJK_FONT_FILES = [
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
]

CJK_FONT_PROP = None


def configure_fonts() -> bool:
    global CJK_FONT_PROP

    selected_font = os.environ.get("MPL_CJK_FONT")
    available_fonts = {font.name for font in font_manager.fontManager.ttflist}
    selected_path = None

    if not selected_font:
        selected_font = next((font for font in CJK_FONT_NAMES if font in available_fonts), None)

    if not selected_font:
        for font_path in CJK_FONT_FILES:
            path = Path(font_path)
            if path.exists():
                font_manager.fontManager.addfont(str(path))
                selected_path = str(path)
                selected_font = font_manager.FontProperties(fname=selected_path).get_name()
                break

    if not selected_font:
        print(
            "未找到可用的 CJK 中文字体，无法正确渲染中文标题和注释。\n"
            "请先运行：bash scripts/setup_plot_fonts.sh\n"
            "或者手动安装：sudo apt-get install -y fonts-noto-cjk fonts-wqy-microhei\n"
            "安装后重新执行：python3 scripts/plot_results.py data/results.csv reports/figures",
            file=sys.stderr,
        )
        return False

    plt.rcParams["font.family"] = "sans-serif"
    plt.rcParams["font.sans-serif"] = [selected_font, "DejaVu Sans"]
    plt.rcParams["axes.unicode_minus"] = False
    CJK_FONT_PROP = (
        font_manager.FontProperties(fname=selected_path)
        if selected_path
        else font_manager.FontProperties(family=selected_font)
    )
    print(f"Using CJK font: {selected_font}", file=sys.stderr)
    return True


def cjk_font() -> font_manager.FontProperties | None:
    return CJK_FONT_PROP


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))

    enrich_derived_metrics(rows)
    return rows


def enrich_derived_metrics(rows: list[dict[str, str]]) -> None:
    for row in rows:
        task_count = float(row.get("task_count", "0") or 0)
        peak_threads = float(row.get("peak_kernel_threads", "0") or 0)
        user_reserved = float(row.get("estimated_user_stack_reserved_bytes", "0") or 0)
        kernel_reserved = float(row.get("estimated_kernel_stack_reserved_bytes", "0") or 0)

        if task_count <= 0:
            continue

        row.setdefault("kernel_threads_per_task", str(peak_threads / task_count))
        row.setdefault("user_stack_reserved_bytes_per_task", str(user_reserved / task_count))
        row.setdefault("kernel_stack_reserved_bytes_per_task", str(kernel_reserved / task_count))
        row.setdefault(
            "total_stack_reserved_bytes_per_task",
            str((user_reserved + kernel_reserved) / task_count),
        )
        row.setdefault("kernel_stack_slots_per_1000_tasks", str(peak_threads * 1000 / task_count))


def task_label(task_count: int) -> str:
    if task_count >= 1000 and task_count % 1000 == 0:
        return f"{task_count // 1000}k"
    return f"{task_count:,}"


def format_value(value: float) -> str:
    if value == 0:
        return "0"
    abs_value = abs(value)
    if abs_value < 0.001:
        return f"{value:.6f}"
    if abs_value < 0.01:
        return f"{value:.4f}"
    if abs_value < 1:
        return f"{value:.3f}"
    if abs_value < 10:
        return f"{value:.2f}"
    if abs_value < 100:
        return f"{value:.1f}"
    return f"{value:.0f}"


def y_tick(value: float, _position: int) -> str:
    return format_value(value)


def metric_table(
    rows: list[dict[str, str]],
    metric: str,
    scale: float,
) -> tuple[list[int], list[str], dict[tuple[int, str], float]]:
    task_counts = sorted({int(row["task_count"]) for row in rows})
    present_models = {row["model"] for row in rows}
    models = [model for model in MODEL_ORDER if model in present_models]
    models.extend(sorted(present_models - set(models)))

    values: dict[tuple[int, str], float] = {}
    for row in rows:
        if metric in row and row[metric] != "":
            values[(int(row["task_count"]), row["model"])] = float(row[metric]) / scale

    return task_counts, models, values


def plot_metric(
    rows: list[dict[str, str]],
    metric: str,
    title: str,
    ylabel: str,
    out: Path,
    scale: float = 1024 * 1024,
    subtitle: str | None = None,
) -> None:
    task_counts, models, values = metric_table(rows, metric, scale)
    if not task_counts or not models:
        return

    group_count = len(task_counts)
    bar_count = len(models)
    width = min(0.22, 0.78 / max(bar_count, 1))
    x_positions = list(range(group_count))
    positive_values = [value for value in values.values() if value > 0]
    max_value = max(positive_values, default=1.0)
    y_top = max_value * 1.28
    zero_label_y = max_value * 0.025
    missing_label_y = max_value * 0.08

    fig_width = max(9.8, group_count * 1.7)
    fig, ax = plt.subplots(figsize=(fig_width, 6.4))

    for model_index, model in enumerate(models):
        offset = (model_index - (bar_count - 1) / 2) * width
        xs = [x + offset for x in x_positions]
        ys = [values.get((task_count, model)) for task_count in task_counts]

        bar_xs = [x for x, y_value in zip(xs, ys) if y_value is not None]
        bar_ys = [y_value for y_value in ys if y_value is not None]
        bars = ax.bar(
            bar_xs,
            bar_ys,
            width=width * 0.92,
            label=MODEL_LABELS.get(model, model),
            color=MODEL_COLORS.get(model),
            edgecolor="white",
            linewidth=0.8,
        )

        for bar, value in zip(bars, bar_ys):
            x = bar.get_x() + bar.get_width() / 2
            if value == 0:
                ax.text(
                    x,
                    zero_label_y,
                    "0",
                    ha="center",
                    va="bottom",
                    fontsize=8,
                    color="#333333",
                )
            else:
                ax.annotate(
                    format_value(value),
                    xy=(x, value),
                    xytext=(0, 4),
                    textcoords="offset points",
                    ha="center",
                    va="bottom",
                    fontsize=8,
                )

        for x, y_value in zip(xs, ys):
            if y_value is None:
                ax.text(
                    x,
                    missing_label_y,
                    "无数据",
                    ha="center",
                    va="bottom",
                    fontsize=8,
                    color="#888888",
                    rotation=90,
                    fontproperties=cjk_font(),
                )

    ax.set_title(title, fontsize=15, pad=14, fontproperties=cjk_font())
    ax.set_xlabel("task count", fontsize=12)
    ax.set_ylabel(ylabel, fontsize=12)
    ax.set_xticks(x_positions)
    ax.set_xticklabels([task_label(task_count) for task_count in task_counts], fontsize=11)
    ax.set_ylim(0, y_top)
    ax.yaxis.set_major_locator(MaxNLocator(nbins=7))
    ax.yaxis.set_major_formatter(FuncFormatter(y_tick))
    ax.grid(axis="y", linestyle="--", alpha=0.35)
    ax.set_axisbelow(True)
    ax.legend(ncol=min(3, len(models)), loc="upper center", bbox_to_anchor=(0.5, -0.12))

    if subtitle is None:
        subtitle = "柱顶数字为对应 metric value；“无数据”表示该 model 在该 task count 下失败或未运行。"
    fig.text(
        0.5,
        0.01,
        subtitle,
        ha="center",
        fontsize=9,
        color="#555555",
        fontproperties=cjk_font(),
    )
    fig.tight_layout(rect=(0, 0.05, 1, 1))
    fig.savefig(out, dpi=220)
    plt.close(fig)


def plot_thread_states(rows: list[dict[str, str]], out: Path) -> None:
    required = list(STATE_LABELS)
    if not rows or any(metric not in rows[0] for metric in required):
        return

    task_counts = sorted({int(row["task_count"]) for row in rows})
    present_models = {row["model"] for row in rows}
    models = [model for model in MODEL_ORDER if model in present_models]
    models.extend(sorted(present_models - set(models)))

    values = {
        (int(row["task_count"]), row["model"], metric): float(row[metric])
        for row in rows
        for metric in required
    }

    group_count = len(task_counts)
    bar_count = len(models)
    width = min(0.22, 0.78 / max(bar_count, 1))
    x_positions = list(range(group_count))
    max_total = max(
        (
            sum(values.get((task_count, model, metric), 0.0) for metric in required)
            for task_count in task_counts
            for model in models
        ),
        default=1.0,
    )

    fig_width = max(9.8, group_count * 1.7)
    fig, ax = plt.subplots(figsize=(fig_width, 6.4))

    for model_index, model in enumerate(models):
        offset = (model_index - (bar_count - 1) / 2) * width
        xs = [x + offset for x in x_positions]
        bottoms = [0.0 for _ in task_counts]
        has_model_point = [
            any(row["model"] == model and int(row["task_count"]) == task_count for row in rows)
            for task_count in task_counts
        ]

        for metric in required:
            ys = [values.get((task_count, model, metric), 0.0) for task_count in task_counts]
            ax.bar(
                xs,
                ys,
                width=width * 0.92,
                bottom=bottoms,
                color=STATE_COLORS[metric],
                edgecolor="white",
                linewidth=0.5,
                label=STATE_LABELS[metric] if model_index == 0 else None,
                alpha=0.88,
            )
            bottoms = [bottom + y_value for bottom, y_value in zip(bottoms, ys)]

        for x, total, present in zip(xs, bottoms, has_model_point):
            if present:
                ax.annotate(
                    f"{total:.0f}",
                    xy=(x, total),
                    xytext=(0, 4),
                    textcoords="offset points",
                    ha="center",
                    va="bottom",
                    fontsize=8,
                )
            else:
                ax.text(
                    x,
                    max_total * 0.08,
                    "无数据",
                    ha="center",
                    va="bottom",
                    fontsize=8,
                    color="#888888",
                    rotation=90,
                    fontproperties=cjk_font(),
                )

    ax.set_title("峰值 kernel thread state 分布", fontsize=15, pad=14, fontproperties=cjk_font())
    ax.set_xlabel("task count", fontsize=12)
    ax.set_ylabel("thread count", fontsize=12)
    ax.set_xticks(x_positions)
    ax.set_xticklabels([task_label(task_count) for task_count in task_counts], fontsize=11)
    ax.set_ylim(0, max_total * 1.28)
    ax.yaxis.set_major_locator(MaxNLocator(nbins=7, integer=True))
    ax.grid(axis="y", linestyle="--", alpha=0.35)
    ax.set_axisbelow(True)

    ax.legend(ncol=3, loc="upper center", bbox_to_anchor=(0.5, -0.12))
    fig.text(
        0.5,
        0.01,
        "柱子按 thread state 堆叠；同一 task count 下，不同 model 按横向偏移排列；柱顶数字为线程数总和。",
        ha="center",
        fontsize=8,
        color="#555555",
        fontproperties=cjk_font(),
    )
    fig.tight_layout(rect=(0, 0.06, 1, 1))
    fig.savefig(out, dpi=220)
    plt.close(fig)


def plot_stack_breakdown_per_task(rows: list[dict[str, str]], out: Path) -> None:
    rows = [
        row
        for row in rows
        if all(component in row for component, _label, _color in STACK_COMPONENTS)
    ]
    if not rows:
        return

    rows.sort(key=lambda row: (int(row["task_count"]), MODEL_ORDER.index(row["model"])))
    xs = list(range(len(rows)))
    labels = [
        f"{task_label(int(row['task_count']))}\n{MODEL_SHORT_LABELS.get(row['model'], row['model'])}"
        for row in rows
    ]
    bottoms = [0.0 for _ in rows]
    totals = [0.0 for _ in rows]

    fig_width = max(10.5, len(rows) * 0.72)
    fig, ax = plt.subplots(figsize=(fig_width, 6.4))

    for metric, label, color in STACK_COMPONENTS:
        values = [float(row[metric]) / 1024 for row in rows]
        ax.bar(
            xs,
            values,
            bottom=bottoms,
            color=color,
            edgecolor="white",
            linewidth=0.6,
            label=label,
            alpha=0.9,
        )
        bottoms = [bottom + value for bottom, value in zip(bottoms, values)]
        totals = [total + value for total, value in zip(totals, values)]

    max_total = max(totals, default=1.0)
    for x, total in zip(xs, totals):
        ax.annotate(
            format_value(total),
            xy=(x, total),
            xytext=(0, 4),
            textcoords="offset points",
            ha="center",
            va="bottom",
            fontsize=8,
        )

    ax.set_title(
        "单位任务 stack reserved 分解：user stack + kernel stack",
        fontsize=15,
        pad=14,
        fontproperties=cjk_font(),
    )
    ax.set_xlabel("task count / execution model", fontsize=12)
    ax.set_ylabel("KiB / task", fontsize=12)
    ax.set_xticks(xs)
    ax.set_xticklabels(labels, fontsize=9)
    ax.set_ylim(0, max_total * 1.28 if max_total > 0 else 1)
    ax.yaxis.set_major_locator(MaxNLocator(nbins=7))
    ax.yaxis.set_major_formatter(FuncFormatter(y_tick))
    ax.grid(axis="y", linestyle="--", alpha=0.35)
    ax.set_axisbelow(True)
    ax.legend(ncol=2, loc="upper center", bbox_to_anchor=(0.5, -0.12))
    fig.text(
        0.5,
        0.01,
        "单位任务 stack reserved = estimated_stack_reserved / task_count；柱顶数字为 user stack 与 kernel stack 之和。",
        ha="center",
        fontsize=9,
        color="#555555",
        fontproperties=cjk_font(),
    )
    fig.tight_layout(rect=(0, 0.06, 1, 1))
    fig.savefig(out, dpi=220)
    plt.close(fig)


def main() -> int:
    if len(sys.argv) != 3:
        print("用法: python3 scripts/plot_results.py data/results.csv reports/figures", file=sys.stderr)
        return 2

    if not configure_fonts():
        return 1

    csv_path = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    output_dir.mkdir(parents=True, exist_ok=True)

    rows = read_rows(csv_path)
    plot_metric(
        rows,
        "estimated_user_stack_reserved_bytes",
        "User stack 预留量对比",
        "MiB",
        output_dir / "user_stack_reserved.png",
    )
    plot_metric(
        rows,
        "estimated_kernel_stack_reserved_bytes",
        "Kernel stack 预留量估算对比",
        "MiB",
        output_dir / "kernel_stack_reserved.png",
    )
    plot_metric(
        rows,
        "kernel_stack_slots_per_1000_tasks",
        "每 1000 个 task 对应的 kernel thread stack slots",
        "kernel threads / 1000 tasks",
        output_dir / "kernel_stack_slots_per_1000_tasks.png",
        scale=1,
        subtitle="该指标越低，说明同样 task count 复用的 kernel thread stack 越少；无数据表示该 data point 失败或未运行。",
    )
    plot_metric(
        rows,
        "kernel_threads_per_task",
        "单位任务 kernel thread 数",
        "peak_threads / task_count",
        output_dir / "kernel_threads_per_task.png",
        scale=1,
        subtitle="单位任务 kernel thread 数 = peak_threads / task_count；OS thread 通常接近 1，async Future 通常接近 0。",
    )
    plot_metric(
        rows,
        "user_stack_reserved_bytes_per_task",
        "单位任务 user stack 预留",
        "KiB / task",
        output_dir / "user_stack_reserved_per_task.png",
        scale=1024,
        subtitle="单位任务 user stack 预留 = estimated_user_stack_reserved / task_count。",
    )
    plot_metric(
        rows,
        "future_state_bytes_per_task",
        "单位任务 Future state machine 大小",
        "KiB / task",
        output_dir / "future_state_per_task.png",
        scale=1024,
        subtitle="单位任务 Future state machine 大小 = size_of_val(async Future)；OS thread 和 green thread 不使用该状态机保存任务上下文。",
    )
    plot_metric(
        rows,
        "estimated_future_state_bytes",
        "Future state machine 总大小估算",
        "MiB",
        output_dir / "future_state_total.png",
        subtitle="Future state machine 总大小估算 = future_state_bytes_per_task * task_count。",
    )
    plot_metric(
        rows,
        "kernel_stack_reserved_bytes_per_task",
        "单位任务 kernel stack 预留估算",
        "KiB / task",
        output_dir / "kernel_stack_reserved_per_task.png",
        scale=1024,
        subtitle="单位任务 kernel stack 预留 = estimated_kernel_stack_reserved / task_count。",
    )
    plot_metric(
        rows,
        "total_stack_reserved_bytes_per_task",
        "单位任务 total stack 预留估算",
        "KiB / task",
        output_dir / "total_stack_reserved_per_task.png",
        scale=1024,
        subtitle="单位任务 total stack 预留 = (user stack 预留 + kernel stack 预留估算) / task_count。",
    )
    plot_metric(
        rows,
        "peak_rss_bytes",
        "进程 RSS peak 对比",
        "MiB",
        output_dir / "peak_rss.png",
    )
    plot_thread_states(rows, output_dir / "kernel_thread_states.png")
    plot_stack_breakdown_per_task(rows, output_dir / "stack_reserved_per_task_breakdown.png")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
