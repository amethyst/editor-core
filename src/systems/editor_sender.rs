use amethyst::ecs::{Entities, Join, System};
use crossbeam_channel::Receiver;
use serializable_entity::SerializableEntity;
use std::cmp::min;
use std::fmt::Write;
use std::net::UdpSocket;
use std::time::{Duration, Instant};
use types::SerializedData;

const MAX_PACKET_SIZE: usize = 32 * 1024;

/// The system in charge of sending updated state data to the editor process.
pub struct EditorSenderSystem {
    receiver: Receiver<SerializedData>,
    socket: UdpSocket,

    send_interval: Duration,
    next_send: Instant,

    scratch_string: String,
}

impl EditorSenderSystem {
    pub fn from_channel(
        receiver: Receiver<SerializedData>,
        send_interval: Duration,
        socket: UdpSocket,
    ) -> Self {
        // Create the socket used for communicating with the editor.
        //
        // NOTE: We set the socket to nonblocking so that we don't block if there are no incoming
        // messages to read. We `expect` on the call to `set_nonblocking` because the game will
        // hang if the socket is still set to block when the game runs.
        let scratch_string = String::with_capacity(MAX_PACKET_SIZE);
        EditorSenderSystem {
            receiver,
            socket,

            send_interval,
            next_send: Instant::now() + send_interval,

            scratch_string,
        }
    }
}

impl<'a> System<'a> for EditorSenderSystem {
    type SystemData = Entities<'a>;

    fn run(&mut self, entities: Self::SystemData) {
        // Determine if we should send full state data this frame.
        let now = Instant::now();
        let send_this_frame = now >= self.next_send;

        // Calculate when we should next send full state data.
        //
        // NOTE: We do `next_send += send_interval` instead of `next_send = now + send_interval`
        // to ensure that state updates happen at a consistent cadence even if there are slight
        // timing variations in when individual frames are sent.
        //
        // NOTE: We repeatedly add `send_interval` to `next_send` to ensure that the next send
        // time is after `now`. This is to avoid running into a death spiral if a frame spike
        // causes frame time to be so long that the next send time would still be in the past.
        while self.next_send < now {
            self.next_send += self.send_interval;
        }

        let mut components = Vec::new();
        let mut resources = Vec::new();
        let mut messages = Vec::new();
        while let Ok(serialized) = self.receiver.try_recv() {
            match serialized {
                SerializedData::Component(c) => components.push(c),
                SerializedData::Resource(r) => resources.push(r),
                SerializedData::Message(m) => messages.push(m),
            }
        }

        let mut entity_data = Vec::<SerializableEntity>::new();
        for (entity,) in (&*entities,).join() {
            entity_data.push(entity.into());
        }
        let entity_string =
            serde_json::to_string(&entity_data).expect("Failed to serialize entities");

        // Create the message and serialize it to JSON. If we don't need to send the full state
        // data this frame, we discard entities, components, and resources, and only send the
        // messages (e.g. log output) from the current frame.
        if send_this_frame {
            write!(
                self.scratch_string,
                r#"{{
                    "type": "message",
                    "data": {{
                        "entities": {},
                        "components": [{}],
                        "resources": [{}],
                        "messages": [{}]
                    }}
                }}"#,
                entity_string,
                // Insert a comma between components so that it's valid JSON.
                components.join(","),
                resources.join(","),
                messages.join(","),
            )
            .expect("Failed to write JSON string");
        } else {
            write!(
                self.scratch_string,
                r#"{{
                    "type": "message",
                    "data": {{
                        "messages": [{}]
                    }}
                }}"#,
                // Insert a comma between components so that it's valid JSON.
                messages.join(","),
            )
            .expect("Failed to write JSON string");
        }

        // NOTE: We need to append a page feed character after each message since that's
        // what node-ipc expects to delimit messages.
        self.scratch_string.push_str("\u{C}");

        // Send the message, breaking it up into multiple packets if the message is too large.
        let editor_address: std::net::SocketAddr = ([127, 0, 0, 1], 8000).into();
        let mut bytes_sent = 0;
        while bytes_sent < self.scratch_string.len() {
            let bytes_to_send = min(self.scratch_string.len() - bytes_sent, MAX_PACKET_SIZE);
            let end_offset = bytes_sent + bytes_to_send;

            // Send the JSON message.
            let bytes = self.scratch_string[bytes_sent..end_offset].as_bytes();
            self.socket
                .send_to(bytes, editor_address)
                .expect("Failed to send message");

            bytes_sent += bytes_to_send;
        }

        self.scratch_string.clear();
    }
}
