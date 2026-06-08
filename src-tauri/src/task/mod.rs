//! Task domain model.

pub mod event;
pub mod state;

pub use event::{TaskEvent, TaskEventKind};
pub use state::{StepStatus, Task, TaskStatus, TaskStep};
