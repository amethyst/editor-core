//! Allows an Amethyst game to communicate with a visualizer/debugger.
//!
//! This crate provides the hooks necessary for an Amethyst game to communicate
//! with a visualizer/debugger running in another process. Once setup, it will
//! send the game's state to the debugger, and can apply commands sent back.
//!
//! # Usage
//!
//! In order to communicate with the editor, you must create a [`SyncEditorBundle`]
//! and register all of the components and resources that you want to display
//! or interact with in the debugger. Any component or resource that implements
//! `Serialize` can be displayed, and any that implements `Deserialize`
//! can also be modified at runtime within the debugger.
//!
//! Create an empty bundle with [`SyncEditorBundle::new`], and then use the
//! various helper macros to register your custom types:
//!
//! * [`sync_components`]
//! * [`read_components`]
//! * [`sync_resources`]
//! * [`read_resources`]
//!
//! You can also use the [`SyncEditorBundle::sync_default_types`] method to
//! register all of the types provided by Amethyst that can be supported by
//! the debugger.
//!
//! If you'd like a builder-like way of chaining method calls together in order
//! to build your entire bundle in a single statement, we recommend using the
//! [tap] crate to do so. The examples below demonstrate using tap in
//! conjuction with the helper macros to succinctly register a variety of
//! custom types.
//!
//! **It is highly recommended to register the bundle last, to ensure the editor
//! receives values after all systems have updated them**
//!
//! # Examples
//!
//! ```
//! extern crate amethyst;
//! extern crate amethyst_editor_sync;
//! extern crate serde;
//! extern crate tap;
//!
//! use amethyst::ecs::*;
//! use amethyst::prelude::*;
//! use amethyst_editor_sync::*;
//! use serde::*;
//! use tap::*;
//!
//! # fn main() -> Result<(), amethyst::Error> {
//! // Create a SyncEditorBundle which will create all necessary systems to send the components
//! // to the editor.
//! let editor_sync_bundle = SyncEditorBundle::default()
//!
//!     // Register the default types from the engine.
//!     .tap(SyncEditorBundle::sync_default_types)
//!
//!     // Register any custom components and resources for your game. By default, components
//!     // and resources support reading and writing, allowing you to modify values at runtime
//!     // from the editor. If your component should be read-only, use the `read_*` variant
//!     // when registering the type.
//!     .tap(|bundle| sync_components!(bundle, MyComponent))
//!     .tap(|bundle| sync_resources!(bundle, MyResource))
//!     .tap(|bundle| read_resources!(bundle, ReadOnlyResource));
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
//! // This resource can't be deserialized because it contains an Entity.
//! // As such, we register it as read-only when setting up editor support.
//! #[derive(Serialize)]
//! struct ReadOnlyResource {
//!     important_entity: SerializableEntity,
//! }
//! ```
//!
//! [`SyncEditorBundle`]: ./struct.SyncEditorBundle.html
//! [`SyncEditorBundle::default()`]: ./struct.SyncEditorBundle.html#method.default
//! [`sync_components`]: ./macro.sync_components.html
//! [`read_components`]: ./macro.read_components.html
//! [`sync_resources`]: ./macro.sync_resources.html
//! [`read_resources`]: ./macro.read_resources.html
//! [`SyncEditorBundle::sync_default_types`]: ./struct.SyncEditorBundle.html#method.sync_default_types
//! [tap]: https://crates.io/crates/tap

extern crate amethyst;
extern crate crossbeam_channel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate log_once;
#[macro_use]
extern crate serde;
extern crate serde_json;

pub use crate::bundle::SyncEditorBundle;
pub use crate::editor_log::EditorLogger;
pub use crate::serializable_entity::SerializableEntity;

mod bundle;
mod editor_log;
mod serializable_entity;
mod systems;
mod types;
