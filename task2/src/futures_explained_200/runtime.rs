use std::{
    future::Future, pin::Pin,
    sync::{atomic::{AtomicUsize, Ordering}, mpsc::{sync_channel, Receiver, SyncSender}, Arc, Mutex},
    task::{Context, Poll, Wake, Waker}, thread, time::Duration,
};

use super::trace::TraceLog;

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct Executor {
    ready: Receiver<Arc<Task>>,
    remaining: Arc<AtomicUsize>,
    trace: TraceLog,
}

#[derive(Clone)]
pub struct Spawner {
    ready: SyncSender<Arc<Task>>,
    remaining: Arc<AtomicUsize>,
    next_id: Arc<AtomicUsize>,
    trace: TraceLog,
}

struct Task {
    id: usize,
    name: &'static str,
    future: Mutex<Option<BoxFuture>>,
    ready: SyncSender<Arc<Task>>,
    remaining: Arc<AtomicUsize>,
    trace: TraceLog,
}

pub fn new_runtime(trace: TraceLog) -> (Spawner, Executor) {
    let (ready_tx, ready_rx) = sync_channel(1024);
    let remaining = Arc::new(AtomicUsize::new(0));
    let spawner = Spawner {
        ready: ready_tx,
        remaining: remaining.clone(),
        next_id: Arc::new(AtomicUsize::new(0)),
        trace: trace.clone(),
    };
    let executor = Executor { ready: ready_rx, remaining, trace };
    (spawner, executor)
}

impl Spawner {
    pub fn spawn<F>(&self, name: &'static str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.remaining.fetch_add(1, Ordering::SeqCst);
        self.trace.record(format!("task:spawn id={id} name={name} state=Ready storage=Future"));
        let task = Arc::new(Task {
            id,
            name,
            future: Mutex::new(Some(Box::pin(future))),
            ready: self.ready.clone(),
            remaining: self.remaining.clone(),
            trace: self.trace.clone(),
        });
        self.ready.send(task).expect("executor queue closed");
    }
}

impl Executor {
    pub fn run(&self) {
        self.trace.record("executor:run begin");
        while self.remaining.load(Ordering::SeqCst) > 0 {
            self.ready.recv().expect("executor queue closed").poll_once();
        }
        self.trace.record("executor:run end; all tasks completed");
    }
}

impl Task {
    fn poll_once(self: Arc<Self>) {
        let mut future_slot = self.future.lock().expect("task future lock poisoned");
        let Some(mut future) = future_slot.take() else {
            self.trace.record(format!("executor:skip id={} name={} reason=stale-wake", self.id, self.name));
            return;
        };

        self.trace.record(format!("executor:poll id={} name={} state=Running", self.id, self.name));
        let waker = Waker::from(self.clone());
        let mut cx = Context::from_waker(&waker);
        match future.as_mut().poll(&mut cx) {
            Poll::Pending => {
                self.trace.record(format!("executor:pending id={} name={} state=Suspended", self.id, self.name));
                *future_slot = Some(future);
            }
            Poll::Ready(()) => {
                self.trace.record(format!("executor:ready id={} name={} state=Finished", self.id, self.name));
                self.trace.record(format!("task:finish id={} name={}", self.id, self.name));
                self.remaining.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn schedule(self: &Arc<Self>, reason: &str) {
        self.trace.record(format!(
            "waker:schedule id={} name={} reason={} executor_state=Ready",
            self.id, self.name, reason
        ));
        if self.ready.send(self.clone()).is_err() {
            self.trace.record(format!("waker:drop id={} name={} reason=executor-closed", self.id, self.name));
        }
    }
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.schedule("wake");
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.schedule("wake_by_ref");
    }
}

pub struct Delay {
    task: &'static str,
    step: usize,
    duration: Duration,
    state: Arc<Mutex<DelayState>>,
    started: bool,
    trace: TraceLog,
}

struct DelayState {
    completed: bool,
    waker: Option<Waker>,
}

impl Delay {
    pub fn new(task: &'static str, step: usize, millis: u64, trace: TraceLog) -> Self {
        trace.record(format!("future:create task={task} step={step} kind=Delay duration_ms={millis}"));
        Self {
            task,
            step,
            duration: Duration::from_millis(millis),
            state: Arc::new(Mutex::new(DelayState { completed: false, waker: None })),
            started: false,
            trace,
        }
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.trace.record(format!("future:poll task={} step={} kind=Delay", self.task, self.step));
        {
            let mut state = self.state.lock().expect("delay state lock poisoned");
            if state.completed {
                self.trace.record(format!("future:ready task={} step={} kind=Delay", self.task, self.step));
                return Poll::Ready(());
            }
            state.waker = Some(cx.waker().clone());
        }

        if !self.started {
            self.started = true;
            let task = self.task;
            let step = self.step;
            let duration = self.duration;
            let state = self.state.clone();
            let trace = self.trace.clone();
            trace.record(format!("reactor:register task={task} step={step} source=timer"));
            let _ = thread::spawn(move || {
                thread::sleep(duration);
                let waker = {
                    let mut state = state.lock().expect("delay state lock poisoned");
                    state.completed = true;
                    state.waker.take()
                };
                trace.record(format!("reactor:event-ready task={task} step={step} source=timer"));
                if let Some(waker) = waker {
                    trace.record(format!("reactor:wake task={task} step={step}"));
                    waker.wake();
                }
            });
        } else {
            self.trace.record(format!("future:update-waker task={} step={}", self.task, self.step));
        }

        self.trace.record(format!("future:pending task={} step={} kind=Delay", self.task, self.step));
        Poll::Pending
    }
}
