extern crate amethyst;
extern crate amethyst_editor_sync;
extern crate serde;
extern crate tap;

use amethyst::ecs::*;
use amethyst::prelude::*;
use amethyst_editor_sync::*;
use serde::*;
use tap::*;

#[test]
fn empty() {
    let editor_bundle = SyncEditorBundle::default().tap(SyncEditorBundle::sync_default_types);

    let _ = GameDataBuilder::default().with_bundle(editor_bundle);
}

#[test]
fn register_component() {
    #[derive(Serialize, Deserialize)]
    struct Foo;

    impl Component for Foo {
        type Storage = DenseVecStorage<Self>;
    }

    let editor_bundle = SyncEditorBundle::default()
        .tap(SyncEditorBundle::sync_default_types)
        .tap(|bundle| sync_components!(bundle, Foo));

    let _ = GameDataBuilder::default().with_bundle(editor_bundle);
}
