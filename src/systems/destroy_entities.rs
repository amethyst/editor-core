use amethyst::ecs::{Entities, System};
use crossbeam_channel::Receiver;
use types::EntityMessage;

pub(crate) struct DestroyEntitiesSystem {
    receiver: Receiver<EntityMessage>,
}

impl DestroyEntitiesSystem {
    pub(crate) fn new(receiver: Receiver<EntityMessage>) -> Self {
        DestroyEntitiesSystem { receiver }
    }
}

impl<'a> System<'a> for DestroyEntitiesSystem {
    type SystemData = Option<Entities<'a>>;

    fn run(&mut self, data: Self::SystemData) {
        trace!("`DestroyEntitiesSystem::run`");

        let entities = match data {
            Some(res) => res,
            None => return,
        };

        while let Ok(message) = self.receiver.try_recv() {
            match message {
                EntityMessage::Destroy(ids) => {
                    for id in ids {
                        let entity = entities.entity(id);
                        entities.delete(entity);
                    }                    
                }
                _ => (),
            }
        }
    }
}
