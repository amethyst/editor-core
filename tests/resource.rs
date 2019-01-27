extern crate amethyst;
extern crate amethyst_editor_sync;
extern crate serde;
extern crate tap;

use amethyst::prelude::*;
use amethyst_editor_sync::*;
use serde::*;
use tap::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SimpleResource {
    value: usize,
}

#[test]
fn serialize_resource() -> amethyst::Result<()> {
    #[derive(Debug, Clone, Copy, Default)]
    struct TestState {
        frames: usize,
    };

    impl SimpleState for TestState {
        fn on_start(&mut self, data: StateData<GameData>) {
            data.world.add_resource(SimpleResource { value: 123 });
        }

        fn update(&mut self, data: &mut StateData<GameData>) -> SimpleTrans {
            data.data.update(&data.world);

            self.frames += 1;
            if self.frames > 10 {
                Trans::Quit
            } else {
                Trans::None
            }
        }
    }

    let editor_sync_bundle =
        SyncEditorBundle::default().tap(|bundle| sync_resources!(bundle, SimpleResource));

    let game_data = GameDataBuilder::default().with_bundle(editor_sync_bundle)?;
    let mut game = Application::build(".", TestState::default())?.build(game_data)?;

    game.run();

    Ok(())
}

#[test]
fn missing_resource() -> amethyst::Result<()> {
    #[derive(Debug, Clone, Copy, Default)]
    struct TestState {
        frames: usize,
    };

    impl SimpleState for TestState {
        fn update(&mut self, data: &mut StateData<GameData>) -> SimpleTrans {
            data.data.update(&data.world);

            self.frames += 1;
            if self.frames > 10 {
                Trans::Quit
            } else {
                Trans::None
            }
        }
    }

    let editor_sync_bundle =
        SyncEditorBundle::default().tap(|bundle| sync_resources!(bundle, SimpleResource));

    let game_data = GameDataBuilder::default().with_bundle(editor_sync_bundle)?;
    let mut game = Application::build(".", TestState::default())?.build(game_data)?;

    game.run();

    Ok(())
}
