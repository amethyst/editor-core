use amethyst::ecs::Entity;
use crossbeam_channel::Sender;
use serde::Serialize;
use serializable_entity::DeserializableEntity;
use std::collections::HashMap;

pub(crate) type ChannelMap<T> = HashMap<&'static str, Sender<T>>;
pub(crate) type ComponentMap = ChannelMap<IncomingComponent>;
pub(crate) type ResourceMap = ChannelMap<serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Message<T> {
    #[serde(rename = "type")]
    ty: &'static str,
    data: T,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct SerializedComponent<'a, T: 'a> {
    pub name: &'static str,
    pub data: HashMap<u32, &'a T>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SerializedResource<'a, T: 'a> {
    pub name: &'static str,
    pub data: &'a T,
}

pub enum SerializedData {
    Resource(String),
    Component(String),
    Message(String),
}

pub enum EntityMessage {
    Create(usize),
    Destroy(Vec<u32>),
}

/// Messages sent from the editor to the game.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    ComponentUpdate {
        id: String,
        entity: DeserializableEntity,
        data: serde_json::Value,
    },

    ResourceUpdate {
        id: String,
        data: serde_json::Value,
    },

    CreateEntities {
        amount: usize,
    },

    DestroyEntities {
        entities: Vec<DeserializableEntity>,
    },
}

#[derive(Debug, Clone)]
pub struct IncomingComponent {
    pub entity: Entity,
    pub data: serde_json::Value,
}

/// A connection to an editor which allows sending messages via a [`SyncEditorSystem`].
///
/// Anything that needs to be able to send messages to the editor needs such a connection.
#[derive(Clone)]
pub struct EditorConnection {
    sender: Sender<SerializedData>,
}

impl EditorConnection {
    /// Construct a connection to the editor via sending messages to the [`SyncEditorSystem`].
    pub(crate) fn new(sender: Sender<SerializedData>) -> Self {
        Self { sender }
    }

    /// Send serialized data to the editor.
    pub(crate) fn send_data(&self, data: SerializedData) {
        self.sender
            .send(data)
            .expect("Disconnected from editor sync system");
    }

    /// Send an arbitrary message to the editor.
    ///
    /// Note that the message types supported by the editor may differ between implementations.
    pub fn send_message<T: Serialize>(&self, message_type: &'static str, data: T) {
        let serialize_data = Message {
            ty: message_type,
            data,
        };
        if let Ok(serialized) = serde_json::to_string(&serialize_data) {
            self.send_data(SerializedData::Message(serialized));
        } else {
            error!("Failed to serialize message");
        }
    }
}
