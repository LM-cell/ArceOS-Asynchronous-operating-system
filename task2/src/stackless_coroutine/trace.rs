use std::{
    cell::Cell,
    rc::Rc,
};

#[derive(Clone, Default)]
pub struct TraceLog {
    tick: Rc<Cell<usize>>,
}

impl TraceLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, message: impl AsRef<str>) {
        let tick = self.tick.get();
        println!("[stackless-trace #{tick:03}] {}", message.as_ref());
        self.tick.set(tick + 1);
    }
}
