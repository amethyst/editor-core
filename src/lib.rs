//! Provides functionality that allows an Amethyst game to communicate with an editor.
//!
//! [`SyncEditorSystem`] is the root system that will send your game's state data to an editor.
//! In order to visualize your game's state in an editor, you'll also need to create a
//! [`ReadComponentSystem`] or [`ReadResourceSystem`] for each component and resource that you want
//! to visualize. It is possible to automatically create these systems by creating a
//! [`SyncEditorBundle`] and registering each component and resource on it instead.
//!
//! # Example
//!
//! ```
//! extern crate amethyst;
//! extern crate amethyst_editor_sync;
//! extern crate serde;
//! extern crate tap;
//!
//! use amethyst::core::Transform;
//! use amethyst::ecs::*;
//! use amethyst::prelude::*;
//! use amethyst_editor_sync::*;
//! use serde::*;
//! use tap::*;
//!
//! # fn main() -> Result<(), amethyst::Error> {
//! // Create a SyncEditorBundle which will create all necessary systems to send the components
//! // to the editor.
//! let editor_sync_bundle = SyncEditorBundle::new()
//!     // Register the default types from the engine.
//!     .tap(SyncEditorBundle::sync_default_types)
//!     // Register the components and resources specified above.
//!     .tap(|bundle| bundle.sync_component::<MyComponent>("MyComponent"))
//!     .tap(|bundle| bundle.sync_resource::<MyResource>("MyResource"))
//!     .tap(|bundle| bundle.read_resource::<ReadOnlyResource>("ReadOnlyResource"));
//!
//! let game_data = GameDataBuilder::default()
//!     .with_bundle(editor_sync_bundle)?;
//! # Ok(())
//! # }
//!
//! // Make sure you enable serialization for your custom components and resources!
//! #[derive(Serialize, Deserialize)]
//! struct MyComponent {
//!     foo: usize,
//!     bar: String,
//! }
//!
//! impl Component for MyComponent {
//!     type Storage = DenseVecStorage<Self>;
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyResource {
//!     baz: usize,
//! }
//!
//! #[derive(Serialize)]
//! struct ReadOnlyResource {
//!     important_entity: SerializableEntity,
//! }
//! ```
//!
//! # Usage
//! First, specify the components and resources that you want to see in the editor using the
//! [`type_set!`] macro.
//! Then create a [`SyncEditorBundle`] object and register the specified components and resources
//! with `sync_components` and `sync_resources` respectively. Some of the engine-specific types can
//! be registered automatically using the `sync_default_types` method. It is also possible to
//! specify the types individually using `sync_component` and `sync_resource`, which allows changing
//! the name of the type when viewed in the editor.
//!
//! Finally, add the [`SyncEditorBundle`] that you created to the game data.

extern crate amethyst;
extern crate crossbeam_channel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate log_once;
#[macro_use]
extern crate serde;
extern crate serde_json;

pub use bundle::SyncEditorBundle;
pub use editor_log::EditorLogger;
pub use serializable_entity::SerializableEntity;

mod bundle;
mod editor_log;
mod serializable_entity;
mod systems;
mod types;
