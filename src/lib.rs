//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll also need to register each component
//! and resource that you want to visualize.
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
//! let editor_system = SyncEditorSystem::new()
//!     // Register any engine-specific components you want to visualize.
//!     .sync_component::<Transform>("Transform")
//!     // Register any custom components that you use in your game.
//!     .sync_component::<Foo>("Foo")
//!
//! let game_data = GameDataBuilder::default()
//!     // Register the `SyncEditorSystem` as thread local to ensure it runs last.
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
//! First, create a [`SyncEditorSystem`] object. You must then register each of the component and
//! resource types that you want to see in the editor:
//!
//! * For each component `T`, register the component with `sync_component::<T>(name)`, specifying
//!   the name of the component and its concrete type.
//! * For each resource, register the component with `sync_resource::<T>(name)`, specifying the
//!   name of the resource and its concrete type.
//!
//! Finally, register the [`SyncEditorSystem`] that you first created as a thread-local system.

extern crate amethyst;
extern crate crossbeam_channel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
extern crate serde_json;

use std::collections::HashMap;
use amethyst::ecs::*;
use amethyst::ecs::world::EntitiesRes;
use amethyst::shred::Resource;
use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use serde::export::PhantomData;
use std::net::UdpSocket;

pub use ::serializable_entity::SerializableEntity;

mod serializable_entity;

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

/// Syncs all components and resources specified by the types.
///
/// T is a list of all synced component types.
/// U is a list of all synced resource types.
pub struct SyncEditorSystem<T, U> {
    component_names: Vec<&'static str>,
    resource_names: Vec<&'static str>,
    socket: UdpSocket,
    _phantom: PhantomData<(T, U)>,
}

impl SyncEditorSystem<(), ()> {
    pub fn new() -> SyncEditorSystem<(), ()> {
        // Setup the socket for communicating with the editor and add it as a resource.
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket.connect("127.0.0.1:8000").expect("Failed to connect to editor");

        SyncEditorSystem {
            component_names: Vec::new(),
            resource_names: Vec::new(),
            socket,
            _phantom: PhantomData,
        }
    }
}

impl<T, U> SyncEditorSystem<T, U> {
    pub fn sync_component<C>(mut self, name: &'static str) -> SyncEditorSystem<(C, T), U>
    where
        C: Component + Serialize,
    {
        self.component_names.push(name);
        SyncEditorSystem {
            component_names: self.component_names,
            resource_names: self.resource_names,
            socket: self.socket,
            _phantom: PhantomData,
        }
    }

    pub fn sync_resource<R>(mut self, name: &'static str) -> SyncEditorSystem<T, (R, U)>
    where
        R: Resource + Serialize,
    {
        self.resource_names.push(name);
        SyncEditorSystem {
            component_names: self.component_names,
            resource_names: self.resource_names,
            socket: self.socket,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T, U> System<'a> for SyncEditorSystem<T, U>
where
    T: ComponentSet<'a>,
    U: ResourceSet<'a>,
{
    type SystemData = (Entities<'a>, T::SystemData, U::SystemData);

    fn run(&mut self, (entities, components, resources): Self::SystemData) {
        let components_string =
            T::serialize_json(&*entities, components, &self.component_names).join(",");
        let resources_string =
            U::serialize_json(resources, &self.resource_names).join(",");

        let mut entity_data = Vec::<SerializableEntity>::new();
        for (entity,) in (&*entities,).join() {
            entity_data.push(entity.into());
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
        self.socket.send(message_string.as_bytes()).expect("Failed to send message");
    }

    fn setup(&mut self, res: &mut Resources) {
        Self::SystemData::setup(res);
        // In order to match the order of the type list.
        self.component_names.reverse();
        self.resource_names.reverse();
    }
}

pub trait ComponentSet<'a> {
    type SystemData: SystemData<'a>;

    /// Serialize each component in a json map. Does not need to return the components in order.
    fn serialize_json(
        entities: &EntitiesRes,
        data: Self::SystemData,
        names: &[&'static str],
    ) -> Vec<String>;
}

impl<'a> ComponentSet<'a> for () {
    type SystemData = ();

    fn serialize_json(_: &EntitiesRes, _: Self::SystemData, _: &[&'static str]) -> Vec<String> {
        Vec::new()
    }
}

impl<'a, A, B> ComponentSet<'a> for (A, B)
where
    A: Component + Serialize,
    B: ComponentSet<'a>,
{
    type SystemData = (ReadStorage<'a, A>, B::SystemData);

    fn serialize_json(
        entities: &EntitiesRes,
        (component, component_rest): Self::SystemData,
        names: &[&'static str],
    ) -> Vec<String> {
        let mut res = B::serialize_json(entities, component_rest, &names[1..]);
        let component_data = (entities, &component)
            .join()
            .map(|(e, c)| (e.id(), c))
            .collect();
        let json = serde_json::to_string(&SerializedComponent {
            name: names[0],
            data: component_data,
        }).expect("Failed to serialize message");
        res.push(json);
        res
    }
}

pub trait ResourceSet<'a> {
    type SystemData: SystemData<'a>;

    /// Serialize each resource. Does not need to return the resources in order.
    fn serialize_json(data: Self::SystemData, names: &[&'static str]) -> Vec<String>;
}

impl<'a> ResourceSet<'a> for () {
    type SystemData = ();

    fn serialize_json(_: Self::SystemData, _: &[&'static str]) -> Vec<String> {
        Vec::new()
    }
}

impl<'a, A, B> ResourceSet<'a> for (A, B)
where
    A: Resource + Serialize,
    B: ResourceSet<'a>,
{
    type SystemData = (ReadExpect<'a, A>, B::SystemData);

    fn serialize_json(
        (resource, resource_rest): Self::SystemData,
        names: &[&'static str],
    ) -> Vec<String> {
        let mut res = B::serialize_json(resource_rest, &names[1..]);
        let json = serde_json::to_string(&SerializedResource {
            name: names[0],
            data: &*resource,
        }).expect("Failed to serialize resource");
        res.push(json);
        res
    }
}
