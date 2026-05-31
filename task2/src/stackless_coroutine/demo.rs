use super::{
    tiny::{yield_now, Executor},
    trace::TraceLog,
};

pub fn run_demo() {
    let trace = TraceLog::new();
    let runtime_trace = trace.clone();
    let mut executor = Executor::new(move |message| runtime_trace.record(message));

    executor.spawn("alpha", coroutine_body("alpha", 3, trace.clone()));
    executor.spawn("beta", coroutine_body("beta", 2, trace.clone()));
    executor.spawn("gamma", coroutine_body("gamma", 2, trace.clone()));

    executor.run();
}

async fn coroutine_body(name: &'static str, steps: usize, trace: TraceLog) {
    trace.record(format!("app:{name} enter state=Running"));
    for step in 0..steps {
        trace.record(format!("app:{name} step={step} before-yield"));
        yield_now().await;
        trace.record(format!("app:{name} step={step} after-yield"));
    }
    trace.record(format!("app:{name} return state=Finished"));
}
