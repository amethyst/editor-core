//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll also need to create a
//! [`SyncComponentSystem`] or [`ReadResourceSystem`] for each component and resource that you want
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
#[macro_use]
extern crate shred_derive;

use ::write_resource::WriteResourceSystem;
use std::cmp::min;
use std::fmt::Write;
use std::collections::HashMap;
use std::str;
use std::time::*;
use amethyst::core::bundle::{Result as BundleResult, SystemBundle};
use amethyst::ecs::*;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde::export::PhantomData;
use std::net::UdpSocket;

pub use editor_log::EditorLogger;
pub use ::serializable_entity::SerializableEntity;

mod editor_log;
mod serializable_entity;
mod write_resource;

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

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum IncomingMessage {
    ComponentUpdate {
        id: String,
        entity: SerializableEntity,
        data: serde_json::Value,
    },

    ResourceUpdate {
        id: String,
        data: serde_json::Value,
    },
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
            send_interval: Duration::from_millis(200),
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
            send_interval: self.send_interval,
            component_names: self.component_names,
            resource_names: self.resource_names,
            sender: self.sender,
            receiver: self.receiver,
            _phantom: PhantomData,
        }
    }

    /// Register a resource for synchronizing with the editor. This will result in a
    /// [`ReadResourceSystem`] being added.
    pub fn sync_resource<R>(mut self, name: &'static str) -> SyncEditorBundle<T, (R, U)>
    where
        R: Resource + Serialize + DeserializeOwned,
    {
        self.resource_names.push(name);
        SyncEditorBundle {
            send_interval: self.send_interval,
            component_names: self.component_names,
            resource_names: self.resource_names,
            sender: self.sender,
            receiver: self.receiver,
            _phantom: PhantomData,
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
    fn build(mut self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        // In order to match the order of the type list.
        self.component_names.reverse();
        self.resource_names.reverse();

        let mut sync_system = SyncEditorSystem::from_channel(
            self.sender,
            self.receiver,
            self.send_interval,
        );
        let connection = sync_system.get_connection();

        // All systems must have finished editing data before syncing can start.
        dispatcher.add_barrier();
        T::create_sync_systems(dispatcher, &connection, &self.component_names);
        U::create_sync_systems(
            dispatcher,
            &connection,
            &self.resource_names,
            &mut sync_system.deserializer_map,
        );

        // All systems must have finished serializing before it can be send to the editor.
        dispatcher.add_barrier();
        dispatcher.add(sync_system, "sync_editor_system", &[]);

        Ok(())
    }
}

/// A system that is in charge of coordinating a number of serialization systems and sending
/// their results to the editor.
pub struct SyncEditorSystem {
    receiver: Receiver<SerializedData>,
    sender: Sender<SerializedData>,
    socket: UdpSocket,

    // Map containing channels used to send incoming serialized component/resource data from the
    // editor. Incoming data is sent to specialized systems that deserialize the data and update
    // the corresponding local data.
    deserializer_map: HashMap<&'static str, Sender<serde_json::Value>>,

    send_interval: Duration,
    next_send: Instant,

    scratch_string: String,

    incoming_buffer: Vec<u8>,
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

            deserializer_map: HashMap::new(),

            send_interval,
            next_send: Instant::now() + send_interval,

            scratch_string,
            incoming_buffer: Vec::with_capacity(1024),
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

        // Read any incoming messages from the editor process.
        let mut buf = [0; 1024];
        loop {
            // TODO: Verify that the incoming address matches the editor process address.
            let (bytes_read, addr) = match self.socket.recv_from(&mut buf[..]) {
                Ok(res) => res,
                Err(error) => {
                    trace!("Error reading incoming: {:?}", error);
                    continue;
                }
            };

            // Stop reading from the socket once there's no more incoming data.
            if bytes_read == 0 { break; }

            // Add the bytes from the incoming packet to the buffer.
            self.incoming_buffer.extend_from_slice(&buf[..bytes_read]);
        }

        // Check the incoming buffer to see if any completed messages have been received.
        while let Some(index) = self.incoming_buffer.iter().position(|&byte| byte == 0xC) {
            let message_bytes = &self.incoming_buffer[..index];
            let result: Option<IncomingMessage> = str::from_utf8(message_bytes)
                .ok()
                .and_then(|message| serde_json::from_str::<IncomingMessage>(message).ok());
            match result {
                Some(message) => match message {
                    IncomingMessage::UpdateComponent { .. } => unimplemented!("Updating components not yet supported"),
                    IncomingMessage::UpdateResource { id, data } => {
                        // TODO: Should we do something if there was no deserialer system for the
                        // specified ID?
                        if let Some(sender) = self.deserializer_map.get(id) {
                            // TODO: Should we do something to prevent this from blocking?
                            sender.send(data);
                        }
                    }
                }

                // If the message string is invalid UTF-8 we simply ignore it.
                None => {}
            }

            // Remove the message bytes from the beginning of the incominb buffer.
            self.incoming_buffer.drain(..=index);
        }
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
struct ReadResourceSystem<T> {
    name: &'static str,
    connection: EditorConnection,
    _phantom: PhantomData<T>,
}

impl<T> ReadResourceSystem<T> {
    pub fn new(name: &'static str, connection: EditorConnection) -> Self {
        Self {
            name,
            connection,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for ReadResourceSystem<T> where T: Resource+Serialize {
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
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
        deserializer_map: &mut HashMap<&'static str, Sender<serde_json::Value>>,
    );
}

impl ResourceSet for () {
    fn create_sync_systems(
        _: &mut DispatcherBuilder,
        _: &EditorConnection,
        _: &[&'static str],
        deserializer_map: &mut HashMap<&'static str, Sender<serde_json::Value>>
    ) { }
}

impl<A, B> ResourceSet for (A, B)
where
    A: Resource + Serialize + DeserializeOwned,
    B: ResourceSet,
{
    fn create_sync_systems(
        dispatcher: &mut DispatcherBuilder,
        connection: &EditorConnection,
        names: &[&'static str],
        deserializer_map: &mut HashMap<&'static str, Sender<serde_json::Value>>
    ) {
        B::create_sync_systems(dispatcher, connection, &names[1..], deserializer_map);

        // Create a system for serialing the resource data and sending it to the `SyncEditorSystem`.
        dispatcher.add(ReadResourceSystem::<A>::new(names[0], connection.clone()), "", &[]);

        // Create a deserializer system for the resource.
        let (sender, receiver) = crossbeam_channel::unbounded();
        deserializer_map.insert(names[0], sender);
        dispatcher.add(WriteResourceSystem::<A>::new(receiver), "", &["sync_editor_system"]);
    }
}
