//! Library code for the execution-flow stack and memory experiment.
//!
//! The model runners are separate from the CLI so tests can exercise scheduler
//! behavior directly, and so the experiment can be reused in other environments.

pub mod metrics;
pub mod models;
pub mod priority;
pub mod stack;

pub use metrics::{ExperimentRecord, LabeledSample};
pub use models::{run_async_future, run_green_thread, run_os_thread, ExecutionModel, RunConfig};
