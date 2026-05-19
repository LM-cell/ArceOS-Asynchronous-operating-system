use std::env;
use std::process::{Command, Stdio};
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
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child") {
        if let Some(url) = args.get(2) {
            let sample = fetch_once(url);
            println!(
                "{}|{}|{}|{}",
                sample.ok, sample.latency_ms, sample.bytes, sample.url
            );
        }
        return;
    }

    let (reqs, mock) = parse_args();
    let urls = if mock {
        mock_urls(reqs)
    } else {
        default_urls(reqs)
    };
    let exe = env::current_exe().expect("failed to locate current executable");
    let started = Instant::now();

    let children: Vec<_> = urls
        .iter()
        .map(|url| {
            Command::new(&exe)
                .arg("--child")
                .arg(url)
                .stdout(Stdio::piped())
                .spawn()
                .expect("failed to spawn child process")
        })
        .collect();

    let mut samples = Vec::with_capacity(children.len());
    for child in children {
        let output = child
            .wait_with_output()
            .expect("failed to wait for child process");
        let line = String::from_utf8_lossy(&output.stdout);
        let mut parts = line.trim().split('|');
        let ok = parts.next().unwrap_or("false") == "true";
        let latency_ms = parts.next().unwrap_or("0").parse::<u128>().unwrap_or(0);
        let bytes = parts.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
        let url = parts.next().unwrap_or("unknown").to_string();
        samples.push(CrawlSample {
            url,
            latency_ms,
            bytes,
            ok,
        });
    }

    let summary = summarize(&samples, started.elapsed());
    print_report("process", &summary, process_memory_kib());
}
