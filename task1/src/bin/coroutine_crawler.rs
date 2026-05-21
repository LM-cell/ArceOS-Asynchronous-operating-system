use std::process;
use std::time::Instant;

use arceos_async_crawler_lab::{
    fetch_school_async, load_schools, parse_common_args, print_report, print_school_results,
    process_memory_kib, summarize,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = parse_common_args().unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(2);
    });
    let schools = load_schools(&config).unwrap_or_else(|err| {
        eprintln!("failed to load schools: {err}");
        process::exit(1);
    });
    if schools.is_empty() {
        eprintln!("no schools to crawl");
        process::exit(1);
    }

    let client = reqwest::Client::builder()
        .timeout(config.timeout)
        .build()
        .unwrap_or_else(|err| {
            eprintln!("failed to build HTTP client: {err}");
            process::exit(1);
        });
    let started = Instant::now();
    let handles = schools
        .into_iter()
        .map(|school| {
            tokio::spawn(fetch_school_async(
                client.clone(),
                school,
                config.output_dir.clone(),
            ))
        })
        .collect::<Vec<_>>();

    let mut samples = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(sample) = handle.await {
            samples.push(sample);
        }
    }

    let summary = summarize(&samples, started.elapsed());
    print_report("coroutine", &summary, process_memory_kib());
    print_school_results(&samples);
}
