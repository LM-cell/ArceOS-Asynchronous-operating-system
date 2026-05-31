use std::{env, process};

#[cfg(all(target_arch = "x86_64", unix))]
mod green_thread;
mod futures_explained_200;
mod stackless_coroutine;
mod tokio_future;

fn main() {
    let demo = env::args()
        .nth(1)
        .or_else(|| env::var("TASK2_DEMO").ok())
        .or_else(|| env::var("GT_DEMO").ok())
        .unwrap_or_else(|| "stage2".to_string());

    match demo.as_str() {
        "stage1" | "green-stage1" => run_green_stage1(),
        "stage2" | "green-stage2" | "priority" => run_green_stage2(),
        "stackless" | "stackless-coroutine" => stackless_coroutine::run_demo(),
        "futures-200" | "futures-explained" => futures_explained_200::run_demo(),
        "tokio" | "tokio-future" => tokio_future::run_demo(),
        "--help" | "-h" | "help" => print_help(),
        unknown => {
            eprintln!("unknown demo: {unknown}");
            print_help();
            process::exit(2);
        }
    }
}

#[cfg(all(target_arch = "x86_64", unix))]
fn run_green_stage1() {
    green_thread::run_stage1_demo();
}

#[cfg(not(all(target_arch = "x86_64", unix)))]
fn run_green_stage1() {
    eprintln!("green thread demos require Linux x86_64 because they use hand-written context-switch assembly");
    process::exit(1);
}

#[cfg(all(target_arch = "x86_64", unix))]
fn run_green_stage2() {
    green_thread::run_stage2_demo();
}

#[cfg(not(all(target_arch = "x86_64", unix)))]
fn run_green_stage2() {
    eprintln!("green thread demos require Linux x86_64 because they use hand-written context-switch assembly");
    process::exit(1);
}

fn print_help() {
    eprintln!(
        "usage: cargo run --release -- <demo>\n\
         demos:\n\
           stage1        original green-thread round-robin trace\n\
           stage2        green-thread priority scheduler trace\n\
           stackless     std-only stack-less coroutine trace\n\
           futures-200   Futures Explained executor/reactor/waker trace\n\
           tokio-future  Tokio Future poll/wake trace"
    );
}
