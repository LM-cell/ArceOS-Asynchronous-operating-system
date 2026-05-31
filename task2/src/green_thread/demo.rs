use super::runtime::{trace_app, yield_now, Runtime};

pub fn run_stage1_demo() {
    let mut runtime = Runtime::new();

    runtime.spawn("alpha", || {
        for step in 0..3 {
            trace_app(format!("alpha step={}", step));
            yield_now();
        }
    });

    runtime.spawn("beta", || {
        for step in 0..2 {
            trace_app(format!("beta step={}", step));
            yield_now();
        }
    });

    runtime.spawn("gamma", || {
        trace_app("gamma step=0");
        yield_now();
        trace_app("gamma step=1");
    });

    runtime.run();
}

pub fn run_stage2_demo() {
    let mut runtime = Runtime::new();

    runtime.spawn_with_priority("low", 1, || {
        for step in 0..2 {
            trace_app(format!("low step={}", step));
            yield_now();
        }
    });

    runtime.spawn_with_priority("high-a", 3, || {
        for step in 0..2 {
            trace_app(format!("high-a step={}", step));
            yield_now();
        }
    });

    runtime.spawn_with_priority("high-b", 3, || {
        trace_app("high-b step=0");
        yield_now();
        trace_app("high-b step=1");
    });

    runtime.spawn_with_priority("mid", 2, || {
        trace_app("mid step=0");
        yield_now();
        trace_app("mid step=1");
    });

    runtime.run();
}
