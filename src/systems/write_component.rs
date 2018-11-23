use amethyst::ecs::prelude::*;
use serde::de::DeserializeOwned;
use serde_json;
use std::marker::PhantomData;
use types::IncomingComponent;

pub(crate) struct WriteComponentSystem<T>
where
    T: Sync + Send + 'static,
{
    id: &'static str,
    reader: crossbeam_channel::Receiver<IncomingComponent>,
    _marker: PhantomData<T>,
}

impl<T> WriteComponentSystem<T>
where
    T: Sync + Send + 'static,
{
    pub(crate) fn new(
        id: &'static str,
        reader: crossbeam_channel::Receiver<IncomingComponent>,
    ) -> Self {
        WriteComponentSystem {
            id,
            reader,
            _marker: PhantomData,
        }
    }
}

impl<'a, T> System<'a> for WriteComponentSystem<T>
where
    T: Component + DeserializeOwned + Send + Sync,
{
    type SystemData = WriteStorage<'a, T>;

    fn run(&mut self, mut storage: Self::SystemData) {
        //println!("`WriteComponentSystem::run` for {}", self.id);

        while let Ok(event) = self.reader.try_recv() {
            println!("Got incoming message for {}: {:?}", self.id, event.data);

            let updated = match serde_json::from_value(event.data.clone()) {
                Ok(updated) => updated,
                Err(error) => {
                    println!("Failed to deserialize update for {}: {:?}", self.id, error);
                    continue;
                }
            };

            if let Some(component) = storage.get_mut(event.entity) {
                *component = updated;
            }
        }
    }
}
