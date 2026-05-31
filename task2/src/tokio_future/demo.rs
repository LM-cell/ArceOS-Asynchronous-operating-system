use tokio::{runtime::Builder, task::JoinHandle};

use super::{trace::TraceLog, traced_future::YieldOnce};

pub fn run_demo() {
    let runtime = Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to build Tokio runtime");

    runtime.block_on(async {
        let trace = TraceLog::new();
        trace.record("runtime:create kind=tokio-current-thread");

        let alpha = spawn_traced_task("alpha", 3, trace.clone());
        let beta = spawn_traced_task("beta", 2, trace.clone());
        let gamma = spawn_traced_task("gamma", 2, trace.clone());

        trace.record("runtime:await join handles");
        alpha.await.expect("alpha task panicked");
        beta.await.expect("beta task panicked");
        gamma.await.expect("gamma task panicked");
        trace.record("runtime:all tasks completed");
    });
}

fn spawn_traced_task(name: &'static str, steps: usize, trace: TraceLog) -> JoinHandle<()> {
    trace.record(format!(
        "task:spawn name={name} state=Ready executor=tokio"
    ));
    tokio::spawn(async move {
        trace.record(format!("task:enter name={name} state=Running"));
        for step in 0..steps {
            trace.record(format!("app:{name} step={step} before-await"));
            YieldOnce::new(name, step, trace.clone()).await;
            trace.record(format!("app:{name} step={step} after-await"));
        }
        trace.record(format!("task:return name={name} state=Finished"));
    })
}
