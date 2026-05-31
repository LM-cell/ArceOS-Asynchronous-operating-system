use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Default)]
pub struct TraceLog {
    tick: Arc<AtomicUsize>,
}

impl TraceLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, message: impl AsRef<str>) {
        let tick = self.tick.fetch_add(1, Ordering::SeqCst);
        println!("[tokio-trace #{tick:03}] {}", message.as_ref());
    }
}
