//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll also need to create a
//! [`SyncComponentSystem`] or [`SyncResourceSystem`] for each component and resource that you want
//! to visualize. It is possible to automatically create these systems by creating a
//! [`SyncEditorBundle`] and registering each component and resource on it instead.
//!
//! # Example
//!
//! ```
//! extern crate amethyst;
//! extern crate amethyst_editor_sync;
//! #[macro_use]
//! extern crate serde;
//!
//! use amethyst::core::Transform;
//! use amethyst::ecs::*;
//! use amethyst::prelude::*;
//! use amethyst_editor_sync::*;
//!
//! # fn main() -> Result<(), amethyst::Error> {
//! // Specify every component that you want to view in the editor.
//! let components = type_set![MyComponent];
//! // Do the same for your resources.
//! let resources = type_set![MyResource];
//!
//! // Create a SyncEditorBundle which will create all necessary systems to send the components
//! // to the editor.
//! let editor_sync_bundle = SyncEditorBundle::new()
//!     // Register the default types from the engine.
//!     .sync_default_types()
//!     // Register the components and resources specified above.
//!     .sync_components(&components)
//!     .sync_resources(&resources);
//!
//! let game_data = GameDataBuilder::default()
//!     .with_bundle(editor_sync_bundle)?;
//! # Ok(())
//! # }
//!
//! // Make sure you enable serialization for your custom components and resources!
//! #[derive(Serialize, Deserialize)]
//! struct MyComponent {
//!     foo: usize,
//!     bar: String,
//! }
//!
//! impl Component for MyComponent {
//!     type Storage = DenseVecStorage<Self>;
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyResource {
//!     baz: usize,
//! }
//! ```
//!
//! # Usage
//! First, specify the components and resources that you want to see in the editor using the
//! [`type_set!`] macro.
//! Then create a [`SyncEditorBundle`] object and register the specified components and resources
//! with `sync_components` and `sync_resources` respectively. Some of the engine-specific types can
//! be registered automatically using the `sync_default_types` method. It is also possible to
//! specify the types individually using `sync_component` and `sync_resource`, which allows changing
//! the name of the type when viewed in the editor.
//!
//! Finally, add the [`SyncEditorBundle`] that you created to the game data.

extern crate amethyst;
extern crate crossbeam_channel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
extern crate serde_json;

use std::cmp::min;
use std::fmt::Write;
use std::collections::HashMap;
use std::time::*;
use amethyst::core::bundle::{Result as BundleResult, SystemBundle};
use amethyst::ecs::*;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::export::PhantomData;
use std::net::UdpSocket;

pub use editor_log::EditorLogger;
pub use ::serializable_entity::SerializableEntity;
pub use type_set::{ComponentSet, ResourceSet, TypeSet};

#[macro_use]
mod type_set;
mod editor_log;
mod serializable_entity;

const MAX_PACKET_SIZE: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message<T> {
    #[serde(rename = "type")]
    ty: &'static str,
    data: T,
}

#[derive(Debug, Clone, Default, Serialize)]
struct SerializedComponent<'a, T: 'a> {
    name: &'static str,
    data: HashMap<u32, &'a T>,
}

#[derive(Debug, Clone, Serialize)]
struct SerializedResource<'a, T: 'a> {
    name: &'static str,
    data: &'a T,
}

enum SerializedData {
    Resource(String),
    Component(String),
    Message(String),
}

/// A connection to an editor which allows sending messages via a [`SyncEditorSystem`].
///
/// Anything that needs to be able to send messages to the editor needs such a connection.
#[derive(Clone)]
pub struct EditorConnection {
    sender: Sender<SerializedData>,
}

impl EditorConnection {
    /// Construct a connection to the editor via sending messages to the [`SyncEditorSystem`].
    fn new(sender: Sender<SerializedData>) -> Self {
        Self { sender }
    }

    /// Send serialized data to the editor.
    fn send_data(&self, data: SerializedData) {
        self.sender.send(data);
    }

    /// Send an arbitrary message to the editor.
    ///
    /// Note that the message types supported by the editor may differ between implementations.
    pub fn send_message<T: Serialize>(&self, message_type: &'static str, data: T) {
        let serialize_data = Message {
            ty: message_type,
            data,
        };
        if let Ok(serialized) = serde_json::to_string(&serialize_data) {
            self.send_data(SerializedData::Message(serialized));
        } else {
            error!("Failed to serialize message");
        }
    }
}

/// Bundles all necessary systems for serializing all registered components and resources and
/// sending them to the editor.
pub struct SyncEditorBundle<T, U> where
    T: ComponentSet,
    U: ResourceSet,
 {
    send_interval: Duration,
    components: TypeSet<T>,
    resources: TypeSet<U>,
    sender: Sender<SerializedData>,
    receiver: Receiver<SerializedData>,
}

impl SyncEditorBundle<(), ()> {
    /// Construct an empty bundle.
    pub fn new() -> SyncEditorBundle<(), ()> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        SyncEditorBundle {
            send_interval: Duration::from_millis(200),
            components: TypeSet::new(),
            resources: TypeSet::new(),
            sender,
            receiver,
        }
    }
}

impl<T, U> SyncEditorBundle<T, U> where
    T: ComponentSet,
    U: ResourceSet,
{
    /// Synchronize amethyst types.
    ///
    /// Currently only a small set is supported. This will be expanded in the future.
    pub fn sync_default_types(
        self
    ) -> SyncEditorBundle<(T, impl ComponentSet), (U, impl ResourceSet)> {
        use amethyst::renderer::{AmbientColor, Camera, Light};
        use amethyst::core::{GlobalTransform, Transform};

        let components = type_set![Light, Camera, Transform, GlobalTransform];
        let resources = type_set![AmbientColor];
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with_set(&components),
            resources: self.resources.with_set(&resources),
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a component for synchronizing with the editor. This will result in a
    /// [`SyncComponentSystem`] being added.
    pub fn sync_component<C>(self, name: &'static str) -> SyncEditorBundle<(T, (C,)), U>
    where
        C: Component + Serialize+Send,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with::<C>(name),
            resources: self.resources,
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a set of components for synchronizing with the editor. This will result
    /// in a [`SyncComponentSystem`] being added for each component type in the set.
    pub fn sync_components<C>(self, set: &TypeSet<C>) -> SyncEditorBundle<(T, C), U>
    where
        C: ComponentSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with_set(set),
            resources: self.resources,
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a resource for synchronizing with the editor. This will result in a
    /// [`SyncResourceSystem`] being added.
    pub fn sync_resource<R>(self, name: &'static str) -> SyncEditorBundle<T, (U, (R,))>
    where
        R: Resource + Serialize,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            resources: self.resources.with::<R>(name),
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a set of resources for synchronizing with the editor. This will result
    /// in a [`SyncResourceSystem`] being added for each resource type in the set.
    pub fn sync_resources<R>(self, set: &TypeSet<R>) -> SyncEditorBundle<T, (U, R)>
    where
        R: ResourceSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            resources: self.resources.with_set(set),
            sender: self.sender,
            receiver: self.receiver,
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
    pub fn send_interval(mut self, send_interval: Duration) -> SyncEditorBundle<T, U> {
        self.send_interval = send_interval;
        self
    }

    /// Retrieve a connection to send messages to the editor via the [`SyncEditorSystem`].
    pub fn get_connection(&self) -> EditorConnection {
        EditorConnection::new(self.sender.clone())
    }
}

impl<'a, 'b, T, U> SystemBundle<'a, 'b> for SyncEditorBundle<T, U> where
    T: ComponentSet,
    U: ResourceSet,
{
    fn build(self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        let sync_system = SyncEditorSystem::from_channel(
            self.sender,
            self.receiver,
            self.send_interval,
        );
        let connection = sync_system.get_connection();

        // All systems must have finished editing data before syncing can start.
        dispatcher.add_barrier();
        self.components.create_component_sync_systems(dispatcher, &connection);
        self.resources.create_resource_sync_systems(dispatcher, &connection);

        // All systems must have finished serializing before it can be send to the editor.
        dispatcher.add_barrier();
        dispatcher.add(sync_system, "", &[]);

        Ok(())
    }
}

/// A system that is in charge of coordinating a number of serialization systems and sending
/// their results to the editor.
pub struct SyncEditorSystem {
    receiver: Receiver<SerializedData>,
    sender: Sender<SerializedData>,
    socket: UdpSocket,

    send_interval: Duration,
    next_send: Instant,

    scratch_string: String,
}

impl SyncEditorSystem {
    pub fn new(send_interval: Duration) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self::from_channel(sender, receiver, send_interval)
    }

    fn from_channel(sender: Sender<SerializedData>, receiver: Receiver<SerializedData>, send_interval: Duration) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        let scratch_string = String::with_capacity(MAX_PACKET_SIZE);
        Self {
            receiver,
            sender,
            socket,

            send_interval,
            next_send: Instant::now() + send_interval,

            scratch_string,
        }
    }

    /// Retrieve a connection to the editor via this system.
    pub fn get_connection(&self) -> EditorConnection {
        EditorConnection::new(self.sender.clone())
    }
}

impl<'a> System<'a> for SyncEditorSystem {
    type SystemData = Entities<'a>;

    fn run(&mut self, entities: Self::SystemData) {
        // Determine if we should send full state data this frame.
        let now = Instant::now();
        let send_this_frame = now >= self.next_send;

        // Calculate when we should next send full state data.
        //
        // NOTE: We do `next_send += send_interval` instead of `next_send = now + send_interval`
        // to ensure that state updates happen at a consistent cadence even if there are slight
        // timing variations in when individual frames are sent.
        //
        // NOTE: We repeatedly add `send_interval` to `next_send` to ensure that the next send
        // time is after `now`. This is to avoid running into a death spiral if a frame spike
        // causes frame time to be so long that the next send time would still be in the past.
        while self.next_send < now {
            self.next_send += self.send_interval;
        }

        let mut components = Vec::new();
        let mut resources = Vec::new();
        let mut messages = Vec::new();
        while let Some(serialized) = self.receiver.try_recv() {
            match serialized {
                SerializedData::Component(c) => components.push(c),
                SerializedData::Resource(r) => resources.push(r),
                SerializedData::Message(m) => messages.push(m),
            }
        }

        let mut entity_data = Vec::<SerializableEntity>::new();
        for (entity,) in (&*entities,).join() {
            entity_data.push(entity.into());
        }
        let entity_string = serde_json::to_string(&entity_data)
            .expect("Failed to serialize entities");

        // Create the message and serialize it to JSON. If we don't need to send the full state
        // data this frame, we discard entities, components, and resources, and only send the
        // messages (e.g. log output) from the current frame.
        if send_this_frame {
            write!(
                self.scratch_string,
                r#"{{
                    "type": "message",
                    "data": {{
                        "entities": {},
                        "components": [{}],
                        "resources": [{}],
                        "messages": [{}]
                    }}
                }}"#,
                entity_string,

                // Insert a comma between components so that it's valid JSON.
                components.join(","),
                resources.join(","),
                messages.join(","),
            );
        } else {
            write!(
                self.scratch_string,
                r#"{{
                    "type": "message",
                    "data": {{
                        "messages": [{}]
                    }}
                }}"#,

                // Insert a comma between components so that it's valid JSON.
                messages.join(","),
            );
        }

        // NOTE: We need to append a page feed character after each message since that's
        // what node-ipc expects to delimit messages.
        self.scratch_string.push_str("\u{C}");

        // Send the message, breaking it up into multiple packets if the message is too large.
        let mut bytes_sent = 0;
        while bytes_sent < self.scratch_string.len() {
            let bytes_to_send = min(self.scratch_string.len() - bytes_sent, MAX_PACKET_SIZE);
            let end_offset = bytes_sent + bytes_to_send;

            // Send the JSON message.
            let bytes = self.scratch_string[bytes_sent..end_offset].as_bytes();
            self.socket.send_to(bytes, "127.0.0.1:8000").expect("Failed to send message");

            bytes_sent += bytes_to_send;
        }

        self.scratch_string.clear();
    }
}

/// A system that serializes all components of a specific type and sends them to the
/// [`SyncEditorSystem`], which will sync them with the editor.
pub struct SyncComponentSystem<T> {
    name: &'static str,
    connection: EditorConnection,
    _phantom: PhantomData<T>,
}

impl<T> SyncComponentSystem<T> {
    pub fn new(name: &'static str, connection: EditorConnection) -> Self {
        Self {
            name,
            connection,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for SyncComponentSystem<T> where T: Component+Serialize {
    type SystemData = (Entities<'a>, ReadStorage<'a, T>);

    fn run(&mut self, (entities, components): Self::SystemData) {
        let data = (&*entities, &components)
            .join()
            .map(|(e, c)| (e.id(), c))
            .collect();
        let serialize_data = SerializedComponent { name: self.name, data };
        if let Ok(serialized) = serde_json::to_string(&serialize_data) {
            self.connection.send_data(SerializedData::Component(serialized));
        } else {
            error!("Failed to serialize component of type {}", self.name);
        }
    }
}

/// A system that serializes a resource of a specific type and sends it to the
/// [`SyncEditorSystem`], which will sync it with the editor.
pub struct SyncResourceSystem<T> {
    name: &'static str,
    connection: EditorConnection,
    _phantom: PhantomData<T>,
}

impl<T> SyncResourceSystem<T> {
    pub fn new(name: &'static str, connection: EditorConnection) -> Self {
        Self {
            name,
            connection,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for SyncResourceSystem<T> where T: Resource+Serialize {
    type SystemData = Option<Read<'a, T>>;

    fn run(&mut self, resource: Self::SystemData) {
        if let Some(resource) = resource {
            let serialize_data = SerializedResource {
                name: self.name,
                data: &*resource,
            };
            if let Ok(serialized) = serde_json::to_string(&serialize_data) {
                self.connection.send_data(SerializedData::Resource(serialized));
            } else {
                warn!("Failed to serialize resource of type {}", self.name);
            }
        } else {
            warn!("Resource named {:?} wasn't registered and will not show up in the editor", self.name);
        }
    }
}