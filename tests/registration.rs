extern crate amethyst;
extern crate amethyst_editor_sync;
extern crate serde;

use amethyst::prelude::*;
use amethyst::ecs::*;
use amethyst_editor_sync::*;
use serde::*;

#[test]
fn empty() {
    let mut editor_bundle = SyncEditorBundle::new();
    editor_bundle.sync_default_types();

    let _ = GameDataBuilder::default()
        .with_bundle(editor_bundle);
}

#[test]
fn register_component() {
    #[derive(Serialize, Deserialize)]
    struct Foo;

    impl Component for Foo {
        type Storage = DenseVecStorage<Self>;
    }

    let mut editor_bundle = SyncEditorBundle::new();
    editor_bundle.sync_default_types();
    editor_bundle.sync_component::<Foo>("Foo");

    let _ = GameDataBuilder::default()
        .with_bundle(editor_bundle);
}
