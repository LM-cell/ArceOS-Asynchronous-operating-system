use std::env;
use std::path::PathBuf;
use std::process::{self, Command, Stdio};
use std::time::{Duration, Instant};

use arceos_async_crawler_lab::{
    fetch_school_blocking, load_schools, parse_common_args, print_report, print_school_results,
    process_memory_kib, sample_from_tsv, sample_to_tsv, summarize, CrawlSample, School,
};

fn run_child(args: &[String]) -> Result<(), String> {
    let name = args
        .get(2)
        .ok_or_else(|| "missing child school name".to_string())?
        .to_string();
    let url = args
        .get(3)
        .ok_or_else(|| "missing child school url".to_string())?
        .to_string();
    let output_dir = PathBuf::from(
        args.get(4)
            .ok_or_else(|| "missing child output dir".to_string())?,
    );
    let timeout_ms = args
        .get(5)
        .ok_or_else(|| "missing child timeout".to_string())?
        .parse::<u64>()
        .map_err(|_| "invalid child timeout".to_string())?;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| err.to_string())?;
    let school = School { name, url };
    let sample = fetch_school_blocking(&client, &school, &output_dir);
    println!("{}", sample_to_tsv(&sample));
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child") {
        if let Err(err) = run_child(&args) {
            eprintln!("{err}");
            process::exit(2);
        }
        return;
    }

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

    let exe = env::current_exe().expect("failed to locate current executable");
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut children = Vec::new();

    for school in schools {
        match Command::new(&exe)
            .arg("--child")
            .arg(&school.name)
            .arg(&school.url)
            .arg(config.output_dir.as_os_str())
            .arg(config.timeout.as_millis().to_string())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => children.push((school, child)),
            Err(err) => samples.push(CrawlSample::failure(
                school.name,
                school.url,
                Duration::from_millis(0),
                err.to_string(),
            )),
        }
    }

    for (school, child) in children {
        match child.wait_with_output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let parsed = stdout.lines().last().and_then(sample_from_tsv);
                match parsed {
                    Some(sample) if output.status.success() || !sample.ok => samples.push(sample),
                    Some(sample) => samples.push(sample),
                    None => samples.push(CrawlSample::failure(
                        school.name,
                        school.url,
                        Duration::from_millis(0),
                        "child process did not return a crawl sample",
                    )),
                }
            }
            Err(err) => samples.push(CrawlSample::failure(
                school.name,
                school.url,
                Duration::from_millis(0),
                err.to_string(),
            )),
        }
    }

    let summary = summarize(&samples, started.elapsed());
    print_report("process", &summary, process_memory_kib());
    print_school_results(&samples);
}
