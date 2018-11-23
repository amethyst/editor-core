use amethyst::ecs::*;
use crossbeam_channel::{Receiver, Sender};
use serde_json;
use types::EditorConnection;

/// A system that deserializes incoming updates for a resource and applies
/// them to the world state.
///
/// An instance of this system is created for each writable resource registered
/// with [`SyncEditorBundle`] by the player during setup for their game.
///
/// [`SyncEditorBundle`]: ./struct.SyncEditorBundle.html
pub(crate) struct CreateEntitiesSystem {
    sender: Sender<serde_json::Value>,
    receiver: Receiver<serde_json::Value>,
    connection: EditorConnection,
}

impl CreateEntitiesSystem {
    pub(crate) fn new(
        sender: Sender<serde_json::Value>,
        receiver: Receiver<serde_json::Value>,
        connection: EditorConnection,
    ) -> Self {
        CreateEntitiesSystem {
            sender,
            receiver,
            connection,
        }
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

        while let Ok(amount) = self.receiver.try_recv() {
            println!("Got incoming message for: {:?}", amount);

            let updated = match serde_json::from_value(amount) {
                Ok(updated) => updated,
                Err(error) => {
                    println!("Failed to deserialize amount: {:?}", error);
                    continue;
                }
            };

            let mut ids = Vec::with_capacity(updated);
            for _ in 0..updated {
                ids.push(entities.create().id());
            }
        }
    }
}
