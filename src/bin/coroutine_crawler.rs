use std::env;
use std::time::{Duration, Instant};

use arceos_async_crawler_lab::{
    default_urls, mock_urls, print_report, process_memory_kib, summarize, CrawlSample,
};

fn parse_args() -> (usize, bool) {
    let mut args = env::args().skip(1);
    let mut requests = 9;
    let mut mock = false;
    while let Some(arg) = args.next() {
        if arg == "--requests" {
            if let Some(v) = args.next() {
                if let Ok(n) = v.parse::<usize>() {
                    requests = n.max(1);
                }
            }
        } else if arg == "--mock" {
            mock = true;
        }
    }
    (requests, mock)
}

fn parse_mock_url(url: &str) -> Option<(u64, usize)> {
    let payload = url.strip_prefix("mock://")?;
    let (latency, bytes) = payload.split_once('/')?;
    Some((latency.parse().ok()?, bytes.parse().ok()?))
}

async fn fetch_once(url: String) -> CrawlSample {
    if let Some((latency_ms, bytes)) = parse_mock_url(&url) {
        let started = Instant::now();
        tokio::time::sleep(Duration::from_millis(latency_ms)).await;
        return CrawlSample::new(url, started.elapsed(), bytes, true);
    }

    let started = Instant::now();
    match reqwest::get(&url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(body) => CrawlSample::new(url, started.elapsed(), body.len(), true),
            Err(_) => CrawlSample::new(url, started.elapsed(), 0, false),
        },
        Err(_) => CrawlSample::new(url, started.elapsed(), 0, false),
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let (reqs, mock) = parse_args();
    let urls = if mock {
        mock_urls(reqs)
    } else {
        default_urls(reqs)
    };
    let started = Instant::now();

    let handles: Vec<_> = urls
        .into_iter()
        .map(|url| tokio::spawn(fetch_once(url)))
        .collect();

    let mut samples = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(sample) = handle.await {
            samples.push(sample);
        }
    }

    let summary = summarize(&samples, started.elapsed());
    print_report("coroutine", &summary, process_memory_kib());
}
