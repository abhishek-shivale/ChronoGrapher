use async_trait::async_trait;
use chronographer::errors::{StandardCoreErrorsCG, TaskError};
use chronographer::prelude::*;
use chronographer::task::dependency::{TaskDependency, TaskResolveFailureOnly};
use chronographer::task::{OnTaskEnd, Task, TaskFrame, TaskFrameContext, TaskScheduleImmediate};
use std::num::NonZeroU64;
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
async fn test_flag_resolution() {
    let flag = Arc::new(AtomicBool::new(false));
    let dep = FlagDependency::new(flag.clone());

    assert!(
        !dep.is_resolved().await,
        "Dependency should be unresolved initially"
    );

    dep.resolve().await;
    assert!(
        dep.is_resolved().await,
        "Dependency should be resolved after calling resolve()"
    );

    dep.unresolve().await;
    assert!(
        !dep.is_resolved().await,
        "Dependency should be unresolved after calling unresolve()"
    );

    assert!(
        dep.is_enabled().await,
        "Dependency should be enabled by default"
    );
    dep.disable().await;
    assert!(
        !dep.is_enabled().await,
        "Dependency should be disabled after disable()"
    );
    dep.enable().await;
    assert!(
        dep.is_enabled().await,
        "Dependency should be enabled after enable()"
    );
}

#[tokio::test]
async fn test_logical_combinations() {
    let f1 = Arc::new(AtomicBool::new(false));
    let f2 = Arc::new(AtomicBool::new(false));

    let d1 = FlagDependency::new(f1.clone());
    let d2 = FlagDependency::new(f2.clone());

    let and_dep = LogicalDependency::and(d1, d2);
    assert!(
        !and_dep.is_resolved().await,
        "AND dependency should not be resolved initially"
    );

    f1.store(true, Ordering::Relaxed);
    assert!(
        !and_dep.is_resolved().await,
        "AND dependency should remain unresolved when only one input is resolved"
    );

    f2.store(true, Ordering::Relaxed);
    assert!(
        and_dep.is_resolved().await,
        "AND dependency should be resolved when both inputs are resolved"
    );

    let d1 = FlagDependency::new(f1.clone());
    let d2 = FlagDependency::new(f2.clone());
    f1.store(false, Ordering::Relaxed);
    f2.store(false, Ordering::Relaxed);

    let or_dep = LogicalDependency::or(d1, d2);
    assert!(
        !or_dep.is_resolved().await,
        "OR dependency should not be resolved initially"
    );

    f1.store(true, Ordering::Relaxed);
    assert!(
        or_dep.is_resolved().await,
        "OR dependency should be resolved when at least one input is resolved"
    );

    let d1 = FlagDependency::new(f1.clone());
    f1.store(true, Ordering::Relaxed);
    let not_dep = LogicalDependency::not(d1);
    assert!(
        !not_dep.is_resolved().await,
        "NOT dependency should not be resolved if its input is resolved"
    );

    f1.store(false, Ordering::Relaxed);
    assert!(
        not_dep.is_resolved().await,
        "NOT dependency should be resolved if its input is not resolved"
    );
}

#[tokio::test]
async fn test_dynamic_dependency() {
    let dep = DynamicDependency::new(|| async { true });
    assert!(
        dep.is_resolved().await,
        "Dynamic dependency should resolve to true based on its future"
    );
    assert!(
        !dep.is_enabled().await,
        "Dynamic dependency should be disabled by default"
    );

    dep.enable().await;
    assert!(
        dep.is_enabled().await,
        "Dynamic dependency should be enabled after calling enable()"
    );

    dep.disable().await;
    assert!(
        !dep.is_enabled().await,
        "Dynamic dependency should be disabled after calling disable()"
    );

    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = flag.clone();

    let stateful_dep = DynamicDependency::new(move || {
        let f = flag_clone.clone();
        async move { f.load(Ordering::Relaxed) }
    });

    assert!(
        !stateful_dep.is_resolved().await,
        "Stateful dynamic dependency should initially resolve to false"
    );

    flag.store(true, Ordering::Relaxed);
    assert!(
        stateful_dep.is_resolved().await,
        "Stateful dynamic dependency should resolve to true after underlying state changes"
    );
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

#[tokio::test]
async fn test_task_dependency_failure_only() {
    let should_succeed = Arc::new(AtomicBool::new(false));
    let frame = SimpleTaskFrame {
        should_succeed: should_succeed.clone(),
    };

    let task = Box::leak(Box::new(Task::new(TaskScheduleImmediate, frame)));

    let dep: TaskDependency = TaskDependency::builder(task)
        .resolve_behavior(TaskResolveFailureOnly)
        .minimum_runs(NonZeroU64::new(1).unwrap())
        .build();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let success_result: Option<Box<dyn TaskError>> = None;
    task.emit_hook_event::<OnTaskEnd>(&success_result.as_ref().map(|x| x.as_ref()))
        .await;

    assert!(
        !dep.is_resolved().await,
        "Task dependency should not be resolved after task succeeds (failure only)"
    );

    let err_result: Option<Box<dyn TaskError>> =
        Some(Box::new(StandardCoreErrorsCG::TaskDependenciesUnresolved));
    task.emit_hook_event::<OnTaskEnd>(&err_result.as_ref().map(|x| x.as_ref()))
        .await;

    assert!(
        dep.is_resolved().await,
        "Task dependency should be resolved after task fails"
    );
}
