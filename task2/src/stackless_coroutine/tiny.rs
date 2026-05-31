use std::{
    collections::VecDeque, future::Future, pin::Pin, sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;

struct Coroutine { name: &'static str, future: BoxFuture }

pub struct Executor {
    queue: VecDeque<Coroutine>,
    trace: Box<dyn FnMut(String)>,
}

impl Executor {
    pub fn new(trace: impl FnMut(String) + 'static) -> Self {
        Self { queue: VecDeque::new(), trace: Box::new(trace) }
    }

    pub fn spawn<F>(&mut self, name: &'static str, future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.event(format!(
            "coroutine:create name={name} state=Created storage=heap-pinned-future"
        ));
        self.queue.push_back(Coroutine { name, future: Box::pin(future) });
    }

    pub fn run(&mut self) {
        self.event("runtime:run begin executor=std-only-stackless");
        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);

        while let Some(mut coroutine) = self.queue.pop_front() {
            self.event(format!("coroutine:poll name={} state=Running", coroutine.name));
            match coroutine.future.as_mut().poll(&mut cx) {
                Poll::Pending => {
                    self.event(format!(
                        "coroutine:pending name={} state=Suspended action=requeue",
                        coroutine.name
                    ));
                    self.queue.push_back(coroutine);
                }
                Poll::Ready(()) => {
                    self.event(format!(
                        "coroutine:finish name={} state=Finished",
                        coroutine.name
                    ));
                }
            }
        }

        self.event("runtime:run end; all coroutines finished");
    }

    fn event(&mut self, message: impl Into<String>) {
        (self.trace)(message.into());
    }
}

pub fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

pub struct YieldNow { yielded: bool }

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}

    fn wake_by_ref(self: &Arc<Self>) {}
}
