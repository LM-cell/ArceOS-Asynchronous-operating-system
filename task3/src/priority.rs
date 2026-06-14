use std::collections::VecDeque;

/// Priority used by the tiny scheduler demo.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Priority {
    High,
    Low,
}

type Job = Box<dyn FnOnce(&mut Vec<&'static str>) + Send + 'static>;

/// A minimal cooperative priority scheduler used by tests.
///
/// This is not a replacement for Tokio or may. It turns the report's priority
/// scheduling extension into executable behavior: high-priority jobs drain
/// before low-priority jobs.
#[derive(Default)]
pub struct PriorityScheduler {
    high: VecDeque<Job>,
    low: VecDeque<Job>,
}

impl PriorityScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn<F>(&mut self, priority: Priority, job: F)
    where
        F: FnOnce(&mut Vec<&'static str>) + Send + 'static,
    {
        match priority {
            Priority::High => self.high.push_back(Box::new(job)),
            Priority::Low => self.low.push_back(Box::new(job)),
        }
    }

    pub fn run_all(mut self) -> Vec<&'static str> {
        let mut completion_order = Vec::new();

        while let Some(job) = self.high.pop_front() {
            job(&mut completion_order);
        }

        while let Some(job) = self.low.pop_front() {
            job(&mut completion_order);
        }

        completion_order
    }
}

pub fn priority_demo_completion_order() -> Vec<&'static str> {
    let mut scheduler = PriorityScheduler::new();

    scheduler.spawn(Priority::Low, |order| order.push("low-1"));
    scheduler.spawn(Priority::High, |order| order.push("high-1"));
    scheduler.spawn(Priority::Low, |order| order.push("low-2"));
    scheduler.spawn(Priority::High, |order| order.push("high-2"));

    scheduler.run_all()
}

/// One task in the preemptive priority scheduling simulation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PreemptiveTask {
    pub name: &'static str,
    pub priority: Priority,
    pub ready_at_tick: u64,
    pub remaining_ticks: u64,
}

impl PreemptiveTask {
    pub fn new(
        name: &'static str,
        priority: Priority,
        ready_at_tick: u64,
        work_ticks: u64,
    ) -> Self {
        assert!(work_ticks > 0, "work_ticks must be positive");

        Self {
            name,
            priority,
            ready_at_tick,
            remaining_ticks: work_ticks,
        }
    }
}

/// One dispatch decision made by the preemptive scheduler.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DispatchEvent {
    pub tick: u64,
    pub task: &'static str,
    pub priority: Priority,
}

/// Full trace returned by the preemptive scheduling simulation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PreemptiveTrace {
    pub dispatch_events: Vec<DispatchEvent>,
    pub completion_order: Vec<&'static str>,
}

impl PreemptiveTrace {
    pub fn dispatch_order(&self) -> Vec<&'static str> {
        self.dispatch_events
            .iter()
            .map(|event| event.task)
            .collect()
    }
}

/// A tiny time-sliced preemptive priority scheduler.
///
/// Each tick runs at most one unit of work. Ready high-priority tasks are always
/// selected before low-priority tasks. A low-priority task that still has work
/// left is requeued, so a high-priority task becoming ready on the next tick can
/// preempt it.
#[derive(Default)]
pub struct PreemptivePriorityScheduler {
    pending: Vec<PreemptiveTask>,
    high: VecDeque<PreemptiveTask>,
    low: VecDeque<PreemptiveTask>,
}

impl PreemptivePriorityScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, task: PreemptiveTask) {
        self.pending.push(task);
        self.pending
            .sort_by_key(|task| (task.ready_at_tick, priority_rank(task.priority)));
    }

    pub fn run(mut self) -> PreemptiveTrace {
        let mut tick = 0;
        let mut dispatch_events = Vec::new();
        let mut completion_order = Vec::new();

        while self.has_work() {
            self.release_ready_tasks(tick);

            let Some(mut task) = self.pop_next_task() else {
                tick = self.next_ready_tick(tick);
                continue;
            };

            dispatch_events.push(DispatchEvent {
                tick,
                task: task.name,
                priority: task.priority,
            });

            task.remaining_ticks -= 1;
            if task.remaining_ticks == 0 {
                completion_order.push(task.name);
            } else {
                self.push_ready_task(task);
            }

            tick += 1;
        }

        PreemptiveTrace {
            dispatch_events,
            completion_order,
        }
    }

    fn has_work(&self) -> bool {
        !(self.pending.is_empty() && self.high.is_empty() && self.low.is_empty())
    }

    fn release_ready_tasks(&mut self, tick: u64) {
        let mut index = 0;
        while index < self.pending.len() {
            if self.pending[index].ready_at_tick <= tick {
                let task = self.pending.remove(index);
                self.push_ready_task(task);
            } else {
                index += 1;
            }
        }
    }

    fn push_ready_task(&mut self, task: PreemptiveTask) {
        match task.priority {
            Priority::High => self.high.push_back(task),
            Priority::Low => self.low.push_back(task),
        }
    }

    fn pop_next_task(&mut self) -> Option<PreemptiveTask> {
        self.high.pop_front().or_else(|| self.low.pop_front())
    }

    fn next_ready_tick(&self, current_tick: u64) -> u64 {
        self.pending
            .iter()
            .map(|task| task.ready_at_tick)
            .filter(|&ready_at| ready_at > current_tick)
            .min()
            .unwrap_or(current_tick + 1)
    }
}

pub fn preemptive_priority_demo_trace() -> PreemptiveTrace {
    let mut scheduler = PreemptivePriorityScheduler::new();

    scheduler.spawn(PreemptiveTask::new("low-long", Priority::Low, 0, 3));
    scheduler.spawn(PreemptiveTask::new("high-short", Priority::High, 1, 1));

    scheduler.run()
}

fn priority_rank(priority: Priority) -> u8 {
    match priority {
        Priority::High => 0,
        Priority::Low => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_priority_tasks_complete_before_low_priority_tasks() {
        let order = priority_demo_completion_order();
        let first_low = order.iter().position(|task| task.starts_with("low")).unwrap();
        let last_high = order.iter().rposition(|task| task.starts_with("high")).unwrap();

        assert!(last_high < first_low, "completion order was {order:?}");
    }

    #[test]
    fn high_priority_task_preempts_running_low_priority_task() {
        let trace = preemptive_priority_demo_trace();

        assert_eq!(trace.dispatch_order(), vec!["low-long", "high-short", "low-long", "low-long"]);
        assert_eq!(trace.completion_order, vec!["high-short", "low-long"]);
    }
}
