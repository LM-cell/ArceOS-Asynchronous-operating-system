use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use task3_exec_flow_memory::metrics::ensure_parent_dir;
use task3_exec_flow_memory::{
    run_async_future, run_green_thread, run_os_thread, ExecutionModel, ExperimentRecord,
    LabeledSample, RunConfig,
};

/// Compare stack and memory usage across OS threads, stackful green threads,
/// and stackless async Futures.
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    /// Models to run, separated by commas: os-thread,green-thread,async-future.
    #[arg(long, value_enum, value_delimiter = ',', default_value = "os-thread,green-thread,async-future")]
    models: Vec<ExecutionModel>,

    /// Task counts to run, separated by commas, for example 1000,10000,50000.
    #[arg(long, value_delimiter = ',', default_value = "1000")]
    tasks: Vec<usize>,

    /// Sleep duration per task. This creates a suspension/switch point.
    #[arg(long, default_value_t = 10)]
    sleep_ms: u64,

    /// Configured OS-thread stack size, in KiB.
    #[arg(long, default_value_t = 64)]
    os_stack_kib: usize,

    /// Configured may coroutine stack size, in KiB.
    #[arg(long, default_value_t = 64)]
    green_stack_kib: usize,

    /// Stack bytes touched by each task, in KiB.
    #[arg(long, default_value_t = 8)]
    touch_stack_kib: usize,

    /// Estimated kernel stack size per kernel thread, in KiB.
    #[arg(long, default_value_t = 16)]
    kernel_stack_kib: usize,

    /// Tokio/may worker count. Defaults to available parallelism.
    #[arg(long)]
    runtime_workers: Option<usize>,

    /// Sampling interval in milliseconds.
    #[arg(long, default_value_t = 5)]
    sample_interval_ms: u64,

    /// Summary CSV output path.
    #[arg(long, default_value = "data/results.csv")]
    csv: PathBuf,

    /// Summary JSON output path.
    #[arg(long, default_value = "data/results.json")]
    json: PathBuf,

    /// Optional time-series sample CSV output path.
    #[arg(long)]
    samples_csv: Option<PathBuf>,

    /// Append CSV rows. Useful when one script launches one process per data point.
    #[arg(long, default_value_t = false)]
    append_csv: bool,
}

#[derive(Debug, Serialize)]
struct JsonOutput {
    results: Vec<ExperimentRecord>,
    samples: Vec<LabeledSample>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    validate_cli(&cli)?;

    let worker_count = cli
        .runtime_workers
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));

    let mut records = Vec::new();
    let mut labeled_samples = Vec::new();

    for &task_count in &cli.tasks {
        for &model in &cli.models {
            let config = RunConfig {
                task_count,
                sleep_ms: cli.sleep_ms,
                os_stack_bytes: cli.os_stack_kib * 1024,
                green_stack_bytes: cli.green_stack_kib * 1024,
                touch_stack_bytes: cli.touch_stack_kib * 1024,
                kernel_stack_bytes: cli.kernel_stack_kib * 1024,
                runtime_workers: worker_count,
                sample_interval_ms: cli.sample_interval_ms,
            };

            eprintln!(
                "running model={} tasks={} sleep_ms={} workers={}",
                model, task_count, cli.sleep_ms, worker_count
            );

            let (record, samples) = match model {
                ExecutionModel::OsThread => run_os_thread(&config),
                ExecutionModel::GreenThread => run_green_thread(&config),
                ExecutionModel::AsyncFuture => run_async_future(&config),
            }
            .with_context(|| format!("model={model} task_count={task_count} failed"))?;

            labeled_samples.extend(
                samples
                    .iter()
                    .map(|sample| sample.labeled(model.as_str(), task_count)),
            );
            records.push(record);
        }
    }

    write_summary_csv(&cli.csv, &records, cli.append_csv)?;
    write_json(
        &cli.json,
        &JsonOutput {
            results: records,
            samples: labeled_samples.clone(),
        },
    )?;

    if let Some(path) = &cli.samples_csv {
        write_samples_csv(path, &labeled_samples, cli.append_csv)?;
    }

    Ok(())
}

fn validate_cli(cli: &Cli) -> Result<()> {
    anyhow::ensure!(!cli.models.is_empty(), "--models cannot be empty");
    anyhow::ensure!(!cli.tasks.is_empty(), "--tasks cannot be empty");
    anyhow::ensure!(cli.os_stack_kib > 0, "--os-stack-kib must be positive");
    anyhow::ensure!(cli.green_stack_kib > 0, "--green-stack-kib must be positive");
    anyhow::ensure!(
        cli.touch_stack_kib < cli.os_stack_kib.min(cli.green_stack_kib),
        "--touch-stack-kib must be smaller than both --os-stack-kib and --green-stack-kib"
    );
    anyhow::ensure!(cli.kernel_stack_kib > 0, "--kernel-stack-kib must be positive");
    Ok(())
}

fn write_summary_csv(path: &Path, records: &[ExperimentRecord], append: bool) -> Result<()> {
    ensure_parent_dir(path)?;
    let has_existing_data = append && path.exists() && fs::metadata(path)?.len() > 0;
    let file = OpenOptions::new()
        .create(true)
        .append(append)
        .write(true)
        .truncate(!append)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    let mut writer = csv::WriterBuilder::new()
        .has_headers(!has_existing_data)
        .from_writer(file);

    for record in records {
        writer.serialize(record)?;
    }

    writer.flush()?;
    Ok(())
}

fn write_samples_csv(path: &Path, samples: &[LabeledSample], append: bool) -> Result<()> {
    ensure_parent_dir(path)?;
    let has_existing_data = append && path.exists() && fs::metadata(path)?.len() > 0;
    let file = OpenOptions::new()
        .create(true)
        .append(append)
        .write(true)
        .truncate(!append)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    let mut writer = csv::WriterBuilder::new()
        .has_headers(!has_existing_data)
        .from_writer(file);

    for sample in samples {
        writer.serialize(sample)?;
    }

    writer.flush()?;
    Ok(())
}

fn write_json(path: &Path, output: &JsonOutput) -> Result<()> {
    ensure_parent_dir(path)?;
    let json = serde_json::to_string_pretty(output)?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
