use std::env;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use arceos_async_crawler_lab::{
    default_urls, mock_urls, print_report, process_memory_kib, summarize, CrawlSample,
};
use reqwest::blocking::Client;

fn fetch_once(url: &str) -> CrawlSample {
    if let Some((latency_ms, bytes)) = parse_mock_url(url) {
        let started = Instant::now();
        thread::sleep(Duration::from_millis(latency_ms));
        return CrawlSample::new(url.to_string(), started.elapsed(), bytes, true);
    }

    let started = Instant::now();
    let client = Client::new();
    let result = client.get(url).send();
    match result {
        Ok(resp) => match resp.bytes() {
            Ok(body) => CrawlSample::new(url.to_string(), started.elapsed(), body.len(), true),
            Err(_) => CrawlSample::new(url.to_string(), started.elapsed(), 0, false),
        },
        Err(_) => CrawlSample::new(url.to_string(), started.elapsed(), 0, false),
    }
}

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

fn main() {
    let (reqs, mock) = parse_args();
    let urls = if mock {
        mock_urls(reqs)
    } else {
        default_urls(reqs)
    };

    let (tx, rx) = mpsc::channel();
    let started = Instant::now();

    for url in urls {
        let tx_cloned = tx.clone();
        thread::spawn(move || {
            let sample = fetch_once(&url);
            let _ = tx_cloned.send(sample);
        });
    }
    drop(tx);

    let samples: Vec<CrawlSample> = rx.into_iter().collect();

    let summary = summarize(&samples, started.elapsed());
    print_report("thread", &summary, process_memory_kib());
}
