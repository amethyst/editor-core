use crate::systems::*;
use crate::types::IncomingComponent;
use crate::types::*;
use amethyst::core::{Result as BundleResult, SystemBundle};
use amethyst::ecs::{Component, DispatcherBuilder};
use amethyst::shred::Resource;
use crossbeam_channel::Receiver;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::net::UdpSocket;
use std::time::Duration;

/// Bundles all necessary systems for serializing all registered components and resources and
/// sending them to the editor.
pub struct SyncEditorBundle {
    send_interval: Duration,
    read_systems: Vec<Box<dyn RegisterReadSystem>>,
    write_systems: Vec<Box<dyn RegisterWriteSystem>>,
    sender: EditorConnection,
    receiver: Receiver<SerializedData>,
    component_map: ComponentMap,
    resource_map: ResourceMap,
    socket: UdpSocket,
}

/// Registers one or more components to be syncronized with the editor.
///
/// Helper macro for quickly registering multiple components at once. This wraps
/// calls to [`SyncEditorBundle::sync_component`], passing the stringified type
/// name as the identifier for the component.
///
/// [`SyncEditorBundle::sync_component`]: ./struct.SyncEditorBundle.html#method.sync_component
#[macro_export]
macro_rules! sync_components {
    ($bundle:ident, $( $component:ty ),* $(,)*) => {
        {
            $( $bundle.sync_component::<$component>(stringify!($component)); )*
        }
    };
}

/// Registers one or more components to be displayed as read-only in the editor.
///
/// Helper macro for quickly registering multiple components at once. This wraps
/// calls to [`SyncEditorBundle::read_component`], passing the stringified type
/// name as the identifier for the component.
///
/// [`SyncEditorBundle::read_component`]: ./struct.SyncEditorBundle.html#method.read_component
#[macro_export]
macro_rules! read_components {
    ($bundle:ident, $( $component:ty ),* $(,)*) => {
        {
            $( $bundle.read_component::<$component>(stringify!($component)); )*
        }
    };
}

/// Registers one or more resources to be synchronized with the editor.
///
/// Helper macro for quickly registering multiple resources at once. This wraps
/// calls to [`SyncEditorBundle::sync_resource`], passing the stringified type
/// name as the identifier for the resource.
///
/// [`SyncEditorBundle::sync_resource`]: ./struct.SyncEditorBundle.html#method.sync_resource
#[macro_export]
macro_rules! sync_resources {
    ($bundle:ident, $( $resource:ty ),* $(,)*) => {
        {
            $( $bundle.sync_resource::<$resource>(stringify!($resource)); )*
        }
    };
}

/// Registers one or more resources to be displayed as read-only in the editor.
///
/// Helper macro for quickly registering multiple resources at once. This wraps
/// calls to [`SyncEditorBundle::read_resource`], passing the stringified type
/// name as the identifier for the resource.
///
/// [`SyncEditorBundle::read_resource`]: ./struct.SyncEditorBundle.html#method.read_resource
#[macro_export]
macro_rules! read_resources {
    ($bundle:ident, $( $resource:ty ),* $(,)*) => {
        {
            $( $bundle.read_resource::<$resource>(stringify!($resource)); )*
        }
    };
}

impl SyncEditorBundle {
    /// Construct an empty bundle.
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket
            .set_nonblocking(true)
            .expect("Failed to make editor socket nonblocking");

        SyncEditorBundle {
            send_interval: Duration::from_millis(200),
            read_systems: Vec::new(),
            write_systems: Vec::new(),
            sender: EditorConnection::new(sender),
            receiver,
            component_map: HashMap::new(),
            resource_map: HashMap::new(),
            socket,
        }
    }

    /// Synchronize amethyst types.
    ///
    /// Currently only a small set is supported. This will be expanded in the future.
    pub fn sync_default_types(&mut self) {
        use amethyst::{
            controls::{FlyControlTag, HideCursor, WindowFocus},
            core::{GlobalTransform, Named, Transform},
            renderer::{AmbientColor, Camera, Light},
            ui::{MouseReactive, UiButton, UiText, UiTransform},
            utils::ortho_camera::CameraOrtho,
            utils::time_destroy::{DestroyAtTime, DestroyInTime},
        };

        sync_components!(
            self,
            Camera,
            CameraOrtho,
            DestroyAtTime,
            DestroyInTime,
            FlyControlTag,
            GlobalTransform,
            Light,
            MouseReactive,
            Named,
            Transform,
            UiButton,
            UiTransform,
        );
        read_components!(self, UiText);
        sync_resources!(self, AmbientColor, HideCursor);
        read_resources!(self, WindowFocus);
    }

    /// Register a component for synchronizing with the editor. This will result in a
    /// [`ReadComponentSystem`] being added.
    pub fn sync_component<C>(&mut self, name: &'static str)
    where
        C: Component + Serialize + DeserializeOwned + Send + Sync,
    {
        let read_component = ReadComponent::<C> {
            name,
            _marker: Default::default(),
        };

        let (sender, receiver) = crossbeam_channel::unbounded();
        self.component_map.insert(name, sender);
        let write_component = WriteComponent::<C> {
            name,
            receiver,
            _marker: Default::default(),
        };

        self.read_systems
            .push(Box::new(read_component) as Box<dyn RegisterReadSystem>);
        self.write_systems
            .push(Box::new(write_component) as Box<dyn RegisterWriteSystem>);
    }

    pub fn read_component<C>(&mut self, name: &'static str)
    where
        C: Component + Serialize + Send,
    {
        let read_component = ReadComponent::<C> {
            name,
            _marker: Default::default(),
        };
        self.read_systems
            .push(Box::new(read_component) as Box<dyn RegisterReadSystem>);
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
    pub fn sync_resource<R>(&mut self, name: &'static str)
    where
        R: Resource + Serialize + DeserializeOwned + Send + Sync,
    {
        let read_resource = ReadResource::<R> {
            name,
            _marker: Default::default(),
        };

        let (sender, receiver) = crossbeam_channel::unbounded();
        self.resource_map.insert(name, sender);
        let write_resource = WriteResource::<R> {
            name,
            receiver,
            _marker: Default::default(),
        };

        self.read_systems
            .push(Box::new(read_resource) as Box<dyn RegisterReadSystem>);
        self.write_systems
            .push(Box::new(write_resource) as Box<dyn RegisterWriteSystem>);
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
    pub fn read_resource<R>(&mut self, name: &'static str)
    where
        R: Resource + Serialize + Send,
    {
        let read_resource = ReadResource::<R> {
            name,
            _marker: Default::default(),
        };

        self.read_systems
            .push(Box::new(read_resource) as Box<dyn RegisterReadSystem>);
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
    pub fn send_interval(&mut self, send_interval: Duration) {
        self.send_interval = send_interval;
    }

    /// Retrieve a connection to send messages to the editor via the [`SyncEditorSystem`].
    pub(crate) fn connection(&self) -> EditorConnection {
        self.sender.clone()
    }
}

impl Default for SyncEditorBundle {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, 'b> SystemBundle<'a, 'b> for SyncEditorBundle {
    fn build(self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        // Create the receiver system, which will read any incoming messages from the
        // editor and pass them to the corresponding systems for applying changes to
        // components/resources/entities.
        let (entity_sender, entity_receiver) = crossbeam_channel::unbounded::<EntityMessage>();
        let receiver_system = EditorReceiverSystem::new(
            self.component_map.clone(),
            self.resource_map.clone(),
            entity_sender,
            self.socket.try_clone().unwrap(),
        );
        dispatcher.add(receiver_system, "editor_receiver_system", &[]);

        // Register the systems for each of the component/resource types that support
        // being edited at runtime. Internally these declare a dependency on the
        // editor receiver system.
        for write_system in self.write_systems {
            write_system.register(dispatcher);
        }

        // Register the system that applies entity changes (creates/destroys entities).
        // This must also depend on the editor reciever system so that it can apply
        // an entity changes specified by the editor.
        dispatcher.add(
            EntityHandlerSystem::new(entity_receiver),
            "entity_creator",
            &["editor_receiver_system"],
        );

        // Register the systems for serializing each of the component/resource types.
        for read_system in self.read_systems {
            read_system.register(dispatcher, &self.sender);
        }

        // Add a barrier to ensure that all of the
        dispatcher.add_barrier();

        let sender_system = EditorSenderSystem::from_channel(
            self.receiver,
            Duration::from_millis(200),
            self.socket,
        );
        dispatcher.add(sender_system, "editor_sender_system", &[]);

        Ok(())
    }
}

struct ReadComponent<T> {
    name: &'static str,
    _marker: PhantomData<T>,
}

struct ReadResource<T> {
    name: &'static str,
    _marker: PhantomData<T>,
}

struct WriteComponent<T> {
    name: &'static str,
    receiver: Receiver<IncomingComponent>,
    _marker: PhantomData<T>,
}

struct WriteResource<T> {
    name: &'static str,
    receiver: Receiver<serde_json::Value>,
    _marker: PhantomData<T>,
}

impl<T> RegisterReadSystem for ReadComponent<T>
where
    T: Component + Serialize + Send,
{
    fn register(
        self: Box<Self>,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        dispatcher.add(
            ReadComponentSystem::<T>::new(self.name, connection.clone()),
            "",
            &[],
        );
    }
}

impl<T> RegisterReadSystem for ReadResource<T>
where
    T: Resource + Serialize + Send,
{
    fn register(
        self: Box<Self>,
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
    ) {
        dispatcher.add(
            ReadResourceSystem::<T>::new(self.name, connection.clone()),
            "",
            &[],
        );
    }
}

impl<T> RegisterWriteSystem for WriteComponent<T>
where
    T: Component + Serialize + DeserializeOwned + Send + Sync,
{
    fn register(self: Box<Self>, dispatcher: &mut DispatcherBuilder) {
        dispatcher.add(
            WriteComponentSystem::<T>::new(self.name, self.receiver),
            "",
            &["editor_receiver_system"],
        );
    }
}

impl<T> RegisterWriteSystem for WriteResource<T>
where
    T: Resource + Serialize + DeserializeOwned + Send + Sync,
{
    fn register(self: Box<Self>, dispatcher: &mut DispatcherBuilder) {
        dispatcher.add(
            WriteResourceSystem::<T>::new(self.name, self.receiver),
            "",
            &["editor_receiver_system"],
        );
    }
}

trait RegisterReadSystem {
    fn register(self: Box<Self>, dispatcher: &mut DispatcherBuilder, connection: &EditorConnection);
}

trait RegisterWriteSystem {
    fn register(self: Box<Self>, dispatcher: &mut DispatcherBuilder);
}

#[cfg(test)]
mod test {
    use crate::SyncEditorBundle;
    use amethyst::renderer::{AmbientColor, Camera, Light};

    /// Tests that the various `sync_*` macros work without a trailing comma.
    #[test]
    fn no_trailing_comma() {
        let mut bundle = SyncEditorBundle::new();
        sync_components!(bundle, Light, Camera);
        read_components!(bundle, Light, Camera);
        sync_resources!(bundle, AmbientColor);
        read_resources!(bundle, AmbientColor);
    }

    /// Tests that the various `sync_*` macros work with a trailing comma.
    #[test]
    fn trailing_comma() {
        let mut bundle = SyncEditorBundle::new();
        sync_components!(bundle, Light, Camera,);
        read_components!(bundle, Light, Camera,);
        sync_resources!(bundle, AmbientColor,);
        read_resources!(bundle, AmbientColor,);
    }
}
