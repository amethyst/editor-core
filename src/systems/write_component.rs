use ::IncomingComponent;
use amethyst::ecs::prelude::*;
use crossbeam_channel::Receiver;
use serde::de::DeserializeOwned;
use serde_json;
use std::marker::PhantomData;

pub(crate) struct WriteComponentSystem<T> {
    id: &'static str,
    incoming: Receiver<IncomingComponent>,
    _phantom: PhantomData<T>,
}

impl<T> WriteComponentSystem<T> {
    pub(crate) fn new(id: &'static str, incoming: Receiver<IncomingComponent>) -> Self {
        WriteComponentSystem {
            id,
            incoming,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for WriteComponentSystem<T> where T: Component + DeserializeOwned + Send + Sync {
    type SystemData = WriteStorage<'a, T>;

    fn run(&mut self, mut storage: Self::SystemData) {
        trace!("`WriteComponentSystem::run` for {}", self.id);

        while let Some(incoming) = self.incoming.try_recv() {
            debug!("Got incoming message for {}: {:?}", self.id, incoming);

            let updated = match serde_json::from_value(incoming.data) {
                Ok(updated) => updated,
                Err(error) => {
                    debug!("Failed to deserialize update for {}: {:?}", self.id, error);
                    continue;
                }
            };

            if let Some(component) = storage.get_mut(incoming.entity.into()) {
                *component = updated;
            }
        }
    }
}
