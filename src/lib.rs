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
//! // Create a `SyncEditorBundle` which will create all necessary systems to send the components
//! // to the editor.
//! let editor_sync_bundle = SyncEditorBundle::new()
//!     // Register any engine-specific components you want to visualize.
//!     .sync_component::<Transform>("Transform")
//!     // Register any custom components that you use in your game.
//!     .sync_component::<Foo>("Foo");
//!
//! let game_data = GameDataBuilder::default()
//!     .with_bundle(editor_sync_bundle)?;
//! # Ok(())
//! # }
//!
//! // Make sure you enable serialization for your custom components and resources!
//! #[derive(Serialize, Deserialize)]
//! struct Foo {
//!     bar: usize,
//!     baz: String,
//! }
//!
//! impl Component for Foo {
//!     type Storage = DenseVecStorage<Self>;
//! }
//! ```
//!
//! # Usage
//!
//! First, create a [`SyncEditorBundle`] object. You must then register each of the component and
//! resource types that you want to see in the editor:
//!
//! * For each component `T`, register the component with `sync_component::<T>(name)`, specifying
//!   the name of the component and its concrete type.
//! * For each resource, register the component with `sync_resource::<T>(name)`, specifying the
//!   name of the resource and its concrete type.
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
use amethyst::core::bundle::{Result as BundleResult, SystemBundle};
use amethyst::ecs::*;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::export::PhantomData;
use std::net::UdpSocket;

pub use editor_log::EditorLogger;
pub use ::serializable_entity::SerializableEntity;

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
    component_names: Vec<&'static str>,
    resource_names: Vec<&'static str>,
    sender: Sender<SerializedData>,
    receiver: Receiver<SerializedData>,
    _phantom: PhantomData<(T, U)>,
}

impl SyncEditorBundle<(), ()> {
    /// Construct an empty bundle.
    pub fn new() -> SyncEditorBundle<(), ()> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        SyncEditorBundle {
            component_names: Vec::new(),
            resource_names: Vec::new(),
            sender,
            receiver,
            _phantom: PhantomData,
        }
    }
}

impl Default for SyncEditorBundle<(), ()> {
    fn default() -> Self { SyncEditorBundle::new() }
}

impl<T, U> SyncEditorBundle<T, U> where
    T: ComponentSet,
    U: ResourceSet,
{
    /// Register a component for synchronizing with the editor. This will result in a
    /// [`SyncComponentSystem`] being added.
    pub fn sync_component<C>(mut self, name: &'static str) -> SyncEditorBundle<(C, T), U>
    where
        C: Component + Serialize+Send,
    {
        self.component_names.push(name);
        SyncEditorBundle {
            component_names: self.component_names,
            resource_names: self.resource_names,
            sender: self.sender,
            receiver: self.receiver,
            _phantom: PhantomData,
        }
    }

    /// Register a resource for synchronizing with the editor. This will result in a
    /// [`SyncResourceSystem`] being added.
    pub fn sync_resource<R>(mut self, name: &'static str) -> SyncEditorBundle<T, (R, U)>
    where
        R: Resource + Serialize,
    {
        self.resource_names.push(name);
        SyncEditorBundle {
            component_names: self.component_names,
            resource_names: self.resource_names,
            sender: self.sender,
            receiver: self.receiver,
            _phantom: PhantomData,
        }
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
    fn build(mut self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        // In order to match the order of the type list.
        self.component_names.reverse();
        self.resource_names.reverse();

        let sync_system = SyncEditorSystem::from_channel(self.sender, self.receiver);
        let connection = sync_system.get_connection();

        // All systems must have finished editing data before syncing can start.
        dispatcher.add_barrier();
        T::create_sync_systems(dispatcher, &connection, &self.component_names);
        U::create_sync_systems(dispatcher, &connection, &self.resource_names);

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

    scratch_string: String,
}

impl SyncEditorSystem {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self::from_channel(sender, receiver)
    }

    fn from_channel(sender: Sender<SerializedData>, receiver: Receiver<SerializedData>) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        let scratch_string = String::with_capacity(MAX_PACKET_SIZE);
        Self { receiver, sender, socket, scratch_string }
    }

    /// Retrieve a connection to the editor via this system.
    pub fn get_connection(&self) -> EditorConnection {
        EditorConnection::new(self.sender.clone())
    }
}

impl Default for SyncEditorSystem {
    fn default() -> Self { SyncEditorSystem::new() }
}

impl<'a> System<'a> for SyncEditorSystem {
    type SystemData = Entities<'a>;

    fn run(&mut self, entities: Self::SystemData) {
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

        // Create the message and serialize it to JSON.
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


pub trait ComponentSet {
    fn create_sync_systems(dispatcher: &mut DispatcherBuilder, connection: &EditorConnection, names: &[&'static str]);
}

impl ComponentSet for () {
    fn create_sync_systems(_: &mut DispatcherBuilder, _: &EditorConnection, _: &[&'static str]) { }
}

impl<A, B> ComponentSet for (A, B)
where
    A: Component + Serialize + Send,
    B: ComponentSet,
{
    fn create_sync_systems(dispatcher: &mut DispatcherBuilder, connection: &EditorConnection, names: &[&'static str]) {
        B::create_sync_systems(dispatcher, connection, &names[1..]);
        dispatcher.add(SyncComponentSystem::<A>::new(names[0], connection.clone()), "", &[]);
    }
}

pub trait ResourceSet {
    fn create_sync_systems(dispatcher: &mut DispatcherBuilder, connection: &EditorConnection, names: &[&'static str]);
}

impl ResourceSet for () {
    fn create_sync_systems(_: &mut DispatcherBuilder, _: &EditorConnection, _: &[&'static str]) { }
}

impl<A, B> ResourceSet for (A, B)
where
    A: Resource + Serialize,
    B: ResourceSet,
{
    fn create_sync_systems(dispatcher: &mut DispatcherBuilder, connection: &EditorConnection, names: &[&'static str]) {
        B::create_sync_systems(dispatcher, connection, &names[1..]);
        dispatcher.add(SyncResourceSystem::<A>::new(names[0], connection.clone()), "", &[]);
    }
}
