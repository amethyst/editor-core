extern crate amethyst;
extern crate amethyst_editor_sync;
#[macro_use]
extern crate serde;

use amethyst::prelude::*;
use amethyst_editor_sync::*;

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

    impl<'a, 'b> SimpleState<'a, 'b> for TestState {
        fn on_start(&mut self, data: StateData<GameData>) {
            data.world.add_resource(SimpleResource { value: 123 });
        }

        fn update(&mut self, data: &mut StateData<GameData>) -> SimpleTrans<'a, 'b> {
            data.data.update(&data.world);

            self.frames += 1;
            if self.frames > 10 {
                Trans::Quit
            } else {
                Trans::None
            }
        }
    }

    let mut editor_sync_bundle = SyncEditorBundle::new();
    editor_sync_bundle.sync_resource::<SimpleResource>("Test State");

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

    impl<'a, 'b> SimpleState<'a, 'b> for TestState {
        fn update(&mut self, data: &mut StateData<GameData>) -> SimpleTrans<'a, 'b> {
            data.data.update(&data.world);

            self.frames += 1;
            if self.frames > 10 {
                Trans::Quit
            } else {
                Trans::None
            }
        }
    }

    let mut editor_sync_bundle = SyncEditorBundle::new();
    editor_sync_bundle.sync_resource::<SimpleResource>("Test State");

    let game_data = GameDataBuilder::default().with_bundle(editor_sync_bundle)?;
    let mut game = Application::build(".", TestState::default())?.build(game_data)?;

    game.run();

    Ok(())
}
