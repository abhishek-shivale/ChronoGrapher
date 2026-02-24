use async_trait::async_trait;
use chronographer::errors::{StandardCoreErrorsCG, TaskError};
use chronographer::prelude::*;
use chronographer::task::dependency::{TaskDependency};
use chronographer::task::{OnTaskEnd, Task, TaskFrame, TaskFrameContext, TaskScheduleImmediate};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct SimpleTaskFrame {
    should_succeed: Arc<AtomicBool>,
}

#[async_trait]
impl TaskFrame for SimpleTaskFrame {
    type Error = Box<dyn TaskError>;

    async fn execute(&self, _ctx: &TaskFrameContext) -> Result<(), Self::Error> {
        if self.should_succeed.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(Box::new(StandardCoreErrorsCG::TaskDependenciesUnresolved))
        }
    }
}


#[tokio::test]
async fn test_task_dependency() {
    let should_succeed = Arc::new(AtomicBool::new(true));
    let frame = SimpleTaskFrame {
        should_succeed: should_succeed.clone(),
    };

    let task = Box::leak(Box::new(Task::new(TaskScheduleImmediate, frame)));

    let dep: TaskDependency = TaskDependency::builder(task).build();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(
        !dep.is_resolved().await,
        "Task dependency should not be resolved initially"
    );

    let result: Option<Box<dyn TaskError>> = None;
    task.emit_hook_event::<OnTaskEnd>(&result.as_ref().map(|x| x.as_ref()))
        .await;

    assert!(
        dep.is_resolved().await,
        "Task dependency should be resolved after task succeeds"
    );

    dep.disable().await;
    assert!(!dep.is_enabled().await);
    dep.enable().await;
    assert!(dep.is_enabled().await);

    dep.unresolve().await;
    assert!(!dep.is_resolved().await);
    dep.resolve().await;
    assert!(dep.is_resolved().await);
}