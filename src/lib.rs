//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll need to register
//! [`SyncEditorSystem`], along with a `SyncComponentSystem<T>` for each component that you
//! want to visualize, and a `SyncResourceSystem<T>` for each resource you want to visualize.
//!
//! # Example
//!
//! ```
//! extern crate amethyst;
//! extern crate amethyst_editor_sync;
//!
//! use amethyst::*;
//! use amethyst_editor_sync::*;
//!
//! // Create a root `SyncEditorSystem` to coordinate sending all data to the editor.
//! let editor_system = SyncEditorSystem::new();
//!
//! let game_data = GameDataBuilder::default()
//!     // Register the systems for your game first.
//!
//!     // Insert a barrier to ensure that the editor syncing runs after all
//!     // other systems have finished.
//!     .with_barrier()
//!
//!     // Register any engine-specific components you want to visualize.
//!     .with(
//!         SyncComponentSystem::<Transform>::new("Transform", &editor_system),
//!         "editor_transform",
//!         &[],
//!     )
//!
//!     // Register any custom components that you use in your game.
//!     .with(
//!         SyncComponentSystem::<Foo>::new("Foo", &editor_system),
//!         "editor_foo",
//!         &[],
//!     )
//!
//!     // Register the `SyncEditorSystem` as thread local to ensure it runs last,
//!     // after the other systems have had a chance to serialize the state data
//!     // for all components/resources.
//!     .with_thread_local(editor_system);
//!
//! // Make sure you enable serialization for your custom components and resources!
//! #[derive(Serialize, Deserialize)]
//! struct Foo {
//!     bar: usize,
//!     baz: String,
//! }
//! ```
//!
//! # Usage
//!
//! First, create a [`SyncEditorSystem`] object. Unlike most systems, you must create this ahead
//! of time because you will need to use it when setting up syncing for each component/resource
//! type.
//!
//! Create your [`GameDataBuilder`] and register your game's systems. Then, insert a barrier
//! using [`with_barrier`]. You must then register a system for each of the component and
//! resource types that you want to see in the editor:
//!
//! * For each component, register a [`SyncComponentSystem<T>`], specifying the name of the
//!   component and its concrete type. The system should have no dependencies
//! * For each resource, register a [`SyncResourceSystem<T>`], specifying the name of the
//!   resource and its concrete type. The system should have no dependencies.
//!
//! Finally, register the [`SyncEditorSystem`] that you first created as a thread-local system.

use std::collections::HashMap;
use amethyst::ecs::*;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::export::PhantomData;
use std::net::UdpSocket;

extern crate amethyst;
extern crate crossbeam_channel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
extern crate serde_json;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct EntityData {
    id: u32,
    generation: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message<T> {
    #[serde(rename = "type")]
    ty: &'static str,
    data: T,
}

enum SerializedData {
    Resource(String),
    Component(String),
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

#[derive(Debug, Clone)]
pub struct SyncComponentSystem<T> {
    name: &'static str,
    sender: Sender<SerializedData>,
    _marker: PhantomData<T>,
}

impl<T> SyncComponentSystem<T> {
    pub fn new(name: &'static str, send_to: &SyncEditorSystem) -> Self {
        SyncComponentSystem {
            name,
            sender: send_to.sender.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for SyncComponentSystem<T> where T: Component + Serialize {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, T>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (entities, transforms) = data;

        let mut entity_data = Vec::new();
        for (entity,) in (&*entities,).join() {
            entity_data.push(EntityData {
                id: entity.id(),
                generation: entity.gen().id(),
            });
        }

        let mut component_data = HashMap::new();
        for (entity, transform) in (&*entities, &transforms).join() {
            component_data.insert(entity.id(), transform);
        }
        let serialized = serde_json::to_string(&SerializedComponent { name: self.name, data: component_data }).expect("Failed to serialize message");

        self.sender.send(SerializedData::Component(serialized));
    }
}

#[derive(Debug, Clone)]
pub struct SyncResourceSystem<T> {
    name: &'static str,
    sender: Sender<SerializedData>,
    _marker: PhantomData<T>,
}

impl<T> SyncResourceSystem<T> {
    pub fn new(name: &'static str, send_to: &SyncEditorSystem) -> Self {
        SyncResourceSystem {
            name,
            sender: send_to.sender.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for SyncResourceSystem<T> where T: Resource + Serialize {
    type SystemData = ReadExpect<'a, T>;

    fn run(&mut self, data: Self::SystemData) {
        let serialized = serde_json::to_string(&SerializedResource { name: self.name, data: &*data }).expect("Failed to serialize resource");
        self.sender.send(SerializedData::Resource(serialized));
    }
}

#[derive(Debug, Clone)]
pub struct SyncEditorSystem {
    sender: Sender<SerializedData>,
    receiver: Receiver<SerializedData>,
}

impl SyncEditorSystem {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        SyncEditorSystem { sender, receiver }
    }
}


impl<'a> System<'a> for SyncEditorSystem {
    type SystemData = (
        ReadExpect<'a, UdpSocket>,
        Entities<'a>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (socket, entities) = data;

        let mut components_string = String::new();
        let mut resources_string = String::new();
        while let Some(serialized) = self.receiver.try_recv() {
            match serialized {
                SerializedData::Component(component) => {
                    // Insert a comma between each component so that it's valid JSON.
                    if components_string.len() > 0 {
                        components_string.push(',');
                    }

                    // Add the component to the JSON chunk for components.
                    components_string.push_str(&component);
                }

                SerializedData::Resource(resource) => {
                    // Insert a comma between each resource so that it's valid JSON.
                    if resources_string.len() > 0 {
                        resources_string.push(',');
                    }

                    // Add the resource to the JSON chunk for resources.
                    resources_string.push_str(&resource);
                }
            }
        }

        let mut entity_data = Vec::new();
        for (entity,) in (&*entities,).join() {
            entity_data.push(EntityData {
                id: entity.id(),
                generation: entity.gen().id(),
            });
        }
        let entity_string = serde_json::to_string(&entity_data).expect("Failed to serialize entities");

        // Create the message and serialize it to JSON.
        let mut message_string = format!(
            r#"{{
                "type": "message",
                "data": {{
                    "entities": {},
                    "components": [{}],
                    "resources": [{}]
                }}
            }}"#,
            entity_string,
            components_string,
            resources_string,
        );

        trace!("{}", message_string);

        // NOTE: We need to append a page feed character after each message since that's what node-ipc
        // expects to delimit messages.
        message_string.push_str("\u{C}");

        // Send the JSON message.
        socket.send(message_string.as_bytes()).expect("Failed to send message");
    }
}
