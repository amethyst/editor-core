use amethyst::ecs::prelude::*;
use amethyst::ecs::shred::Resource;

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;

use crate::systems::{
    ReadComponentSystem, ReadResourceSystem, WriteComponentSystem, WriteResourceSystem,
};
use types::{ComponentMap, EditorConnection, ResourceMap};

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
#[derive(Default)]
pub struct TypeSet<T> {
    // Stored in left to right traversal order of the type tree.
    pub names: Vec<&'static str>,
    _phantom: PhantomData<T>,
}

impl TypeSet<()> {
    /// Construct an empty set.
    pub fn new() -> Self {
        Default::default()
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
    T: ReadComponentSet,
{
    /// Create a component-synchronization system for each type in the set.
    pub(crate) fn create_component_read_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        T::create_sync_systems(dispatcher, connection, &self.names);
    }
}

impl<T> TypeSet<T>
where
    T: WriteComponentSet,
{
    /// Create a component-synchronization system for each type in the set.
    pub(crate) fn create_component_write_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        map: ComponentMap,
    ) {
        T::create_sync_systems(dispatcher, &self.names, map);
    }
}

impl<T> TypeSet<T>
where
    T: ReadResourceSet,
{
    /// Create a resource-synchronization system for each type in the set.
    pub(crate) fn create_resource_read_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        T::create_read_systems(dispatcher, connection, &self.names);
    }
}

impl<T> TypeSet<T>
where
    T: WriteResourceSet,
{
    pub(crate) fn create_resource_write_systems(
        &self,
        dispatcher: &mut DispatcherBuilder,
        map: ResourceMap,
    ) {
        T::create_write_systems(dispatcher, &self.names, map);
    }
}

/// A type that groups component types.
///
/// This is an implementation detail used to construct synchronization systems.
pub trait ReadComponentSet {
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

pub trait WriteComponentSet {
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ComponentMap,
    ) -> usize;
}

impl ReadComponentSet for () {
    fn create_sync_systems(
        _: &mut DispatcherBuilder,
        _: &EditorConnection,
        _: &[&'static str],
    ) -> usize {
        0
    }
}

impl WriteComponentSet for () {
    fn create_sync_systems(
        _: &mut DispatcherBuilder,
        _: &[&'static str],
        _: ComponentMap,
    ) -> usize {
        0
    }
}

impl<T> ReadComponentSet for (T,)
where
    T: Component + Serialize + Send,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        dispatcher.add(
            ReadComponentSystem::<T>::new(names[0], connection.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T> WriteComponentSet for (T,)
where
    T: Component + DeserializeOwned + Send + Sync,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ComponentMap,
    ) -> usize {
        dispatcher.add(
            WriteComponentSystem::<T>::new(names[0], map[names[0]].1.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T, U> ReadComponentSet for (T, U)
where
    T: ReadComponentSet,
    U: ReadComponentSet,
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

impl<T, U> WriteComponentSet for (T, U)
where
    T: WriteComponentSet,
    U: WriteComponentSet,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ComponentMap,
    ) -> usize {
        let idx = T::create_sync_systems(dispatcher, names, map.clone());
        idx + U::create_sync_systems(dispatcher, &names[idx..], map)
    }
}

/// A type that groups resource types.
///
/// This is an implementation detail used to construct synchronization systems.
pub trait ReadResourceSet {
    /// Create the synchronization systems.
    ///
    /// Their names are passed in the order they are inserted into the type set.
    /// Returns the number of systems created.
    fn create_read_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize;
}

pub trait WriteResourceSet {
    fn create_write_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ResourceMap,
    ) -> usize;
}

impl ReadResourceSet for () {
    fn create_read_systems(
        _: &mut DispatcherBuilder,
        _: &EditorConnection,
        _: &[&'static str],
    ) -> usize {
        0
    }
}

impl WriteResourceSet for () {
    fn create_write_systems(
        _: &mut DispatcherBuilder,
        _: &[&'static str],
        _: ResourceMap,
    ) -> usize {
        0
    }
}

impl<T> ReadResourceSet for (T,)
where
    T: Resource + Serialize,
{
    fn create_read_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        dispatcher.add(
            ReadResourceSystem::<T>::new(names[0], connection.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T> WriteResourceSet for (T,)
where
    T: Resource + DeserializeOwned,
{
    fn create_write_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ResourceMap,
    ) -> usize {
        dispatcher.add(
            WriteResourceSystem::<T>::new(names[0], map[names[0]].1.clone()),
            "",
            &[],
        );
        1
    }
}

impl<T, U> ReadResourceSet for (T, U)
where
    T: ReadResourceSet,
    U: ReadResourceSet,
{
    fn create_read_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
    ) -> usize {
        let idx = T::create_read_systems(dispatcher, connection, names);
        idx + U::create_read_systems(dispatcher, connection, &names[idx..])
    }
}

impl<T, U> WriteResourceSet for (T, U)
where
    T: WriteResourceSet,
    U: WriteResourceSet,
{
    fn create_write_systems(
        dispatcher: &mut DispatcherBuilder,
        names: &[&'static str],
        map: ResourceMap,
    ) -> usize {
        let idx = T::create_write_systems(dispatcher, names, map.clone());
        idx + U::create_write_systems(dispatcher, &names[idx..], map)
    }
}
