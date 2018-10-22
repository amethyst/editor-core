use ::{EditorConnection, SerializedComponent, SerializedData};
use serde_json;
use std::str;
use amethyst::ecs::*;
use serde::Serialize;
use serde::export::PhantomData;

/// A system that serializes all components of a specific type and sends them to the
/// [`SyncEditorSystem`], which will sync them with the editor.
pub struct ReadComponentSystem<T> {
    name: &'static str,
    connection: EditorConnection,
    _phantom: PhantomData<T>,
}

impl<T> ReadComponentSystem<T> {
    pub fn new(name: &'static str, connection: EditorConnection) -> Self {
        Self {
            name,
            connection,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for ReadComponentSystem<T> where T: Component+Serialize {
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
