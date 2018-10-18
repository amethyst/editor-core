use ::{ScoreBoard};
use ::systems::ScoreText;
use amethyst::ecs::prelude::{Read, ReadExpect, System, WriteStorage};
use amethyst::ui::UiText;

pub struct ScoreTextSystem;

impl<'s> System<'s> for ScoreTextSystem {
    type SystemData = (
        WriteStorage<'s, UiText>,
        Read<'s, ScoreBoard>,
        ReadExpect<'s, ScoreText>,
    );

    fn run(
        &mut self,
        (
            mut text,
            score_board,
            score_text,
        ): Self::SystemData,
    ) {
        if let Some(text) = text.get_mut(score_text.p2_score) {
            text.text = score_board.score_right.to_string();
        }

        if let Some(text) = text.get_mut(score_text.p1_score) {
            text.text = score_board.score_left.to_string();
        }
    }
}
