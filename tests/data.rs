extern crate amethyst;
extern crate amethyst_editor_sync;

use amethyst::prelude::*;
use amethyst_editor_sync::*;

#[derive(Debug, Clone, Copy, Default)]
struct TestState {
    num_entities: usize,
    frames: usize,
}

impl TestState {
    fn new(num_entities: usize) -> Self {
        TestState {
            num_entities,
            frames: 0,
        }
    }
}

impl<'a, 'b> SimpleState<'a, 'b> for TestState {
    fn on_start(&mut self, data: StateData<GameData>) {
        for _ in 0..self.num_entities {
            data.world.create_entity().build();
        }
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

fn run_world(num_entities: usize) -> amethyst::Result<()> {
    let editor_sync_bundle = SyncEditorBundle::new();
    let game_data = GameDataBuilder::default()
        .with_bundle(editor_sync_bundle)?;
    let mut game = Application::build(".", TestState::new(num_entities))?
        .build(game_data)?;
    game.run();

    Ok(())
}

#[test]
fn small_world() -> amethyst::Result<()> {
    run_world(500)
}

#[test]
fn med_world() -> amethyst::Result<()> {
    run_world(2_500)
}

#[test]
fn large_world() -> amethyst::Result<()> {
    run_world(10_000)
}

#[test]
fn huge_world() -> amethyst::Result<()> {
    run_world(100_000)
}
