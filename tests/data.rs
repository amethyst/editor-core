extern crate amethyst;
extern crate amethyst_editor_sync;
#[macro_use]
extern crate serde;

use amethyst::prelude::*;
use amethyst_editor_sync::*;

#[test]
fn many_entities() -> amethyst::Result<()> {
    #[derive(Debug, Clone, Copy, Default)]
    struct TestState {
        frames: usize,
    }

    impl<'a, 'b> State<GameData<'a, 'b>> for TestState {
        fn on_start(&mut self, data: StateData<GameData>) {
            for _ in 0..2_500 {
                data.world.create_entity().build();
            }
        }

        fn update(&mut self, data: StateData<GameData>) -> Trans<GameData<'a, 'b>> {
            data.data.update(&data.world);

            self.frames += 1;
            if self.frames > 10 {
                Trans::Quit
            } else {
                Trans::None
            }
        }
    }

    let editor_sync_bundle = SyncEditorBundle::new();
    let game_data = GameDataBuilder::default()
        .with_bundle(editor_sync_bundle)?;
    let mut game = Application::build(".", TestState::default())?
        .build(game_data)?;
    game.run();

    Ok(())
}
