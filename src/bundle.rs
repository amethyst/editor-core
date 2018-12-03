use amethyst::core::{Result as BundleResult, SystemBundle};
use amethyst::ecs::{Component, DispatcherBuilder};
use amethyst::shred::Resource;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::Duration;
use systems::*;
use type_set::*;
use types::{ComponentMap, EditorConnection, EntityMessage, ResourceMap};

/// Bundles all necessary systems for serializing all registered components and resources and
/// sending them to the editor.
pub struct SyncEditorBundle<T, U, V, W>
where
    T: ReadComponentSet,
    U: ReadComponentSet + WriteComponentSet,
    V: ReadResourceSet,
    W: ReadResourceSet + WriteResourceSet,
{
    send_interval: Duration,
    read_components: TypeSet<T>,
    write_components: TypeSet<U>,
    read_resources: TypeSet<V>,
    write_resources: TypeSet<W>,
    sender: EditorConnection,
    component_map: ComponentMap,
    resource_map: ResourceMap,
    socket: UdpSocket,
}

impl SyncEditorBundle<(), (), (), ()> {
    /// Construct an empty bundle.
    pub fn new() -> Self {
        let (sender, _) = crossbeam_channel::unbounded();
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket
            .set_nonblocking(true)
            .expect("Failed to make editor socket nonblocking");

        SyncEditorBundle {
            send_interval: Duration::from_millis(200),
            read_components: TypeSet::new(),
            write_components: TypeSet::new(),
            read_resources: TypeSet::new(),
            write_resources: TypeSet::new(),
            sender: EditorConnection::new(sender),
            component_map: HashMap::new(),
            resource_map: HashMap::new(),
            socket,
        }
    }
}

impl Default for SyncEditorBundle<(), (), (), ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, U, V, W> SyncEditorBundle<T, U, V, W>
where
    T: ReadComponentSet,
    U: ReadComponentSet + WriteComponentSet,
    V: ReadResourceSet,
    W: ReadResourceSet + WriteResourceSet,
{
    /// Synchronize amethyst types.
    ///
    /// Currently only a small set is supported. This will be expanded in the future.
    pub fn sync_default_types(
        mut self,
    ) -> SyncEditorBundle<
        impl ReadComponentSet,
        impl ReadComponentSet + WriteComponentSet,
        impl ReadResourceSet,
        impl ReadResourceSet + WriteResourceSet,
    > {
        use amethyst::core::{GlobalTransform, Transform};
        use amethyst::renderer::{AmbientColor, Camera, Light};

        let read_components = type_set![];
        let write_components = type_set![Light, Camera, Transform, GlobalTransform];
        let read_resources = type_set![];
        let write_resources = type_set![AmbientColor];

        for name in &write_components.names {
            self.component_map
                .insert(name, crossbeam_channel::unbounded());
        }

        for name in &write_resources.names {
            self.resource_map
                .insert(name, crossbeam_channel::unbounded());
        }

        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components.with_set(&read_components),
            write_components: self.write_components.with_set(&write_components),
            read_resources: self.read_resources.with_set(&read_resources),
            write_resources: self.write_resources.with_set(&write_resources),
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Register a component for synchronizing with the editor. This will result in a
    /// [`ReadComponentSystem`] being added.
    pub fn sync_component<C>(
        mut self,
        name: &'static str,
    ) -> SyncEditorBundle<T, impl ReadComponentSet + WriteComponentSet, V, W>
    where
        C: Component + Serialize + DeserializeOwned + Send + Sync,
    {
        self.component_map
            .insert(name, crossbeam_channel::unbounded());

        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components.with::<C>(name),
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Register a set of components for synchronizing with the editor. This will result
    /// in a [`ReadComponentSystem`] being added for each component type in the set.
    pub fn sync_components<C>(
        mut self,
        set: &TypeSet<C>,
    ) -> SyncEditorBundle<T, impl ReadComponentSet + WriteComponentSet, V, W>
    where
        C: ReadComponentSet + WriteComponentSet,
    {
        for name in &set.names {
            self.component_map
                .insert(name, crossbeam_channel::unbounded());
        }

        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components.with_set(set),
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    pub fn read_component<C>(
        self,
        name: &'static str,
    ) -> SyncEditorBundle<impl ReadComponentSet, U, V, W>
    where
        C: Component + Serialize + Send + Sync,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components.with::<C>(name),
            write_components: self.write_components,
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    pub fn read_components<C>(
        self,
        set: &TypeSet<C>,
    ) -> SyncEditorBundle<impl ReadComponentSet, U, V, W>
    where
        C: ReadComponentSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components.with_set(set),
            write_components: self.write_components,
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Registers a resource type to be synchronized with the editor.
    ///
    /// At runtime, the state data for `R` will be sent to the editor for viewing and debugging.
    /// The editor will also be able to send back changes to the resource's data, which will
    /// automatically be applied to the local world state.
    ///
    /// It is safe to register a resource type for the editor even if it's not also going to be
    /// registered in the world. A warning will be emitted at runtime notifing that the resource
    /// won't appear in the editor, however it will not otherwise be treated as an error.
    pub fn sync_resource<R>(
        mut self,
        name: &'static str,
    ) -> SyncEditorBundle<T, U, V, impl ReadResourceSet + WriteResourceSet>
    where
        R: Resource + Serialize + DeserializeOwned,
    {
        self.resource_map
            .insert(name, crossbeam_channel::unbounded());

        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components,
            read_resources: self.read_resources,
            write_resources: self.write_resources.with::<R>(name),
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Registers a set of resource types to be synchronized with the editor.
    ///
    /// At runtime, the state data for the resources in `R` will be sent to the editor for
    /// viewing and debugging. The editor will also be able to send back changes to the
    /// resource's data, which will automatically be applied to the local world state.
    pub fn sync_resources<R>(
        mut self,
        set: &TypeSet<R>,
    ) -> SyncEditorBundle<T, U, V, impl ReadResourceSet + WriteResourceSet>
    where
        R: ReadResourceSet + WriteResourceSet,
    {
        for name in &set.names {
            self.resource_map
                .insert(name, crossbeam_channel::unbounded());
        }

        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components,
            read_resources: self.read_resources,
            write_resources: self.write_resources.with_set(set),
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Registers a resource to be sent to the editor as read-only data.
    ///
    /// At runtime, the state data for `R` will be sent to the editor for viewing, however
    /// the editor will not be able to send back changes. If you would like to be able to
    /// edit `R` in the editor, you can [implement `DeserializeOwned`] for it and then use
    /// [`sync_resource`] to register it as read-write data.
    ///
    /// [implement `DeserializeOwned`]: https://serde.rs/derive.html
    /// [`sync_resource`]: #method.sync_resource
    pub fn read_resource<R>(
        self,
        name: &'static str,
    ) -> SyncEditorBundle<T, U, impl ReadResourceSet, W>
    where
        R: Resource + Serialize,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components,
            read_resources: self.read_resources.with::<R>(name),
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Registers a set of resources to be sent to the editor as read-only data.
    ///
    /// At runtime, the state data for the resources in `R` will be sent to the editor
    /// for viewing, however the editor will not be able to send back changes. If you
    /// would like to be able to edit any of the resources in `R` in the editor, you
    /// can [implement `DeserializeOwned`] for them and then use [`sync_resources`] to
    /// register them as read-write data.
    ///
    /// [implement `DeserializeOwned`]: https://serde.rs/derive.html
    /// [`sync_resources`]: #method.sync_resources
    pub fn read_resources<R>(
        self,
        set: &TypeSet<R>,
    ) -> SyncEditorBundle<T, U, impl ReadResourceSet, W>
    where
        R: ReadResourceSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            read_components: self.read_components,
            write_components: self.write_components,
            read_resources: self.read_resources.with_set(set),
            write_resources: self.write_resources,
            sender: self.sender,
            component_map: self.component_map,
            resource_map: self.resource_map,
            socket: self.socket,
        }
    }

    /// Sets the interval at which the current game state will be sent to the editor.
    ///
    /// In order to reduce the amount of work the editor has to do to keep track of the latest
    /// game state, the rate at which the game state is sent can be reduced. This defaults to
    /// sending updated data every 200 ms. Setting this to 0 will ensure that data is sent every
    /// frame.
    ///
    /// Note that log output is sent every frame regardless of this interval, the interval only
    /// controls how often the game's state is sent.
    pub fn send_interval(mut self, send_interval: Duration) -> SyncEditorBundle<T, U, V, W> {
        self.send_interval = send_interval;
        self
    }

    /// Retrieve a connection to send messages to the editor via the [`SyncEditorSystem`].
    pub fn get_connection(&self) -> EditorConnection {
        self.sender.clone()
    }
}

impl<'a, 'b, T, U, V, W> SystemBundle<'a, 'b> for SyncEditorBundle<T, U, V, W>
where
    T: ReadComponentSet,
    U: ReadComponentSet + WriteComponentSet,
    V: ReadResourceSet,
    W: ReadResourceSet + WriteResourceSet,
{
    fn build(self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        let (entity_sender, entity_receiver) = crossbeam_channel::unbounded::<EntityMessage>();
        let input_system = EditorInputSystem::new(
            self.component_map.clone(),
            self.resource_map.clone(),
            entity_sender,
            self.socket.try_clone().ok().unwrap(),
        );
        dispatcher.add(input_system, "editor_input_system", &[]);

        let (c, r) = crossbeam_channel::unbounded();
        let connection = EditorConnection::new(c);

        // All systems must have finished serializing before it can be send to the editor.
        dispatcher.add_barrier();
        self.write_components
            .create_component_write_systems(dispatcher, self.component_map);
        self.write_resources
            .create_resource_write_systems(dispatcher, self.resource_map);
        let entity_handler = EntityHandlerSystem::new(entity_receiver);
        dispatcher.add(entity_handler, "entity_creator", &[]);

        // All systems must have finished editing data before syncing can start.
        dispatcher.add_barrier();
        self.read_components
            .create_component_read_systems(dispatcher, &connection);
        self.write_components
            .create_component_read_systems(dispatcher, &connection);
        self.read_resources
            .create_resource_read_systems(dispatcher, &connection);
        self.write_resources
            .create_resource_read_systems(dispatcher, &connection);

        let sync_system = SyncEditorSystem::from_channel(
            r,
            Duration::from_millis(200),
            self.socket.try_clone().ok().unwrap(),
        );
        dispatcher.add_barrier();
        dispatcher.add(sync_system, "sync_editor_system", &[]);

        Ok(())
    }
}
