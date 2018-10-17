use amethyst::shred::Resource;
use amethyst::ecs::*;
use crossbeam_channel::Receiver;
use serde::de::DeserializeOwned;
use serde_json;
use std::marker::PhantomData;

/// A system that deserializes incoming updates for a resource and applies them to the local
/// instance of that resource.
pub(crate) struct WriteResourceSystem<T> {
    id: &'static str,
    incoming: Receiver<serde_json::Value>,
    _phantom: PhantomData<T>,
}

impl<T> WriteResourceSystem<T> {
    pub(crate) fn new(id: &'static str, incoming: Receiver<serde_json::Value>) -> Self {
        WriteResourceSystem {
            id,
            incoming,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for WriteResourceSystem<T> where T: Resource + DeserializeOwned {
    type SystemData = Option<Write<'a, T>>;

    fn run(&mut self, data: Self::SystemData) {
        trace!("`WriteResourceSystem::run` for {}", self.id);

        let mut resource = match data {
            Some(res) => res,
            None => return,
        };

        while let Some(incoming) = self.incoming.try_recv() {
            debug!("Got incoming message for {}: {:?}", self.id, incoming);

            let updated = match serde_json::from_value(incoming) {
                Ok(updated) => updated,
                Err(error) => {
                    debug!("Failed to deserialize update for {}: {:?}", self.id, error);
                    continue;
                }
            };

            *resource = updated;
        }
    }
}
