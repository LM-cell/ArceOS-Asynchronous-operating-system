use std::ptr;

use super::context::{gt_switch, Context};

const STACK_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowState {
    Ready,
    Running,
    Finished,
}

pub struct GreenThread {
    id: usize,
    name: String,
    priority: usize,
    state: FlowState,
    context: Context,
    stack: Vec<u8>,
    entry: Option<Box<dyn FnMut()>>,
}

impl GreenThread {
    fn new(
        id: usize,
        name: impl Into<String>,
        priority: usize,
        entry: impl FnMut() + 'static,
    ) -> Self {
        let mut stack = vec![0_u8; STACK_SIZE];
        let stack_top = unsafe { stack.as_mut_ptr().add(stack.len()) as usize };
        let stack_top = stack_top & !0xf;
        let initial_rsp = stack_top - 16;

        unsafe {
            let ret_slot = initial_rsp as *mut usize;
            ret_slot.write(thread_bootstrap as usize);
        }

        Self {
            id,
            name: name.into(),
            priority,
            state: FlowState::Ready,
            context: Context::with_rsp(initial_rsp),
            stack,
            entry: Some(Box::new(entry)),
        }
    }
}

pub struct Runtime {
    threads: Vec<GreenThread>,
    main_context: Context,
    current: Option<usize>,
    tick: usize,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            threads: Vec::new(),
            main_context: Context::default(),
            current: None,
            tick: 0,
        }
    }

    pub fn spawn(&mut self, name: impl Into<String>, entry: impl FnMut() + 'static) {
        self.spawn_with_priority(name, 0, entry);
    }

    pub fn spawn_with_priority(
        &mut self,
        name: impl Into<String>,
        priority: usize,
        entry: impl FnMut() + 'static,
    ) {
        let id = self.threads.len();
        let thread = GreenThread::new(id, name, priority, entry);
        self.trace(format!(
            "task:create id={} name={} priority={} state={:?} stack={}B",
            thread.id,
            thread.name,
            thread.priority,
            thread.state,
            thread.stack.len()
        ));
        self.threads.push(thread);
    }

    pub fn run(&mut self) {
        unsafe {
            RUNTIME = self as *mut Runtime;
        }

        self.trace("runtime:run begin");
        if let Some(next) = self.pick_next(None) {
            self.switch_from_main_to(next);
        }
        self.trace("runtime:run end; all execution flows finished");

        unsafe {
            RUNTIME = ptr::null_mut();
        }
    }

    fn yield_current(&mut self) {
        let Some(from) = self.current else {
            return;
        };

        self.transition(from, FlowState::Ready, "yield");
        match self.pick_next(Some(from)) {
            Some(next) => self.switch_between(from, next),
            None => {
                self.transition(from, FlowState::Running, "yield-noop");
                let priority = self.threads[from].priority;
                self.trace(format!(
                    "scheduler:no other ready task; keep running task:{} priority={}",
                    from, priority
                ));
            }
        }
    }

    fn finish_current(&mut self) -> ! {
        let current = self
            .current
            .expect("finish_current called without a running green thread");
        self.transition(current, FlowState::Finished, "return");

        match self.pick_next(None) {
            Some(next) => self.switch_between(current, next),
            None => self.switch_to_main_from(current),
        }

        unreachable!("a finished green thread must never resume");
    }

    fn pick_next(&self, excluded: Option<usize>) -> Option<usize> {
        if self.threads.is_empty() {
            return None;
        }

        let best_priority = self
            .threads
            .iter()
            .enumerate()
            .filter(|(id, thread)| Some(*id) != excluded && thread.state == FlowState::Ready)
            .map(|(_, thread)| thread.priority)
            .max()?;

        let start = self.current.map_or(0, |id| (id + 1) % self.threads.len());
        for offset in 0..self.threads.len() {
            let id = (start + offset) % self.threads.len();
            if Some(id) == excluded {
                continue;
            }
            let thread = &self.threads[id];
            if thread.state == FlowState::Ready && thread.priority == best_priority {
                return Some(id);
            }
        }
        None
    }

    fn switch_from_main_to(&mut self, next: usize) {
        self.transition(next, FlowState::Running, "dispatch");
        self.current = Some(next);
        let next_priority = self.threads[next].priority;
        self.trace(format!(
            "switch:main -> task:{} priority={}",
            next, next_priority
        ));

        let old = &mut self.main_context as *mut Context;
        let new = &self.threads[next].context as *const Context;
        unsafe {
            gt_switch(old, new);
        }
    }

    fn switch_between(&mut self, from: usize, next: usize) {
        self.transition(next, FlowState::Running, "dispatch");
        self.current = Some(next);
        let from_priority = self.threads[from].priority;
        let next_priority = self.threads[next].priority;
        self.trace(format!(
            "switch:task:{} priority={} -> task:{} priority={}",
            from, from_priority, next, next_priority
        ));

        let old = &mut self.threads[from].context as *mut Context;
        let new = &self.threads[next].context as *const Context;
        unsafe {
            gt_switch(old, new);
        }
    }

    fn switch_to_main_from(&mut self, from: usize) {
        self.current = None;
        let from_priority = self.threads[from].priority;
        self.trace(format!(
            "switch:task:{} priority={} -> main",
            from, from_priority
        ));

        let old = &mut self.threads[from].context as *mut Context;
        let new = &self.main_context as *const Context;
        unsafe {
            gt_switch(old, new);
        }
    }

    fn transition(&mut self, id: usize, next: FlowState, reason: &str) {
        let old = self.threads[id].state;
        self.threads[id].state = next;
        self.trace(format!(
            "state:task:{} {} priority={} {:?}->{:?} reason={}",
            id, self.threads[id].name, self.threads[id].priority, old, next, reason
        ));
    }

    fn trace(&mut self, message: impl AsRef<str>) {
        println!("[trace #{:03}] {}", self.tick, message.as_ref());
        self.tick += 1;
    }
}

static mut RUNTIME: *mut Runtime = ptr::null_mut();

pub fn yield_now() {
    unsafe {
        assert!(!RUNTIME.is_null(), "green thread runtime is not installed");
        let runtime = &mut *RUNTIME;
        runtime.yield_current();
    }
}

pub fn trace_app(message: impl AsRef<str>) {
    unsafe {
        assert!(!RUNTIME.is_null(), "green thread runtime is not installed");
        let runtime = &mut *RUNTIME;
        runtime.trace(format!("app:{}", message.as_ref()));
    }
}

extern "C" fn thread_bootstrap() -> ! {
    unsafe {
        assert!(!RUNTIME.is_null(), "green thread runtime is not installed");
        let entry = {
            let runtime = &mut *RUNTIME;
            let id = runtime
                .current
                .expect("thread bootstrap called without current thread");
            let priority = runtime.threads[id].priority;
            runtime.trace(format!("task:{} priority={} enter", id, priority));
            runtime.threads[id]
                .entry
                .as_mut()
                .expect("green thread entry is missing") as *mut Box<dyn FnMut()>
        };
        (*entry)();

        let runtime = &mut *RUNTIME;
        let id = runtime
            .current
            .expect("thread entry returned without current thread");
        let priority = runtime.threads[id].priority;
        runtime.trace(format!(
            "task:{} priority={} return from entry",
            id, priority
        ));
        runtime.finish_current();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_noop(runtime: &mut Runtime, name: &'static str, priority: usize) {
        runtime.spawn_with_priority(name, priority, || {});
    }

    #[test]
    fn high_priority_ready_thread_runs_first() {
        let mut runtime = Runtime::new();
        spawn_noop(&mut runtime, "low", 1);
        spawn_noop(&mut runtime, "high", 10);
        spawn_noop(&mut runtime, "mid", 5);

        assert_eq!(runtime.pick_next(None), Some(1));
    }

    #[test]
    fn same_priority_uses_round_robin_order() {
        let mut runtime = Runtime::new();
        spawn_noop(&mut runtime, "a", 7);
        spawn_noop(&mut runtime, "b", 7);
        spawn_noop(&mut runtime, "c", 7);

        runtime.current = Some(0);
        assert_eq!(runtime.pick_next(Some(0)), Some(1));

        runtime.current = Some(1);
        assert_eq!(runtime.pick_next(Some(1)), Some(2));

        runtime.current = Some(2);
        assert_eq!(runtime.pick_next(Some(2)), Some(0));
    }

    #[test]
    fn yielded_thread_rejoins_priority_scheduling() {
        let mut runtime = Runtime::new();
        spawn_noop(&mut runtime, "high", 10);
        spawn_noop(&mut runtime, "low", 1);

        runtime.current = Some(0);
        runtime.threads[0].state = FlowState::Running;
        runtime.transition(0, FlowState::Ready, "test-yield");
        assert_eq!(runtime.pick_next(Some(0)), Some(1));

        runtime.current = Some(1);
        runtime.threads[1].state = FlowState::Running;
        runtime.transition(1, FlowState::Ready, "test-yield");
        assert_eq!(runtime.pick_next(Some(1)), Some(0));
    }
}
