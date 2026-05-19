use std::fs;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CrawlSample {
    pub url: String,
    pub latency_ms: u128,
    pub bytes: usize,
    pub ok: bool,
}

impl CrawlSample {
    pub fn new(url: String, latency: Duration, bytes: usize, ok: bool) -> Self {
        Self {
            url,
            latency_ms: latency.as_millis(),
            bytes,
            ok,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrawlSummary {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub max_ms: u128,
    pub throughput_rps: f64,
}

pub fn summarize(samples: &[CrawlSample], total_elapsed: Duration) -> CrawlSummary {
    let mut latencies: Vec<u128> = samples
        .iter()
        .filter(|s| s.ok)
        .map(|s| s.latency_ms)
        .collect();
    latencies.sort_unstable();

    let success = latencies.len();
    let total = samples.len();
    let failed = total.saturating_sub(success);

    let p50_ms = percentile(&latencies, 0.50);
    let p95_ms = percentile(&latencies, 0.95);
    let max_ms = latencies.last().copied().unwrap_or(0);
    let elapsed_secs = total_elapsed.as_secs_f64();
    let throughput_rps = if elapsed_secs > 0.0 {
        success as f64 / elapsed_secs
    } else {
        0.0
    };

    CrawlSummary {
        total,
        success,
        failed,
        p50_ms,
        p95_ms,
        max_ms,
        throughput_rps,
    }
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

pub fn default_urls(requests: usize) -> Vec<String> {
    let seed = [
        "https://example.com/",
        "https://www.rust-lang.org/",
        "https://docs.rs/",
    ];

    (0..requests)
        .map(|i| seed[i % seed.len()].to_string())
        .collect()
}

pub fn mock_urls(requests: usize) -> Vec<String> {
    let seed = ["mock://25/1024", "mock://35/2048", "mock://45/3072"];
    (0..requests)
        .map(|i| seed[i % seed.len()].to_string())
        .collect()
}

pub fn process_memory_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    line.split_whitespace().nth(1)?.parse::<u64>().ok()
}

pub fn print_report(model: &str, summary: &CrawlSummary, memory_kib: Option<u64>) {
    println!("model={model}");
    println!(
        "total={} success={} failed={}",
        summary.total, summary.success, summary.failed
    );
    println!(
        "latency_ms p50={} p95={} max={}",
        summary.p50_ms, summary.p95_ms, summary.max_ms
    );
    println!("throughput_rps={:.2}", summary.throughput_rps);
    if let Some(mem) = memory_kib {
        println!("memory_kib={mem}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_calculates_percentiles() {
        let samples = vec![
            CrawlSample::new("u1".into(), Duration::from_millis(10), 1, true),
            CrawlSample::new("u2".into(), Duration::from_millis(20), 1, true),
            CrawlSample::new("u3".into(), Duration::from_millis(30), 1, true),
            CrawlSample::new("u4".into(), Duration::from_millis(40), 1, true),
            CrawlSample::new("u5".into(), Duration::from_millis(50), 1, true),
        ];

        let summary = summarize(&samples, Duration::from_secs(1));
        assert_eq!(summary.total, 5);
        assert_eq!(summary.success, 5);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.p50_ms, 30);
        assert_eq!(summary.p95_ms, 50);
        assert_eq!(summary.max_ms, 50);
        assert!((summary.throughput_rps - 5.0).abs() < 1e-6);
    }

    #[test]
    fn summarize_handles_failures() {
        let samples = vec![
            CrawlSample::new("ok".into(), Duration::from_millis(15), 1, true),
            CrawlSample::new("bad".into(), Duration::from_millis(0), 0, false),
        ];

        let summary = summarize(&samples, Duration::from_millis(500));
        assert_eq!(summary.total, 2);
        assert_eq!(summary.success, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.p50_ms, 15);
        assert_eq!(summary.p95_ms, 15);
        assert_eq!(summary.max_ms, 15);
        assert!((summary.throughput_rps - 2.0).abs() < 1e-6);
    }
}
