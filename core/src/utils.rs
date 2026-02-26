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

/// TaskIdentifier trait use for defining unique indentifier for example [Uuid], integers, strings, and generally any kind of identifier format the user can use which suits their needs.
///
/// # Semantics
/// Implementors must provide a way to generate unique indentifier for task.
///
/// # Required Subtrait(s)
/// TaskIdentifier requires the following subtraits in order to be implemented:
/// - `Debug` - For displaying the ID.
/// - `Clone` - For fully cloning the ID.
/// - `PartialEq` - For comparing 2 IDs and checking if they are equal.
/// - `Eq` - For ensuring the comparison applies in both directions.
/// - `Hash` - For producing a hash from the ID.
///
/// TaskIdentifier also requires `Send` + `Sync` + `'static`.
///
/// # Required Method(s)
/// - [`generate`](TaskIdentifier::generate) - Produces a new unique identifier.
///
/// # Implementation(s)
/// - [`Uuid`] - Generates a random UUID v4 via [`Uuid::new_v4`].
///
/// # Object Safety / Dynamic Dispatching
/// This trait is **not** object-safe due to the `Clone` and `Sized` requirement from its supertraits.
///
/// # Example(s)
///
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
/// let task_id = TaskId::generate();
/// let id: Uuid = Uuid::generate();
/// 
/// assert_ne!(id, task_id.0);
/// ```
/// # See Also
/// - [`Uuid`] - The default implementation, generating random v4 UUIDs.
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
