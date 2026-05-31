use std::{thread, time::Duration};

use super::{
    runtime::{new_runtime, Delay},
    trace::TraceLog,
};

pub fn run_demo() {
    let trace = TraceLog::new();
    trace.record("experiment:start name=futures-explained-200");
    run_blocking_alternative(trace.clone());
    run_thread_alternative(trace.clone());
    run_future_runtime(trace.clone());
    trace.record("experiment:end name=futures-explained-200");
}

fn run_blocking_alternative(trace: TraceLog) {
    trace.record("alternative:blocking begin model=sequential-sleep");
    for name in ["alpha", "beta"] {
        trace.record(format!("blocking:start name={name} blocks=current-thread"));
        thread::sleep(Duration::from_millis(3));
        trace.record(format!("blocking:finish name={name}"));
    }
    trace.record("alternative:blocking end observation=no-overlap");
}

fn run_thread_alternative(trace: TraceLog) {
    trace.record("alternative:thread begin model=os-thread-per-task");
    let handles: Vec<_> = ["alpha", "beta"]
        .iter()
        .copied()
        .map(|name| {
            let trace = trace.clone();
            thread::spawn(move || {
                trace.record(format!("thread:start name={name} storage=os-stack"));
                thread::sleep(Duration::from_millis(3));
                trace.record(format!("thread:finish name={name}"));
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread alternative panicked");
    }
    trace.record("alternative:thread end observation=overlap-but-stack-per-task");
}

fn run_future_runtime(trace: TraceLog) {
    trace.record("alternative:future begin model=executor-reactor-waker");
    let (spawner, executor) = new_runtime(trace.clone());

    spawner.spawn("alpha", async_job("alpha", vec![12, 8], trace.clone()));
    spawner.spawn("beta", async_job("beta", vec![4, 6], trace.clone()));
    spawner.spawn("gamma", async_job("gamma", vec![7], trace.clone()));
    drop(spawner);

    executor.run();
    trace.record("alternative:future end observation=overlap-without-stack-per-task");
}

async fn async_job(name: &'static str, waits: Vec<u64>, trace: TraceLog) {
    trace.record(format!("app:{name} enter state=Running"));
    for (step, millis) in waits.into_iter().enumerate() {
        trace.record(format!(
            "app:{name} step={step} before-await duration_ms={millis}"
        ));
        Delay::new(name, step, millis, trace.clone()).await;
        trace.record(format!("app:{name} step={step} after-await"));
    }
    trace.record(format!("app:{name} return state=Finished"));
}
