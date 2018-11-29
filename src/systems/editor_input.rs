use amethyst::ecs::{Entities, System};
use crossbeam_channel::Sender;
use std::io;
use std::net::UdpSocket;
use std::str;
use types::{ComponentMap, EntityMessage, IncomingComponent, IncomingMessage, ResourceMap};

pub struct EditorInputSystem {
    socket: UdpSocket,

    // Map containing channels used to send incoming serialized component/resource data from the
    // editor. Incoming data is sent to specialized systems that deserialize the data and update
    // the corresponding local data.
    component_map: ComponentMap,
    resource_map: ResourceMap,
    entity_handler: Sender<EntityMessage>,
    incoming_buffer: Vec<u8>,
}

impl EditorInputSystem {
    pub fn new(
        component_map: ComponentMap,
        resource_map: ResourceMap,
        entity_handler: Sender<EntityMessage>,
        socket: UdpSocket,
    ) -> EditorInputSystem {
        // Create the socket used for communicating with the editor.
        //
        // NOTE: We set the socket to nonblocking so that we don't block if there are no incoming
        // messages to read. We `expect` on the call to `set_nonblocking` because the game will
        // hang if the socket is still set to block when the game runs.
        EditorInputSystem {
            socket,
            component_map,
            resource_map,
            entity_handler,
            incoming_buffer: Vec::with_capacity(1024),
        }
    }
}

impl<'a> System<'a> for EditorInputSystem {
    type SystemData = Entities<'a>;

    fn run(&mut self, entities: Self::SystemData) {
        let editor_address = ([127, 0, 0, 1], 8000).into();

        // Read any incoming messages from the editor process.
        let mut buf = [0; 1024];
        loop {
            // TODO: Verify that the incoming address matches the editor process address.
            let (bytes_read, addr) = match self.socket.recv_from(&mut buf[..]) {
                Ok(res) => res,
                Err(error) => {
                    match error.kind() {
                        // If the read would block, it means that there was no incoming data and we
                        // should break from the loop.
                        io::ErrorKind::WouldBlock => break,

                        // This is an "error" that happens on Windows if no editor is running to
                        // receive the state update we just sent. The OS gives a "connection was
                        // forcibly closed" error when no socket receives the message, but we
                        // don't care if that happens (in fact, we use UDP specifically so that
                        // we can broadcast messages without worrying about establishing a
                        // connection).
                        io::ErrorKind::ConnectionReset => continue,

                        // All other error kinds should be indicative of a genuine error. For our
                        // purposes we still want to ignore them, but we'll at least log a warning
                        // in case it helps debug an issue.
                        _ => {
                            warn!("Error reading incoming: {:?}", error);
                            continue;
                        }
                    }
                }
            };

            if addr != editor_address {
                trace!("Packet received from unknown address {:?}", addr);
                continue;
            }

            debug!("Packet: {:?}", &buf[..bytes_read]);

            // Add the bytes from the incoming packet to the buffer.
            self.incoming_buffer.extend_from_slice(&buf[..bytes_read]);
        }

        // Check the incoming buffer to see if any completed messages have been received.
        while let Some(index) = self.incoming_buffer.iter().position(|&byte| byte == 0xC) {
            // HACK: Manually introduce a scope here so that the compiler can tell when we're done
            // using borrowing the message bytes from `self.incoming_buffer`. This can be removed
            // once NLL is stable.
            {
                let message_bytes = &self.incoming_buffer[..index];
                let result = str::from_utf8(message_bytes)
                    .ok()
                    .and_then(|message| serde_json::from_str(message).ok());
                debug!("Message str: {:?}", result);

                if let Some(message) = result {
                    debug!("Message: {:#?}", message);

                    match message {
                        IncomingMessage::ComponentUpdate {
                            id,
                            entity: entity_data,
                            data,
                        } => {
                            let entity = entities.entity(entity_data.id);

                            // Skip the update if the entity is no longer valid.
                            if entity.gen().id() != entity_data.generation {
                                debug!(
                                    "Entity {:?} had invalid generation {} (expected {})",
                                    entity_data,
                                    entity_data.generation,
                                    entity.gen().id()
                                );
                                continue;
                            }

                            if let Some(sender) = self.component_map.get(&*id) {
                                sender.0.send(IncomingComponent { entity, data });
                            } else {
                                debug!("No deserializer found for component {:?}", id);
                            }
                        }

                        IncomingMessage::ResourceUpdate { id, data } => {
                            // TODO: Should we do something if there was no deserialer system for the
                            // specified ID?
                            if let Some(sender) = self.resource_map.get(&*id) {
                                // TODO: Should we do something to prevent this from blocking?
                                sender.0.send(data);
                            }
                        }

                        IncomingMessage::CreateEntities { amount } => {
                            self.entity_handler.send(EntityMessage::Create(amount));
                        }

                        IncomingMessage::DestroyEntities { entities } => {
                            self.entity_handler.send(EntityMessage::Destroy(
                                entities.iter().map(|e| e.id).collect(),
                            ));
                        }
                    }
                }
            }

            // Remove the message bytes from the beginning of the incoming buffer.
            self.incoming_buffer.drain(..=index);
        }
    }
}
