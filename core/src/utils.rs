use std::fmt::Debug;
use std::hash::Hash;
use uuid::Uuid;

#[macro_export]
macro_rules! define_event_group {
    ($(#[$($attrss:tt)*])* $name: ident, $($events: ident),*) => {
        $(#[$($attrss)*])*
        pub trait $name: TaskHookEvent {}

        $(
            impl $name for $events {}
        )*
    };

    ($(#[$($attrss:tt)*])* $name: ident, $payload: ty | $($events: ident),*) => {
        $(#[$($attrss)*])*
        pub trait $name<'a>: TaskHookEvent<Payload<'a> = $payload> {}

        $(
            impl<'a> $name<'a> for $events {}
        )*
    };
}

#[macro_export]
macro_rules! define_event {
    ($(#[$($attrss:tt)*])* $name: ident, $payload: ty) => {
        $(#[$($attrss)*])*
        #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name;

        impl TaskHookEvent for $name {
            type Payload<'a> = $payload where Self: 'a;
            const EVENT_ID: &'static str = concat!("chronographer_core#", stringify!($name));
        }
    };
}

/// [`TaskIdentifier`] trait use for defining unique indentifier for example UUID, integers, strings, and generally any kind of identifier format the user can use which suits their needs.
/// The identifier is used internally in "[`Scheduler`] Land" to hold references to tasks with a simple representation.
///
/// Specifically its used in the [`SchedulerTaskStore`] internally. Identifiers can be configured via the [`SchedulerConfig`] trait, different [`Schedulers`] may have different [`TaskIdentifiers`]
/// defined via their configuration.
///
/// # Semantics
/// Implementors must provide a way to generate unique indentifier for task via the [`generate`](TaskIdentifier::generate) method as listed in the trait itself.
///
/// # Required Subtrait(s)
/// [`TaskIdentifier`] requires the following subtraits in order to be implemented:
/// - ``Debug`` - For displaying the ID.
/// - ``Clone`` - For fully cloning the ID.
/// - ``PartialEq`` - For comparing 2 IDs and checking if they are equal.
/// - ``Eq`` - For ensuring the comparison applies in both directions.
/// - ``Hash`` - For producing a hash from the ID.
///
/// [`TaskIdentifier`] also requires `Send` + `Sync` + `'static`.
///
/// # Required Method(s)
/// The [`TaskIdentifier`] trait requires developers to implement the [`generate`](TaskIdentifier::generate) method, which produces a new unique identifier
/// per call.
///
/// # Implementation(s)
/// The main implementor inside the core is [`Uuid`] which generates a random UUID v4 via using [`Uuid::new_v4`].
///
/// # Object Safety / Dynamic Dispatching
/// This trait is **not** object-safe due to the `Clone` and more specifically the `Sized` supertrait requirement.
///
/// # Example(s)
/// ```
/// use uuid::Uuid;
/// use chronographer::utils::TaskIdentifier;
/// 
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// struct TaskId(Uuid);
/// impl TaskIdentifier for TaskId {
///     fn generate() -> Self {
///         TaskId(Uuid::new_v4())
///     }
/// }
/// 
/// let task_id1 = TaskId::generate();
/// let task_id2 = TaskId::generate();
/// 
/// // Unequal, as they are unique entries
/// assert_ne!(task_id1, task_id2);
///
/// fn calculate_hash<T: Hash>(t: &T) -> u64 {
///     let mut s = DefaultHasher::new();
////    t.hash(&mut s);
///     s.finish()
/// }
///
/// // They produce different hashes (since they are unique)
/// assert_ne!(calculate_hash(&task_id1), calculate_hash(&task_id2));
/// ```
/// # See Also
/// - [`Uuid`] - The default implementation, generating random v4 UUIDs.
/// - [`SchedulerConfig`] - One of configuration parameters over lots of others.
/// - [`Scheduler`] - The interface around the store using the identifier.
/// - [`SchedulerTaskStore`] - Manages linking identifiers to tasks.
/// - [`Task`](crate::task::Task) - The primary consumer of task identifiers.

pub trait TaskIdentifier:
    'static + Debug + Clone + Eq + PartialEq<Self> + Hash + Send + Sync
{
    fn generate() -> Self;
}

impl TaskIdentifier for Uuid {
    fn generate() -> Self {
        Uuid::new_v4()
    }
}
