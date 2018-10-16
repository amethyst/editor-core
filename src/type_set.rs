use amethyst::ecs::prelude::*;
use amethyst::ecs::shred::Resource;
use serde::Serialize;
use std::marker::PhantomData;

use {EditorConnection, SyncComponentSystem, SyncResourceSystem};

/// Create a set of types, where the value is the stringified typename.
#[macro_export]
macro_rules! type_set {
    ( $($t:ty),* ) => {
        {
            let type_set = TypeSet::new();
            $(
                let type_set = type_set.with::<$t>(stringify!($t));
            )*
            type_set
        }
    };
}

/// A set of types with associated data.
///
/// `T` is essentially a tree built of 0-2 tuples.
pub struct TypeSet<T> {
    // Stored in left to right traversal order of the type tree.
    names: Vec<&'static str>,
    _phantom: PhantomData<T>,
}

impl TypeSet<()> {
    /// Construct an empty set.
    pub fn new() -> Self {
        TypeSet {
            names: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<T> TypeSet<T> {
    /// Insert a type.
    pub fn with<U>(mut self, value: &'static str) -> TypeSet<(T, (U,))> {
        self.names.push(value);
        TypeSet {
            names: self.names,
            _phantom: PhantomData,
        }
    }

    /// Insert each type in the given set.
    pub fn with_set<U>(mut self, set: &TypeSet<U>) -> TypeSet<(T, U)> {
        self.names.extend(&set.names);
        TypeSet {
            names: self.names,
            _phantom: PhantomData,
        }
    }
}

impl<T> TypeSet<T>
where
    T: ComponentSet,
{
    /// Create a component-synchronization system for each type in the set.
    pub(crate) fn create_component_sync_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        T::create_sync_systems(dispatcher, connection, &self.names);
    }
}

impl<T> TypeSet<T>
where
    T: ResourceSet,
{
    /// Create a resource-synchronization system for each type in the set.
    pub(crate) fn create_resource_sync_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        T::create_sync_systems(dispatcher, connection, &self.names);
    }
}

/// A type that groups component types.
///
/// This is an implementation detail used to construct synchronization systems.
pub trait ComponentSet {
    /// Create the synchronization systems.
    ///
    /// Their names are passed in the order they are inserted into the type set.
    /// Returns the number of systems created.
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize;
}

impl ComponentSet for () {
    fn create_sync_systems(
        _: &mut DispatcherBuilder,
        _: &EditorConnection,
        _: &[&'static str],
    ) -> usize {
        0
    }
}

impl<T> ComponentSet for (T,)
where
    T: Component + Serialize + Send,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        dispatcher.add(
            SyncComponentSystem::<T>::new(names[0], connection.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T, U> ComponentSet for (T, U)
where
    T: ComponentSet,
    U: ComponentSet,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        let idx = T::create_sync_systems(dispatcher, connection, names);
        idx + U::create_sync_systems(dispatcher, connection, &names[idx..])
    }
}

/// A type that groups resource types.
///
/// This is an implementation detail used to construct synchronization systems.
pub trait ResourceSet {
    /// Create the synchronization systems.
    ///
    /// Their names are passed in the order they are inserted into the type set.
    /// Returns the number of systems created.
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize;
}

impl ResourceSet for () {
    fn create_sync_systems(
        _: &mut DispatcherBuilder,
        _: &EditorConnection,
        _: &[&'static str],
    ) -> usize {
        0
    }
}

impl<T> ResourceSet for (T,)
where
    T: Resource + Serialize,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        dispatcher.add(
            SyncResourceSystem::<T>::new(names[0], connection.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T, U> ResourceSet for (T, U)
where
    T: ResourceSet,
    U: ResourceSet,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        let idx = T::create_sync_systems(dispatcher, connection, names);
        idx + U::create_sync_systems(dispatcher, connection, &names[idx..])
    }
}
