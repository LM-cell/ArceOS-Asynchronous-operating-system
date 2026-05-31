use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use super::trace::TraceLog;

pub struct YieldOnce {
    task: &'static str,
    point: usize,
    yielded: bool,
    trace: TraceLog,
}

impl YieldOnce {
    pub fn new(task: &'static str, point: usize, trace: TraceLog) -> Self {
        trace.record(format!(
            "future:create task={task} await_point={point} state=Created"
        ));
        Self {
            task,
            point,
            yielded: false,
            trace,
        }
    }
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.trace.record(format!(
            "future:poll task={} await_point={} state=Running yielded={}",
            self.task, self.point, self.yielded
        ));

        if self.yielded {
            self.trace.record(format!(
                "future:ready task={} await_point={} state=Ready",
                self.task, self.point
            ));
            Poll::Ready(())
        } else {
            self.yielded = true;
            self.trace.record(format!(
                "future:pending task={} await_point={} state=Pending",
                self.task, self.point
            ));
            cx.waker().wake_by_ref();
            self.trace.record(format!(
                "future:wake task={} await_point={} executor_state=Ready",
                self.task, self.point
            ));
            Poll::Pending
        }
    }
}
