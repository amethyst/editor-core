use amethyst::ecs::*;
use amethyst::shred::Resource;
use serde::Serialize;
use serde_json;
use std::marker::PhantomData;
use types::{EditorConnection, SerializedData, SerializedResource};

/// A system that serializes a resource of a specific type and sends it to the
/// [`SyncEditorSystem`].
///
/// An instance of this system will be created for each resource type the user
/// registers with the [`SyncEditorBundle`] when initializing their game.
///
/// [`SyncEditorSystem`]: ./struct.SyncEditorSystem.html
/// [`SyncEditorBundle`]: ./struct.SyncEditorBundle.html
pub(crate) struct ReadResourceSystem<T> {
    name: &'static str,
    connection: EditorConnection,
    _phantom: PhantomData<T>,
}

impl<T> ReadResourceSystem<T> {
    pub(crate) fn new(name: &'static str, connection: EditorConnection) -> Self {
        Self {
            name,
            connection,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for ReadResourceSystem<T>
where
    T: Resource + Serialize,
{
    type SystemData = Option<Read<'a, T>>;

    fn run(&mut self, resource: Self::SystemData) {
        let resource = match resource {
            Some(resource) => resource,
            None => {
                warn_once!(
                    "Resource named {:?} wasn't registered and will not show up in the editor",
                    self.name
                );
                return;
            }
        };

        //println!("`ReadResourceSystem::run` for {}", self.name);

        let serialize_data = SerializedResource {
            name: self.name,
            data: &*resource,
        };

        if let Ok(serialized) = serde_json::to_string(&serialize_data) {
            self.connection
                .send_data(SerializedData::Resource(serialized));
        } else {
            warn!("Failed to serialize resource of type {}", self.name);
        }
    }
}
