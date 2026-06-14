use serde::Serialize;
#[cfg(target_os = "linux")]
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

/// Summary row for one experiment run.
///
/// The kernel stack number is an estimate: normal user-space code cannot read
/// the live stack depth of every kernel thread. The experiment records peak
/// kernel thread count and multiplies it by the configured kernel stack size.
#[derive(Debug, Clone, Serialize)]
pub struct ExperimentRecord {
    pub model: String,
    pub task_count: usize,
    pub sleep_ms: u64,
    pub elapsed_ms: u64,
    pub configured_user_stack_bytes_per_task: u64,
    pub estimated_user_stack_reserved_bytes: u64,
    pub user_stack_reserved_bytes_per_task: f64,
    pub future_state_bytes_per_task: u64,
    pub estimated_future_state_bytes: u64,
    pub peak_kernel_threads: usize,
    pub kernel_threads_per_task: f64,
    pub configured_kernel_stack_bytes_per_thread: u64,
    pub estimated_kernel_stack_reserved_bytes: u64,
    pub kernel_stack_reserved_bytes_per_task: f64,
    pub total_stack_reserved_bytes_per_task: f64,
    pub peak_running_kernel_threads: usize,
    pub peak_sleeping_kernel_threads: usize,
    pub peak_uninterruptible_kernel_threads: usize,
    pub peak_blocked_kernel_threads: usize,
    pub avg_kernel_threads: f64,
    pub avg_running_kernel_threads: f64,
    pub avg_sleeping_kernel_threads: f64,
    pub kernel_stack_slots_per_1000_tasks: f64,
    pub dominant_wait_channel: String,
    pub dominant_wait_channel_count: usize,
    pub peak_rss_bytes: u64,
    pub peak_vm_size_bytes: u64,
    pub peak_vm_stack_bytes: u64,
    pub proc_status_kernel_stack_bytes: u64,
    pub sample_count: usize,
    pub notes: String,
}

/// A labeled time-series sample. These rows are useful for memory-over-time plots.
#[derive(Debug, Clone, Serialize)]
pub struct LabeledSample {
    pub model: String,
    pub task_count: usize,
    pub elapsed_ms: u64,
    pub vm_rss_bytes: u64,
    pub vm_size_bytes: u64,
    pub vm_stack_bytes: u64,
    pub kernel_stack_bytes: u64,
    pub kernel_threads: usize,
    pub running_kernel_threads: usize,
    pub sleeping_kernel_threads: usize,
    pub uninterruptible_kernel_threads: usize,
    pub stopped_kernel_threads: usize,
    pub zombie_kernel_threads: usize,
    pub idle_kernel_threads: usize,
    pub blocked_kernel_threads: usize,
    pub kernel_stack_slots_per_1000_tasks: f64,
    pub top_wait_channel: String,
    pub top_wait_channel_count: usize,
}

/// One `/proc/self/status` sample.
#[derive(Debug, Clone, Default)]
pub struct ProcSample {
    pub elapsed_ms: u64,
    pub vm_rss_bytes: Option<u64>,
    pub vm_size_bytes: Option<u64>,
    pub vm_stack_bytes: Option<u64>,
    pub kernel_stack_bytes: Option<u64>,
    pub kernel_threads: Option<usize>,
    pub running_kernel_threads: Option<usize>,
    pub sleeping_kernel_threads: Option<usize>,
    pub uninterruptible_kernel_threads: Option<usize>,
    pub stopped_kernel_threads: Option<usize>,
    pub zombie_kernel_threads: Option<usize>,
    pub idle_kernel_threads: Option<usize>,
    pub top_wait_channel: Option<String>,
    pub top_wait_channel_count: Option<usize>,
}

impl ProcSample {
    pub fn labeled(&self, model: &str, task_count: usize) -> LabeledSample {
        let sleeping = self.sleeping_kernel_threads.unwrap_or_default();
        let uninterruptible = self.uninterruptible_kernel_threads.unwrap_or_default();
        let kernel_threads = self.kernel_threads.unwrap_or_default();

        LabeledSample {
            model: model.to_string(),
            task_count,
            elapsed_ms: self.elapsed_ms,
            vm_rss_bytes: self.vm_rss_bytes.unwrap_or_default(),
            vm_size_bytes: self.vm_size_bytes.unwrap_or_default(),
            vm_stack_bytes: self.vm_stack_bytes.unwrap_or_default(),
            kernel_stack_bytes: self.kernel_stack_bytes.unwrap_or_default(),
            kernel_threads,
            running_kernel_threads: self.running_kernel_threads.unwrap_or_default(),
            sleeping_kernel_threads: sleeping,
            uninterruptible_kernel_threads: uninterruptible,
            stopped_kernel_threads: self.stopped_kernel_threads.unwrap_or_default(),
            zombie_kernel_threads: self.zombie_kernel_threads.unwrap_or_default(),
            idle_kernel_threads: self.idle_kernel_threads.unwrap_or_default(),
            blocked_kernel_threads: sleeping + uninterruptible,
            kernel_stack_slots_per_1000_tasks: per_1000(kernel_threads, task_count),
            top_wait_channel: self.top_wait_channel.clone().unwrap_or_default(),
            top_wait_channel_count: self.top_wait_channel_count.unwrap_or_default(),
        }
    }
}

/// Background sampler that periodically reads process status.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<Vec<ProcSample>>,
}

impl Sampler {
    pub fn start(interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let sampler_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let mut samples = Vec::new();

            while !sampler_stop.load(Ordering::Relaxed) {
                samples.push(capture_proc_sample(start));
                thread::sleep(interval);
            }

            samples.push(capture_proc_sample(start));
            samples
        });

        Self { stop, handle }
    }

    pub fn stop(self) -> Vec<ProcSample> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.join().unwrap_or_default()
    }
}

pub fn peak_u64(samples: &[ProcSample], f: impl Fn(&ProcSample) -> Option<u64>) -> u64 {
    samples.iter().filter_map(f).max().unwrap_or_default()
}

pub fn peak_usize(samples: &[ProcSample], f: impl Fn(&ProcSample) -> Option<usize>) -> usize {
    samples.iter().filter_map(f).max().unwrap_or_default()
}

pub fn avg_usize(samples: &[ProcSample], f: impl Fn(&ProcSample) -> Option<usize>) -> f64 {
    let values: Vec<usize> = samples.iter().filter_map(f).collect();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<usize>() as f64 / values.len() as f64
    }
}

pub fn dominant_wait_channel(samples: &[ProcSample]) -> (String, usize) {
    samples
        .iter()
        .filter_map(|sample| {
            let channel = sample.top_wait_channel.as_ref()?;
            let count = sample.top_wait_channel_count?;
            if channel.is_empty() || count == 0 {
                None
            } else {
                Some((channel.clone(), count))
            }
        })
        .max_by_key(|(_, count)| *count)
        .unwrap_or_else(|| (String::new(), 0))
}

pub fn per_1000(count: usize, task_count: usize) -> f64 {
    if task_count == 0 {
        0.0
    } else {
        count as f64 * 1000.0 / task_count as f64
    }
}

pub fn per_task(count: u64, task_count: usize) -> f64 {
    if task_count == 0 {
        0.0
    } else {
        count as f64 / task_count as f64
    }
}

pub fn count_per_task(count: usize, task_count: usize) -> f64 {
    if task_count == 0 {
        0.0
    } else {
        count as f64 / task_count as f64
    }
}

pub fn bytes_mul(count: usize, bytes: usize) -> u64 {
    let value = (count as u128).saturating_mul(bytes as u128);
    value.min(u64::MAX as u128) as u64
}

pub fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn capture_proc_sample(start: Instant) -> ProcSample {
    let mut sample = read_linux_proc_status().unwrap_or_default();
    sample.elapsed_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    sample
}

#[cfg(target_os = "linux")]
fn read_linux_proc_status() -> Option<ProcSample> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let mut sample = ProcSample::default();

    for line in status.lines() {
        if let Some(value) = parse_kib_field(line, "VmRSS:") {
            sample.vm_rss_bytes = Some(value);
        } else if let Some(value) = parse_kib_field(line, "VmSize:") {
            sample.vm_size_bytes = Some(value);
        } else if let Some(value) = parse_kib_field(line, "VmStk:") {
            sample.vm_stack_bytes = Some(value);
        } else if let Some(value) = parse_kib_field(line, "KernelStack:") {
            sample.kernel_stack_bytes = Some(value);
        } else if let Some(value) = parse_usize_field(line, "Threads:") {
            sample.kernel_threads = Some(value);
        }
    }

    if sample.kernel_threads.is_none() {
        sample.kernel_threads = fs::read_dir("/proc/self/task").ok().map(|entries| entries.count());
    }

    read_linux_thread_snapshot(&mut sample);

    Some(sample)
}

#[cfg(not(target_os = "linux"))]
fn read_linux_proc_status() -> Option<ProcSample> {
    None
}

#[cfg(target_os = "linux")]
fn parse_kib_field(line: &str, key: &str) -> Option<u64> {
    let value = line.strip_prefix(key)?.split_whitespace().next()?;
    value.parse::<u64>().ok().map(|kib| kib.saturating_mul(1024))
}

#[cfg(target_os = "linux")]
fn parse_usize_field(line: &str, key: &str) -> Option<usize> {
    let value = line.strip_prefix(key)?.split_whitespace().next()?;
    value.parse::<usize>().ok()
}

#[cfg(target_os = "linux")]
fn read_linux_thread_snapshot(sample: &mut ProcSample) {
    let Ok(entries) = fs::read_dir("/proc/self/task") else {
        return;
    };

    let mut total = 0;
    let mut running = 0;
    let mut sleeping = 0;
    let mut uninterruptible = 0;
    let mut stopped = 0;
    let mut zombie = 0;
    let mut idle = 0;
    let mut wchan_counts: HashMap<String, usize> = HashMap::new();

    for entry in entries.flatten() {
        let task_dir = entry.path();
        let stat_path = task_dir.join("stat");
        let state = fs::read_to_string(&stat_path)
            .ok()
            .and_then(|stat| parse_task_state(&stat));

        if let Some(state) = state {
            total += 1;
            match state {
                'R' => running += 1,
                'S' => sleeping += 1,
                'D' => uninterruptible += 1,
                'T' | 't' => stopped += 1,
                'Z' => zombie += 1,
                'I' => idle += 1,
                _ => {}
            }
        }

        let wchan_path = task_dir.join("wchan");
        if let Ok(wchan) = fs::read_to_string(wchan_path) {
            let wchan = wchan.trim();
            if !wchan.is_empty() && wchan != "0" {
                *wchan_counts.entry(wchan.to_string()).or_default() += 1;
            }
        }
    }

    if total > 0 {
        sample.kernel_threads = Some(total);
    }
    sample.running_kernel_threads = Some(running);
    sample.sleeping_kernel_threads = Some(sleeping);
    sample.uninterruptible_kernel_threads = Some(uninterruptible);
    sample.stopped_kernel_threads = Some(stopped);
    sample.zombie_kernel_threads = Some(zombie);
    sample.idle_kernel_threads = Some(idle);

    if let Some((channel, count)) = wchan_counts.into_iter().max_by_key(|(_, count)| *count) {
        sample.top_wait_channel = Some(channel);
        sample.top_wait_channel_count = Some(count);
    }
}

#[cfg(target_os = "linux")]
fn parse_task_state(stat: &str) -> Option<char> {
    let command_end = stat.rfind(')')?;
    stat[command_end + 1..]
        .split_whitespace()
        .next()?
        .chars()
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_mul_saturates_instead_of_overflowing() {
        assert_eq!(bytes_mul(2, 1024), 2048);
        assert_eq!(bytes_mul(usize::MAX, usize::MAX), u64::MAX);
    }
}
