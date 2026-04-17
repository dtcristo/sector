mod physics;
mod player;

use crate::render::RenderView;

pub use crate::player::{
    EARTH_GRAVITY_MPS2, PLAYER_CROUCH_EYE_HEIGHT_METERS, PLAYER_CROUCH_HEIGHT_METERS,
    PLAYER_EYE_HEIGHT_METERS, PLAYER_HEIGHT_METERS, PLAYER_JUMP_HEIGHT_METERS,
    PLAYER_MAX_STEP_HEIGHT_METERS, PLAYER_RADIUS_METERS, PLAYER_WALK_SPEED_MPS,
};
pub use physics::{resolve_current_sector, sector_contains_player, simulate_player};
pub use player::{
    apply_player_look, desired_horizontal_velocity, jump_speed_mps, setup_player_system, Direction,
    Player, PlayerInput,
};

pub fn player_render_view(player: &Player) -> RenderView {
    RenderView::new(
        player.eye_position(),
        player.direction.0,
        player.current_sector,
    )
}
