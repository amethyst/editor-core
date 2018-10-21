use amethyst::ecs::Entity;

mod bounce;
mod move_balls;
mod paddle;
mod score_text;
mod winner;

pub use self::bounce::BounceSystem;
pub use self::move_balls::MoveBallsSystem;
pub use self::paddle::PaddleSystem;
pub use self::score_text::ScoreTextSystem;
pub use self::winner::WinnerSystem;

/// Stores the entities that are displaying the player score with UiText.
pub struct ScoreText {
    pub p1_score: Entity,
    pub p2_score: Entity,
}
