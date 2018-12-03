extern crate amethyst;
extern crate amethyst_editor_sync;
extern crate serde;

use amethyst::prelude::*;
use amethyst::ecs::*;
use amethyst_editor_sync::*;
use serde::*;

#[test]
fn empty() {
    let components = type_set![];
    let resources = type_set![];

    let editor_bundle = SyncEditorBundle::new()
        .sync_default_types()
        .sync_components(&components)
        .sync_resources(&resources);

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

    let components = type_set![Foo];

    let editor_bundle = SyncEditorBundle::new()
        .sync_default_types()
        .sync_components(&components);

    let _ = GameDataBuilder::default()
        .with_bundle(editor_bundle);
}
