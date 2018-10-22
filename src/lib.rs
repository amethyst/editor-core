//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll also need to create a
//! [`ReadComponentSystem`] or [`ReadResourceSystem`] for each component and resource that you want
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
//!
//! // Do the same for your resources.
//! let resources = type_set![MyResource];
//!
//! // Read-only resources (i.e. that don't implement `DeserializeOwned`) can still
//! // be displayed in the editor, but they can't be edited.
//! let read_only_resources = type_set![ReadOnlyResource];
//!
//! // Create a SyncEditorBundle which will create all necessary systems to send the components
//! // to the editor.
//! let editor_sync_bundle = SyncEditorBundle::new()
//!     // Register the default types from the engine.
//!     .sync_default_types()
//!     // Register the components and resources specified above.
//!     .sync_components(&components)
//!     .sync_resources(&resources)
//!     .read_resources(&read_only_resources);
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
//!
//! #[derive(Serialize)]
//! struct ReadOnlyResource {
//!     important_entity: SerializableEntity,
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
extern crate log_once;
#[macro_use]
extern crate serde;
extern crate serde_json;

use std::cmp::min;
use std::fmt::Write;
use std::collections::HashMap;
use std::io;
use std::str;
use std::time::*;
use amethyst::core::bundle::{Result as BundleResult, SystemBundle};
use amethyst::ecs::*;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::net::UdpSocket;

pub use editor_log::EditorLogger;
pub use ::serializable_entity::SerializableEntity;
pub use type_set::{ReadComponentSet, ReadResourceSet, TypeSet, WriteResourceSet};

#[macro_use]
mod type_set;
mod editor_log;
mod serializable_entity;
mod read_component;
mod read_resource;
mod write_resource;

const MAX_PACKET_SIZE: usize = 32 * 1024;

type DeserializerMap = HashMap<&'static str, Sender<serde_json::Value>>;

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

/// Messages sent from the editor to the game.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum IncomingMessage {
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
pub struct SyncEditorBundle<T, U, V> where
    T: ReadComponentSet,
    U: ReadResourceSet,
    V: ReadResourceSet + WriteResourceSet,
 {
    send_interval: Duration,
    components: TypeSet<T>,
    read_resources: TypeSet<U>,
    write_resources: TypeSet<V>,
    sender: Sender<SerializedData>,
    receiver: Receiver<SerializedData>,
}

impl SyncEditorBundle<(), (), ()> {
    /// Construct an empty bundle.
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        SyncEditorBundle {
            send_interval: Duration::from_millis(200),
            components: TypeSet::new(),
            read_resources: TypeSet::new(),
            write_resources: TypeSet::new(),
            sender,
            receiver,
        }
    }
}

impl Default for SyncEditorBundle<(), (), ()> {
    fn default() -> Self { Self::new() }
}

impl<T, U, V> SyncEditorBundle<T, U, V> where
    T: ReadComponentSet,
    U: ReadResourceSet,
    V: ReadResourceSet + WriteResourceSet,
{
    /// Synchronize amethyst types.
    ///
    /// Currently only a small set is supported. This will be expanded in the future.
    pub fn sync_default_types(
        self
    ) -> SyncEditorBundle<
        impl ReadComponentSet,
        impl ReadResourceSet,
        impl ReadResourceSet + WriteResourceSet,
    > {
        use amethyst::renderer::{AmbientColor, Camera, Light};
        use amethyst::core::{GlobalTransform, Transform};

        let components = type_set![Light, Camera, Transform, GlobalTransform];
        let read_resources = type_set![];
        let write_resources = type_set![AmbientColor];
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with_set(&components),
            read_resources: self.read_resources.with_set(&read_resources),
            write_resources: self.write_resources.with_set(&write_resources),
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a component for synchronizing with the editor. This will result in a
    /// [`ReadComponentSystem`] being added.
    pub fn sync_component<C>(self, name: &'static str) -> SyncEditorBundle<impl ReadComponentSet, U, V>
    where
        C: Component + Serialize+Send,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with::<C>(name),
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Register a set of components for synchronizing with the editor. This will result
    /// in a [`ReadComponentSystem`] being added for each component type in the set.
    pub fn sync_components<C>(self, set: &TypeSet<C>) -> SyncEditorBundle<impl ReadComponentSet, U, V>
    where
        C: ReadComponentSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components.with_set(set),
            read_resources: self.read_resources,
            write_resources: self.write_resources,
            sender: self.sender,
            receiver: self.receiver,
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
    pub fn sync_resource<R>(self, name: &'static str) -> SyncEditorBundle<T, U, impl ReadResourceSet + WriteResourceSet>
    where
        R: Resource + Serialize + DeserializeOwned,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            read_resources: self.read_resources,
            write_resources: self.write_resources.with::<R>(name),
            sender: self.sender,
            receiver: self.receiver,
        }
    }

    /// Registers a set of resource types to be synchronized with the editor.
    ///
    /// At runtime, the state data for the resources in `R` will be sent to the editor for
    /// viewing and debugging. The editor will also be able to send back changes to the
    /// resource's data, which will automatically be applied to the local world state.
    pub fn sync_resources<R>(self, set: &TypeSet<R>) -> SyncEditorBundle<T, U, impl ReadResourceSet + WriteResourceSet>
    where
        R: ReadResourceSet + WriteResourceSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            read_resources: self.read_resources,
            write_resources: self.write_resources.with_set(set),
            sender: self.sender,
            receiver: self.receiver,
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
    pub fn read_resource<R>(self, name: &'static str) -> SyncEditorBundle<T, impl ReadResourceSet, V>
    where
        R: Resource + Serialize,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            read_resources: self.read_resources.with::<R>(name),
            write_resources: self.write_resources,
            sender: self.sender,
            receiver: self.receiver,
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
    pub fn read_resources<R>(self, set: &TypeSet<R>) -> SyncEditorBundle<T, impl ReadResourceSet, V>
    where
        R: ReadResourceSet,
    {
        SyncEditorBundle {
            send_interval: self.send_interval,
            components: self.components,
            read_resources: self.read_resources.with_set(set),
            write_resources: self.write_resources,
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
    pub fn send_interval(mut self, send_interval: Duration) -> SyncEditorBundle<T, U, V> {
        self.send_interval = send_interval;
        self
    }

    /// Retrieve a connection to send messages to the editor via the [`SyncEditorSystem`].
    pub fn get_connection(&self) -> EditorConnection {
        EditorConnection::new(self.sender.clone())
    }
}

impl<'a, 'b, T, U, V> SystemBundle<'a, 'b> for SyncEditorBundle<T, U, V> where
    T: ReadComponentSet,
    U: ReadResourceSet,
    V: ReadResourceSet + WriteResourceSet,
{
    fn build(self, dispatcher: &mut DispatcherBuilder<'a, 'b>) -> BundleResult<()> {
        let mut sync_system = SyncEditorSystem::from_channel(
            self.sender,
            self.receiver,
            self.send_interval,
        );
        let connection = sync_system.get_connection();

        // All systems must have finished editing data before syncing can start.
        dispatcher.add_barrier();
        self.components.create_component_sync_systems(dispatcher, &connection);
        self.read_resources.create_resource_read_systems(dispatcher, &connection);
        self.write_resources.create_resource_read_systems(dispatcher, &connection);

        // All systems must have finished serializing before it can be send to the editor.
        dispatcher.add_barrier();
        self.write_resources.create_resource_write_systems(
            dispatcher,
            &mut sync_system.deserializer_map,
        );
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
        // Create the socket used for communicating with the editor.
        //
        // NOTE: We set the socket to nonblocking so that we don't block if there are no incoming
        // messages to read. We `expect` on the call to `set_nonblocking` because the game will
        // hang if the socket is still set to block when the game runs.
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket.set_nonblocking(true).expect("Failed to make editor socket nonblocking");

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
        let editor_address = ([127, 0, 0, 1], 8000).into();
        let mut bytes_sent = 0;
        while bytes_sent < self.scratch_string.len() {
            let bytes_to_send = min(self.scratch_string.len() - bytes_sent, MAX_PACKET_SIZE);
            let end_offset = bytes_sent + bytes_to_send;

            // Send the JSON message.
            let bytes = self.scratch_string[bytes_sent..end_offset].as_bytes();
            self.socket.send_to(bytes, editor_address).expect("Failed to send message");

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
                    match error.kind() {
                        // If the read would block, it means that there was no incoming data and we
                        // should break from the loop.
                        io::ErrorKind::WouldBlock => break,

                        // This is an "error" that happens on Windows if no editor is running to
                        // receive the state update we just sent. The OS gives a "connection was
                        // forcibly closed" error when no socket receives the message, but we
                        // don't care if that happens (in fact, we use UDP specifically so that
                        // we can broadcast messages without worrying about establishing a
                        // connection).
                        io::ErrorKind::ConnectionReset => continue,

                        // All other error kinds should be indicative of a genuine error. For our
                        // purposes we still want to ignore them, but we'll at least log a warning
                        // in case it helps debug an issue.
                        _ => {
                            warn!("Error reading incoming: {:?}", error);
                            continue;
                        }
                    }
                }
            };

            if addr != editor_address {
                trace!("Packet received from unknown address {:?}", addr);
                continue;
            }

            // Add the bytes from the incoming packet to the buffer.
            self.incoming_buffer.extend_from_slice(&buf[..bytes_read]);
        }

        // Check the incoming buffer to see if any completed messages have been received.
        while let Some(index) = self.incoming_buffer.iter().position(|&byte| byte == 0xC) {
            // HACK: Manually introduce a scope here so that the compiler can tell when we're done
            // using borrowing the message bytes from `self.incoming_buffer`. This can be removed
            // once NLL is stable.
            {
                let message_bytes = &self.incoming_buffer[..index];
                let result = str::from_utf8(message_bytes)
                    .ok()
                    .and_then(|message| serde_json::from_str(message).ok());
                if let Some(message) = result {
                    match message {
                        IncomingMessage::ResourceUpdate { id, data } => {
                            // TODO: Should we do something if there was no deserialer system for the
                            // specified ID?
                            if let Some(sender) = self.deserializer_map.get(&*id) {
                                // TODO: Should we do something to prevent this from blocking?
                                sender.send(data);
                            }
                        }
                    }
                }
            }

            // Remove the message bytes from the beginning of the incoming buffer.
            self.incoming_buffer.drain(..=index);
        }
    }
}
