use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct School {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct CrawlConfig {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
    pub limit: Option<usize>,
    pub mock: bool,
    pub requests: usize,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        let data_input = PathBuf::from("data").join("schools.csv");
        let input = if data_input.exists() {
            data_input
        } else {
            PathBuf::from("schools.csv")
        };

        Self {
            input,
            output_dir: PathBuf::from("."),
            timeout: Duration::from_millis(15_000),
            limit: None,
            mock: false,
            requests: 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrawlSample {
    pub name: String,
    pub url: String,
    pub latency_ms: u128,
    pub bytes: usize,
    pub ok: bool,
    pub error: Option<String>,
}

impl CrawlSample {
    pub fn success(name: String, url: String, latency: Duration, bytes: usize) -> Self {
        Self {
            name,
            url,
            latency_ms: latency.as_millis(),
            bytes,
            ok: true,
            error: None,
        }
    }

    pub fn failure(name: String, url: String, latency: Duration, error: impl Into<String>) -> Self {
        Self {
            name,
            url,
            latency_ms: latency.as_millis(),
            bytes: 0,
            ok: false,
            error: Some(error.into()),
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
    pub bytes_saved: usize,
}

pub fn parse_common_args() -> Result<CrawlConfig, String> {
    parse_common_args_from(env::args().skip(1))
}

pub fn parse_common_args_from<I, S>(args: I) -> Result<CrawlConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut config = CrawlConfig::default();
    let mut args = args.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => {
                config.input = PathBuf::from(require_value(&mut args, "--input")?);
            }
            "--output-dir" => {
                config.output_dir = PathBuf::from(require_value(&mut args, "--output-dir")?);
            }
            "--timeout-ms" => {
                let value = require_value(&mut args, "--timeout-ms")?;
                let timeout_ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --timeout-ms value: {value}"))?;
                config.timeout = Duration::from_millis(timeout_ms.max(1));
            }
            "--limit" => {
                let value = require_value(&mut args, "--limit")?;
                let limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --limit value: {value}"))?;
                config.limit = Some(limit.max(1));
            }
            "--mock" => {
                config.mock = true;
            }
            "--requests" => {
                let value = require_value(&mut args, "--requests")?;
                let requests = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --requests value: {value}"))?;
                config.requests = requests.max(1);
            }
            "--help" | "-h" => {
                return Err(help_text().to_string());
            }
            other => {
                return Err(format!("unknown argument: {other}\n\n{}", help_text()));
            }
        }
    }

    Ok(config)
}

fn require_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value after {flag}"))
}

pub fn help_text() -> &'static str {
    "usage: crawler [--input data/schools.csv] [--output-dir outputs/run] [--timeout-ms 15000] [--limit N] [--mock --requests N]"
}

pub fn load_schools(config: &CrawlConfig) -> io::Result<Vec<School>> {
    let mut schools = if config.mock {
        mock_schools(config.requests)
    } else {
        read_schools(&config.input)?
    };

    if let Some(limit) = config.limit {
        schools.truncate(limit);
    }

    Ok(schools)
}

pub fn read_schools(path: &Path) -> io::Result<Vec<School>> {
    let content = fs::read_to_string(path)?;
    let mut rows = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(split_csv_line);

    let Some(first) = rows.next() else {
        return Ok(Vec::new());
    };

    let (name_idx, url_idx, data_rows) = if looks_like_header(&first) {
        let name_idx = find_header(&first, &["院校名称", "学校名称", "学校", "name"]).unwrap_or(0);
        let url_idx = find_header(&first, &["官方网站", "官网", "网址", "url", "website"]).unwrap_or(1);
        (name_idx, url_idx, rows.collect::<Vec<_>>())
    } else {
        let mut data_rows = vec![first];
        data_rows.extend(rows);
        (0, 1, data_rows)
    };

    let schools = data_rows
        .into_iter()
        .filter_map(|row| {
            let name = row.get(name_idx)?.trim().to_string();
            let url = normalize_url(row.get(url_idx)?.trim());
            if name.is_empty() || url.is_empty() {
                None
            } else {
                Some(School { name, url })
            }
        })
        .collect();

    Ok(schools)
}

fn looks_like_header(row: &[String]) -> bool {
    row.iter().any(|field| {
        let field = field.trim().to_ascii_lowercase();
        field.contains("院校")
            || field.contains("学校")
            || field.contains("官网")
            || field.contains("url")
            || field.contains("website")
    })
}

fn find_header(row: &[String], candidates: &[&str]) -> Option<usize> {
    row.iter().position(|field| {
        let normalized = field.trim().to_ascii_lowercase();
        candidates
            .iter()
            .any(|candidate| normalized.contains(&candidate.to_ascii_lowercase()))
    })
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(ch),
        }
    }

    fields.push(field.trim().to_string());
    fields
}

fn normalize_url(raw: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("mock://") {
        raw.to_string()
    } else if raw.is_empty() {
        String::new()
    } else {
        format!("https://{raw}")
    }
}

pub fn mock_schools(requests: usize) -> Vec<School> {
    let seed = ["mock://25/1024", "mock://35/2048", "mock://45/3072"];
    (0..requests)
        .map(|i| School {
            name: format!("模拟学校{:03}", i + 1),
            url: seed[i % seed.len()].to_string(),
        })
        .collect()
}

pub fn fetch_school_blocking(
    client: &reqwest::blocking::Client,
    school: &School,
    output_dir: &Path,
) -> CrawlSample {
    let started = Instant::now();
    let result = if let Some((_, bytes)) = parse_mock_url(&school.url) {
        std::thread::sleep(mock_latency(&school.url));
        Ok(mock_plain_text(&school.name, &school.url, bytes))
    } else {
        client
            .get(&school.url)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.bytes())
            .map(|body| html_to_plain_text(&String::from_utf8_lossy(&body)))
            .map_err(|err| err.to_string())
    };

    match result {
        Ok(text) => match save_plain_text(output_dir, &school.name, &text) {
            Ok(bytes) => CrawlSample::success(
                school.name.clone(),
                school.url.clone(),
                started.elapsed(),
                bytes,
            ),
            Err(err) => CrawlSample::failure(
                school.name.clone(),
                school.url.clone(),
                started.elapsed(),
                err.to_string(),
            ),
        },
        Err(err) => CrawlSample::failure(
            school.name.clone(),
            school.url.clone(),
            started.elapsed(),
            err,
        ),
    }
}

pub async fn fetch_school_async(
    client: reqwest::Client,
    school: School,
    output_dir: PathBuf,
) -> CrawlSample {
    let started = Instant::now();
    let result = if let Some((_, bytes)) = parse_mock_url(&school.url) {
        tokio::time::sleep(mock_latency(&school.url)).await;
        Ok(mock_plain_text(&school.name, &school.url, bytes))
    } else {
        match client
            .get(&school.url)
            .send()
            .await
            .and_then(|response| response.error_for_status())
        {
            Ok(response) => response
                .bytes()
                .await
                .map(|body| html_to_plain_text(&String::from_utf8_lossy(&body)))
                .map_err(|err| err.to_string()),
            Err(err) => Err(err.to_string()),
        }
    };

    match result {
        Ok(text) => match save_plain_text(&output_dir, &school.name, &text) {
            Ok(bytes) => CrawlSample::success(school.name, school.url, started.elapsed(), bytes),
            Err(err) => CrawlSample::failure(
                school.name,
                school.url,
                started.elapsed(),
                err.to_string(),
            ),
        },
        Err(err) => CrawlSample::failure(school.name, school.url, started.elapsed(), err),
    }
}

fn parse_mock_url(url: &str) -> Option<(u64, usize)> {
    let payload = url.strip_prefix("mock://")?;
    let (latency, bytes) = payload.split_once('/')?;
    Some((latency.parse().ok()?, bytes.parse().ok()?))
}

fn mock_latency(url: &str) -> Duration {
    let (latency_ms, _) = parse_mock_url(url).unwrap_or((1, 128));
    Duration::from_millis(latency_ms)
}

fn mock_plain_text(name: &str, url: &str, bytes: usize) -> String {
    let base = format!("{name}\n{url}\n这是用于离线基准测试的模拟首页纯文本。\n");
    let repeated = base.repeat((bytes / base.len()).max(1) + 1);
    repeated.chars().take(bytes).collect()
}

pub fn save_plain_text(output_dir: &Path, school_name: &str, text: &str) -> io::Result<usize> {
    fs::create_dir_all(output_dir)?;
    let path = output_path(output_dir, school_name);
    fs::write(&path, text)?;
    Ok(text.as_bytes().len())
}

pub fn output_path(output_dir: &Path, school_name: &str) -> PathBuf {
    output_dir.join(sanitize_file_name(school_name))
}

pub fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();
    let sanitized = sanitized.trim().trim_matches('.').to_string();
    if sanitized.is_empty() {
        "school".to_string()
    } else {
        sanitized
    }
}

pub fn html_to_plain_text(html: &str) -> String {
    let without_scripts = ["script", "style", "noscript"]
        .iter()
        .fold(html.to_string(), |acc, tag| strip_tag_blocks(&acc, tag));

    let mut text = String::with_capacity(without_scripts.len());
    let mut in_tag = false;
    for ch in without_scripts.chars() {
        match ch {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }

    collapse_whitespace(&decode_basic_entities(&text))
}

fn strip_tag_blocks(input: &str, tag: &str) -> String {
    let mut result = input.to_string();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");

    loop {
        let lower = result.to_ascii_lowercase();
        let Some(start) = lower.find(&open) else {
            break;
        };
        let Some(close_start) = lower[start..].find(&close).map(|idx| start + idx) else {
            break;
        };
        let end = close_start + close.len();
        result.replace_range(start..end, " ");
    }

    result
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn collapse_whitespace(text: &str) -> String {
    let mut collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if !collapsed.is_empty() {
        collapsed.push('\n');
    }
    collapsed
}

pub fn summarize(samples: &[CrawlSample], total_elapsed: Duration) -> CrawlSummary {
    let mut latencies: Vec<u128> = samples
        .iter()
        .filter(|sample| sample.ok)
        .map(|sample| sample.latency_ms)
        .collect();
    latencies.sort_unstable();

    let success = latencies.len();
    let total = samples.len();
    let failed = total.saturating_sub(success);
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
        p50_ms: percentile(&latencies, 0.50),
        p95_ms: percentile(&latencies, 0.95),
        max_ms: latencies.last().copied().unwrap_or(0),
        throughput_rps,
        bytes_saved: samples
            .iter()
            .filter(|sample| sample.ok)
            .map(|sample| sample.bytes)
            .sum(),
    }
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

pub fn process_memory_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
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
    println!("bytes_saved={}", summary.bytes_saved);
    if let Some(mem) = memory_kib {
        println!("memory_kib={mem}");
    }
}

pub fn print_school_results(samples: &[CrawlSample]) {
    println!("success_schools:");
    let mut success_count = 0;
    for sample in samples.iter().filter(|sample| sample.ok) {
        success_count += 1;
        println!(
            "  - {} latency_ms={} bytes={}",
            sample.name, sample.latency_ms, sample.bytes
        );
    }
    if success_count == 0 {
        println!("  - none");
    }

    println!("failed_schools:");
    let mut failed_count = 0;
    for sample in samples.iter().filter(|sample| !sample.ok) {
        failed_count += 1;
        println!(
            "  - {} url={} latency_ms={} error={}",
            sample.name,
            sample.url,
            sample.latency_ms,
            sample.error.as_deref().unwrap_or("unknown")
        );
    }
    if failed_count == 0 {
        println!("  - none");
    }
}

pub fn sample_to_tsv(sample: &CrawlSample) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        sample.ok,
        sample.latency_ms,
        sample.bytes,
        clean_tsv_field(&sample.name),
        clean_tsv_field(&sample.url),
        clean_tsv_field(sample.error.as_deref().unwrap_or(""))
    )
}

pub fn sample_from_tsv(line: &str) -> Option<CrawlSample> {
    let mut parts = line.trim_end().splitn(6, '\t');
    let ok = parts.next()? == "true";
    let latency_ms = parts.next()?.parse().ok()?;
    let bytes = parts.next()?.parse().ok()?;
    let name = parts.next()?.to_string();
    let url = parts.next()?.to_string();
    let error = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    Some(CrawlSample {
        name,
        url,
        latency_ms,
        bytes,
        ok,
        error,
    })
}

fn clean_tsv_field(value: &str) -> String {
    value
        .replace('\t', " ")
        .replace('\r', " ")
        .replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_calculates_percentiles() {
        let samples = vec![
            CrawlSample::success("a".into(), "u1".into(), Duration::from_millis(10), 1),
            CrawlSample::success("b".into(), "u2".into(), Duration::from_millis(20), 1),
            CrawlSample::success("c".into(), "u3".into(), Duration::from_millis(30), 1),
            CrawlSample::success("d".into(), "u4".into(), Duration::from_millis(40), 1),
            CrawlSample::success("e".into(), "u5".into(), Duration::from_millis(50), 1),
        ];

        let summary = summarize(&samples, Duration::from_secs(1));
        assert_eq!(summary.total, 5);
        assert_eq!(summary.success, 5);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.p50_ms, 30);
        assert_eq!(summary.p95_ms, 50);
        assert_eq!(summary.max_ms, 50);
        assert_eq!(summary.bytes_saved, 5);
        assert!((summary.throughput_rps - 5.0).abs() < 1e-6);
    }

    #[test]
    fn summarize_handles_failures() {
        let samples = vec![
            CrawlSample::success("ok".into(), "u".into(), Duration::from_millis(15), 3),
            CrawlSample::failure("bad".into(), "u".into(), Duration::from_millis(0), "boom"),
        ];

        let summary = summarize(&samples, Duration::from_millis(500));
        assert_eq!(summary.total, 2);
        assert_eq!(summary.success, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.p50_ms, 15);
        assert_eq!(summary.p95_ms, 15);
        assert_eq!(summary.max_ms, 15);
        assert_eq!(summary.bytes_saved, 3);
        assert!((summary.throughput_rps - 2.0).abs() < 1e-6);
    }

    #[test]
    fn csv_parser_accepts_chinese_headers() {
        let row = split_csv_line("院校名称,官方网站");
        assert!(looks_like_header(&row));
        assert_eq!(find_header(&row, &["院校名称"]), Some(0));
        assert_eq!(find_header(&row, &["官方网站"]), Some(1));
    }

    #[test]
    fn html_to_plain_text_removes_markup_and_scripts() {
        let html = r#"<html><script>bad()</script><body><h1>标题</h1><p>A&nbsp;&amp;&nbsp;B</p></body></html>"#;
        assert_eq!(html_to_plain_text(html), "标题 A & B\n");
    }

    #[test]
    fn sample_tsv_roundtrip() {
        let sample = CrawlSample::failure(
            "清华大学".into(),
            "https://www.tsinghua.edu.cn/".into(),
            Duration::from_millis(7),
            "network error",
        );
        let parsed = sample_from_tsv(&sample_to_tsv(&sample)).unwrap();
        assert_eq!(parsed.name, "清华大学");
        assert_eq!(parsed.latency_ms, 7);
        assert!(!parsed.ok);
        assert_eq!(parsed.error.as_deref(), Some("network error"));
    }
}
