use crate::metrics::{
    avg_usize, bytes_mul, count_per_task, dominant_wait_channel, peak_u64, peak_usize, per_1000,
    per_task,
    ExperimentRecord, ProcSample, Sampler,
};
use crate::stack::touch_stack_bytes;
use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use std::fmt;
use std::mem;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

/// The three execution-flow models covered by the experiment.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, ValueEnum)]
pub enum ExecutionModel {
    /// One kernel-scheduled OS thread per task.
    #[value(name = "os-thread")]
    OsThread,
    /// One stackful may coroutine per task, scheduled in user space.
    #[value(name = "green-thread")]
    GreenThread,
    /// One Rust Future per task, scheduled by Tokio.
    #[value(name = "async-future")]
    AsyncFuture,
}

impl ExecutionModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OsThread => "os_thread",
            Self::GreenThread => "green_thread",
            Self::AsyncFuture => "async_future",
        }
    }
}

impl fmt::Display for ExecutionModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parameters for one model run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub task_count: usize,
    pub sleep_ms: u64,
    pub os_stack_bytes: usize,
    pub green_stack_bytes: usize,
    pub touch_stack_bytes: usize,
    pub kernel_stack_bytes: usize,
    pub runtime_workers: usize,
    pub sample_interval_ms: u64,
}

impl RunConfig {
    pub fn sleep_duration(&self) -> Duration {
        Duration::from_millis(self.sleep_ms)
    }

    pub fn sample_interval(&self) -> Duration {
        Duration::from_millis(self.sample_interval_ms.max(1))
    }
}

/// OS-thread model: each task is a `std::thread`.
pub fn run_os_thread(config: &RunConfig) -> Result<(ExperimentRecord, Vec<ProcSample>)> {
    let sampler = Sampler::start(config.sample_interval());
    let started_at = Instant::now();
    let start_gate = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(config.task_count);

    for id in 0..config.task_count {
        let task_start_gate = Arc::clone(&start_gate);
        let sleep = config.sleep_duration();
        let touch_bytes = config.touch_stack_bytes;
        let stack_size = config.os_stack_bytes;

        let spawn_result = thread::Builder::new()
            .name(format!("os-task-{id}"))
            .stack_size(stack_size)
            .spawn(move || {
                while !task_start_gate.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                sample_sync_task(id, sleep, touch_bytes)
            });

        match spawn_result {
            Ok(handle) => handles.push(handle),
            Err(err) => {
                let spawned = handles.len();
                start_gate.store(true, Ordering::Release);
                for handle in handles {
                    let _ = handle.join();
                }
                let _ = sampler.stop();
                return Err(anyhow!(
                    "failed to spawn OS thread {id} after spawning {spawned} threads: {err}; try a smaller --tasks value"
                ));
            }
        }
    }

    start_gate.store(true, Ordering::Release);

    let mut checksum = 0_u64;
    for handle in handles {
        checksum ^= handle.join().map_err(|_| anyhow!("OS thread panicked"))?;
    }

    let elapsed_ms = elapsed_ms(started_at);
    let samples = sampler.stop();
    let record = build_record(
        ExecutionModel::OsThread,
        config,
        &samples,
        elapsed_ms,
        config.os_stack_bytes,
        0,
        format!(
            "checksum={checksum}; user_stack_reserved = task_count * configured OS thread stack"
        ),
    );

    Ok((record, samples))
}

/// Stackful green-thread model: each task is a may coroutine.
pub fn run_green_thread(config: &RunConfig) -> Result<(ExperimentRecord, Vec<ProcSample>)> {
    may::config()
        .set_workers(config.runtime_workers)
        .set_stack_size(config.green_stack_bytes);
    may::config().set_worker_pin(false);

    let sampler = Sampler::start(config.sample_interval());
    let started_at = Instant::now();
    let start_gate = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(config.task_count);

    for id in 0..config.task_count {
        let task_start_gate = Arc::clone(&start_gate);
        let sleep = config.sleep_duration();
        let touch_bytes = config.touch_stack_bytes;
        let stack_size = config.green_stack_bytes;

        // Safety: may stack switching cannot be proven by Rust's type system.
        // The experiment keeps --touch-stack-kib below --green-stack-kib.
        let spawn_result = unsafe {
            may::coroutine::Builder::new()
                .name(format!("green-task-{id}"))
                .stack_size(stack_size)
                .spawn(move || {
                    while !task_start_gate.load(Ordering::Acquire) {
                        may::coroutine::sleep(Duration::from_millis(1));
                    }
                    sample_green_task(id, sleep, touch_bytes)
                })
        };

        match spawn_result {
            Ok(handle) => handles.push(handle),
            Err(err) => {
                let spawned = handles.len();
                start_gate.store(true, Ordering::Release);
                for handle in handles {
                    let _ = handle.join();
                }
                let _ = sampler.stop();
                return Err(anyhow!(
                    "failed to spawn green thread {id} after spawning {spawned} green threads: {err}; try a smaller --tasks value"
                ));
            }
        }
    }

    start_gate.store(true, Ordering::Release);

    let mut checksum = 0_u64;
    for handle in handles {
        checksum ^= handle
            .join()
            .map_err(|_| anyhow!("green thread panicked"))?;
    }

    let elapsed_ms = elapsed_ms(started_at);
    let samples = sampler.stop();
    let record = build_record(
        ExecutionModel::GreenThread,
        config,
        &samples,
        elapsed_ms,
        config.green_stack_bytes,
        0,
        format!(
            "checksum={checksum}; user_stack_reserved = task_count * configured may coroutine stack"
        ),
    );

    Ok((record, samples))
}

/// Stackless coroutine model: each task is a Tokio task holding a Future.
pub fn run_async_future(config: &RunConfig) -> Result<(ExperimentRecord, Vec<ProcSample>)> {
    let future_state_bytes = mem::size_of_val(&sample_async_task(
        usize::MAX,
        config.sleep_duration(),
        config.touch_stack_bytes,
    ));

    let sampler = Sampler::start(config.sample_interval());
    let started_at = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime_workers.max(1))
        .enable_time()
        .build()
        .context("failed to build tokio runtime")?;

    let checksum = runtime.block_on(async {
        let barrier = Arc::new(tokio::sync::Barrier::new(config.task_count + 1));
        let mut handles = Vec::with_capacity(config.task_count);

        for id in 0..config.task_count {
            let task_barrier = Arc::clone(&barrier);
            let sleep = config.sleep_duration();
            let touch_bytes = config.touch_stack_bytes;

            handles.push(tokio::spawn(async move {
                task_barrier.wait().await;
                sample_async_task(id, sleep, touch_bytes).await
            }));
        }

        barrier.wait().await;

        let mut checksum = 0_u64;
        for handle in handles {
            checksum ^= handle.await.context("async task panicked or was cancelled")?;
        }

        Result::<u64>::Ok(checksum)
    })?;

    drop(runtime);

    let elapsed_ms = elapsed_ms(started_at);
    let samples = sampler.stop();
    let record = build_record(
        ExecutionModel::AsyncFuture,
        config,
        &samples,
        elapsed_ms,
        0,
        future_state_bytes,
        format!(
            "checksum={checksum}; future_state_bytes is size_of_val(sample_async_task(...)); Tokio task allocation overhead is reflected in RSS"
        ),
    );

    Ok((record, samples))
}

fn sample_sync_task(id: usize, sleep: Duration, touch_bytes: usize) -> u64 {
    let stack_checksum = touch_stack_bytes(touch_bytes);
    thread::sleep(sleep);
    stack_checksum ^ id as u64
}

fn sample_green_task(id: usize, sleep: Duration, touch_bytes: usize) -> u64 {
    let stack_checksum = touch_stack_bytes(touch_bytes);
    may::coroutine::sleep(sleep);
    stack_checksum ^ id as u64
}

async fn sample_async_task(id: usize, sleep: Duration, touch_bytes: usize) -> u64 {
    // The touched stack frame is dropped before `.await`, so it is not part of
    // the suspended Future state.
    let stack_checksum = touch_stack_bytes(touch_bytes);
    tokio::time::sleep(sleep).await;
    stack_checksum ^ id as u64
}

fn build_record(
    model: ExecutionModel,
    config: &RunConfig,
    samples: &[ProcSample],
    elapsed_ms: u64,
    configured_user_stack_bytes_per_task: usize,
    future_state_bytes_per_task: usize,
    notes: String,
) -> ExperimentRecord {
    let peak_threads = peak_usize(samples, |sample| sample.kernel_threads);
    let user_stack_reserved = bytes_mul(config.task_count, configured_user_stack_bytes_per_task);
    let kernel_stack_reserved = bytes_mul(peak_threads, config.kernel_stack_bytes);
    let peak_sleeping = peak_usize(samples, |sample| sample.sleeping_kernel_threads);
    let peak_uninterruptible = peak_usize(samples, |sample| sample.uninterruptible_kernel_threads);
    let peak_blocked = samples
        .iter()
        .map(|sample| {
            sample.sleeping_kernel_threads.unwrap_or_default()
                + sample.uninterruptible_kernel_threads.unwrap_or_default()
        })
        .max()
        .unwrap_or_default();
    let (dominant_wait_channel, dominant_wait_channel_count) = dominant_wait_channel(samples);

    ExperimentRecord {
        model: model.as_str().to_string(),
        task_count: config.task_count,
        sleep_ms: config.sleep_ms,
        elapsed_ms,
        configured_user_stack_bytes_per_task: configured_user_stack_bytes_per_task as u64,
        estimated_user_stack_reserved_bytes: user_stack_reserved,
        user_stack_reserved_bytes_per_task: per_task(user_stack_reserved, config.task_count),
        future_state_bytes_per_task: future_state_bytes_per_task as u64,
        estimated_future_state_bytes: bytes_mul(config.task_count, future_state_bytes_per_task),
        peak_kernel_threads: peak_threads,
        kernel_threads_per_task: count_per_task(peak_threads, config.task_count),
        configured_kernel_stack_bytes_per_thread: config.kernel_stack_bytes as u64,
        estimated_kernel_stack_reserved_bytes: kernel_stack_reserved,
        kernel_stack_reserved_bytes_per_task: per_task(kernel_stack_reserved, config.task_count),
        total_stack_reserved_bytes_per_task: per_task(
            user_stack_reserved.saturating_add(kernel_stack_reserved),
            config.task_count,
        ),
        peak_running_kernel_threads: peak_usize(samples, |sample| sample.running_kernel_threads),
        peak_sleeping_kernel_threads: peak_sleeping,
        peak_uninterruptible_kernel_threads: peak_uninterruptible,
        peak_blocked_kernel_threads: peak_blocked,
        avg_kernel_threads: avg_usize(samples, |sample| sample.kernel_threads),
        avg_running_kernel_threads: avg_usize(samples, |sample| sample.running_kernel_threads),
        avg_sleeping_kernel_threads: avg_usize(samples, |sample| sample.sleeping_kernel_threads),
        kernel_stack_slots_per_1000_tasks: per_1000(peak_threads, config.task_count),
        dominant_wait_channel,
        dominant_wait_channel_count,
        peak_rss_bytes: peak_u64(samples, |sample| sample.vm_rss_bytes),
        peak_vm_size_bytes: peak_u64(samples, |sample| sample.vm_size_bytes),
        peak_vm_stack_bytes: peak_u64(samples, |sample| sample.vm_stack_bytes),
        proc_status_kernel_stack_bytes: peak_u64(samples, |sample| sample.kernel_stack_bytes),
        sample_count: samples.len(),
        notes,
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
