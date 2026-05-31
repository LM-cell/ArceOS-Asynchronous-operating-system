use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct TraceLog {
    tick: Arc<Mutex<usize>>,
}

impl TraceLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, message: impl AsRef<str>) {
        let mut tick = self.tick.lock().expect("trace lock poisoned");
        println!("[futures-200-trace #{:03}] {}", *tick, message.as_ref());
        *tick += 1;
    }
}
