use serde::Serializer;
use serde::Serialize;
use serde::ser::SerializeStruct;
use amethyst::ecs::Entity;
use amethyst::ecs::world::Generation;
use std::fmt::{self, Debug, Formatter};

/// Helper type that wraps an [`Entity`] to provide serialization support.
///
/// [`Entity`] does not directly implement [`Serialize`] because it rarely makes sense to
/// serialize an entity directly. [Specs] encourages users to treat entities as a collection of
/// components, and to only serialize component data while letting the entity be implicit.
/// For the purposes of the editor, though, we would like to be able to reason about entities
/// directly. `SerializableEntity` acts as a minimal wrapper around [`Entity`] that provides
/// serialization support. You can use it in your components instead of [`Entity`] so that you
/// can `#[derive(Serialize)]` for your component type and display it in the editor.
///
/// Note that `SerializableEntity` does not support deserialization. A robust solution for
/// component deserialization is more complex than what is necessary for the editor at this point.
/// Users interested in full deserialization of entities should have a look at the [`saveload`]
/// functionality in specs.
///
/// [`Entity`]: https://docs.rs/specs/0.12/specs/struct.Entity.html
/// [`Serialize`]: https://docs.rs/serde/1/serde/trait.Serialize.html
/// [`saveload`]: https://docs.rs/specs/0.12/specs/saveload/index.html
/// [Specs]: https://crates.io/crates/specs
#[derive(Clone, Copy)]
pub struct SerializableEntity(pub Entity);

impl SerializableEntity {
    /// Creates a new `SerializableEntity` from an [`Entity`].
    ///
    /// [`Entity`]: https://docs.rs/specs/0.12/specs/struct.Entity.html
    pub fn new(entity: Entity) -> Self {
        SerializableEntity(entity)
    }

    /// Gets the ID of the entity.
    pub fn id(&self) -> u32 { self.0.id() }

    /// Gets the generation of the entity.
    pub fn gen(&self) -> Generation { self.0.gen() }
}

impl Serialize for SerializableEntity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Entity", 2)?;
        state.serialize_field("id", &self.0.id())?;
        state.serialize_field("generation", &self.0.gen().id())?;
        state.end()
    }
}

impl Debug for SerializableEntity {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(formatter)
    }
}

impl From<Entity> for SerializableEntity {
    fn from(from: Entity) -> Self {
        SerializableEntity(from)
    }
}

impl From<SerializableEntity> for Entity {
    fn from(from: SerializableEntity) -> Self {
        from.0
    }
}

/// Secret struct for easy serialization/deserialization of `Entity` within
/// `SerializableEntity`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct DeserializableEntity {
    id: u32,
    generation: i32,
}
