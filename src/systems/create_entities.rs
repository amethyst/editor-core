use amethyst::ecs::{Entities, System};
use crossbeam_channel::Receiver;
use types::EntityMessage;

/// A system that deserializes incoming updates for a resource and applies
/// them to the world state.
///
/// An instance of this system is created for each writable resource registered
/// with [`SyncEditorBundle`] by the player during setup for their game.
///
/// [`SyncEditorBundle`]: ./struct.SyncEditorBundle.html
pub(crate) struct CreateEntitiesSystem {
    receiver: Receiver<EntityMessage>,
}

impl CreateEntitiesSystem {
    pub(crate) fn new(receiver: Receiver<EntityMessage>) -> Self {
        CreateEntitiesSystem { receiver }
    }
}

impl<'a> System<'a> for CreateEntitiesSystem {
    type SystemData = Option<Entities<'a>>;

    fn run(&mut self, data: Self::SystemData) {
        trace!("`CreateEntitiesSystem::run`");

        let entities = match data {
            Some(res) => res,
            None => return,
        };

        while let Ok(message) = self.receiver.try_recv() {
            match message {
                EntityMessage::Create(amount) => {
                    let mut ids = Vec::with_capacity(amount);
                    for _ in 0..amount {
                        ids.push(entities.create().id());
                    }
                }
                _ => (),
            }
        }
    }
}
