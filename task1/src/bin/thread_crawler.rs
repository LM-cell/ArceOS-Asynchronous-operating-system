use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use arceos_async_crawler_lab::{
    fetch_school_blocking, load_schools, parse_common_args, print_report, print_school_results,
    process_memory_kib, summarize,
};

fn main() {
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

    let client = reqwest::blocking::Client::builder()
        .timeout(config.timeout)
        .build()
        .unwrap_or_else(|err| {
            eprintln!("failed to build HTTP client: {err}");
            process::exit(1);
        });
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();

    for school in schools {
        let tx = tx.clone();
        let client = client.clone();
        let output_dir = config.output_dir.clone();
        thread::spawn(move || {
            let sample = fetch_school_blocking(&client, &school, &output_dir);
            let _ = tx.send(sample);
        });
    }
    drop(tx);

    let samples = rx.into_iter().collect::<Vec<_>>();
    let summary = summarize(&samples, started.elapsed());
    print_report("thread", &summary, process_memory_kib());
    print_school_results(&samples);
}
