use crate::{
    player::{
        EARTH_GRAVITY_MPS2, PLAYER_CROUCH_EYE_HEIGHT_METERS, PLAYER_CROUCH_HEIGHT_METERS,
        PLAYER_EYE_HEIGHT_METERS, PLAYER_HEIGHT_METERS, PLAYER_JUMP_HEIGHT_METERS,
        PLAYER_WALK_SPEED_MPS,
    },
    Position3, SectorId,
};

use bevy::{
    ecs::system::Commands,
    input::ButtonInput,
    math::vec3,
    prelude::{Component, KeyCode, Vec3},
};

const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;
const KEYBOARD_LOOK_STEP: f32 = 0.04;

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Direction(pub f32);

#[derive(Component, Debug, Copy, Clone, PartialEq)]
pub struct Player {
    /// Player feet position in meters.
    pub position: Position3,
    pub velocity: Vec3,
    pub direction: Direction,
    pub current_sector: Option<SectorId>,
    pub grounded: bool,
    pub crouching: bool,
    pub noclip: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: Position3(vec3(0.0, 0.0, 0.0)),
            velocity: Vec3::ZERO,
            direction: Direction(0.0),
            current_sector: None,
            grounded: true,
            crouching: false,
            noclip: false,
        }
    }
}

impl Player {
    pub fn eye_position(self) -> Position3 {
        Position3(self.position.0 + Vec3::Z * self.eye_height())
    }

    pub fn head_z(self) -> f32 {
        self.position.0.z + self.height()
    }

    pub fn height(self) -> f32 {
        if self.crouching {
            PLAYER_CROUCH_HEIGHT_METERS
        } else {
            PLAYER_HEIGHT_METERS
        }
    }

    pub fn eye_height(self) -> f32 {
        if self.crouching {
            PLAYER_CROUCH_EYE_HEIGHT_METERS
        } else {
            PLAYER_EYE_HEIGHT_METERS
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub strafe_left: bool,
    pub strafe_right: bool,
    pub jump_pressed: bool,
    pub crouch_pressed: bool,
    pub turn_left: bool,
    pub turn_right: bool,
    pub mouse_delta_x: f32,
    pub mouse_look_enabled: bool,
}

impl PlayerInput {
    pub fn from_keys(keys: &ButtonInput<KeyCode>, jump_pressed: bool) -> Self {
        Self {
            forward: keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW),
            backward: keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS),
            strafe_left: keys.pressed(KeyCode::KeyA),
            strafe_right: keys.pressed(KeyCode::KeyD),
            jump_pressed,
            crouch_pressed: keys.pressed(KeyCode::ControlLeft)
                || keys.pressed(KeyCode::ControlRight),
            turn_left: keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyQ),
            turn_right: keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyE),
            ..Self::default()
        }
    }

    pub fn with_mouse_look(mut self, mouse_delta_x: f32, enabled: bool) -> Self {
        self.mouse_delta_x = mouse_delta_x;
        self.mouse_look_enabled = enabled;
        self
    }
}

pub fn setup_player_system(mut commands: Commands) {
    commands.spawn(Player::default());
}

pub fn apply_player_look(player: &mut Player, input: PlayerInput) {
    if input.mouse_look_enabled {
        player.direction.0 += -input.mouse_delta_x * MOUSE_LOOK_SENSITIVITY;
    }

    if input.turn_left {
        player.direction.0 += KEYBOARD_LOOK_STEP;
    }
    if input.turn_right {
        player.direction.0 -= KEYBOARD_LOOK_STEP;
    }
}

pub fn desired_horizontal_velocity(player: &Player, input: PlayerInput) -> Vec3 {
    let mut velocity = Vec3::ZERO;

    if input.forward {
        velocity.x -= player.direction.0.sin();
        velocity.y += player.direction.0.cos();
    }
    if input.backward {
        velocity.x += player.direction.0.sin();
        velocity.y -= player.direction.0.cos();
    }
    if input.strafe_left {
        velocity.x -= player.direction.0.cos();
        velocity.y -= player.direction.0.sin();
    }
    if input.strafe_right {
        velocity.x += player.direction.0.cos();
        velocity.y += player.direction.0.sin();
    }

    if velocity.length_squared() > 1.0 {
        velocity = velocity.normalize();
    }

    velocity * PLAYER_WALK_SPEED_MPS
}

pub fn jump_speed_mps() -> f32 {
    (2.0 * EARTH_GRAVITY_MPS2 * PLAYER_JUMP_HEIGHT_METERS).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonInput;

    #[test]
    fn player_moves_forward_from_default_direction() {
        let player = Player::default();
        let velocity = desired_horizontal_velocity(
            &player,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
        );
        assert_eq!(velocity, vec3(0.0, PLAYER_WALK_SPEED_MPS, 0.0));
    }

    #[test]
    fn mouse_look_only_applies_when_enabled() {
        let mut player = Player::default();
        apply_player_look(
            &mut player,
            PlayerInput::default().with_mouse_look(10.0, false),
        );
        assert_eq!(player.direction.0, 0.0);

        apply_player_look(
            &mut player,
            PlayerInput::default().with_mouse_look(10.0, true),
        );
        assert!((player.direction.0 + 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn keyboard_turn_uses_larger_step() {
        let mut player = Player::default();

        apply_player_look(
            &mut player,
            PlayerInput {
                turn_left: true,
                ..PlayerInput::default()
            },
        );

        assert!((player.direction.0 - 0.04).abs() < f32::EPSILON);
    }

    #[test]
    fn jump_speed_matches_target_height() {
        let apex_height = jump_speed_mps() * jump_speed_mps() / (2.0 * EARTH_GRAVITY_MPS2);
        assert!((apex_height - PLAYER_JUMP_HEIGHT_METERS).abs() < 0.0001);
    }

    #[test]
    fn control_keys_drive_crouch_input() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ControlLeft);

        assert!(PlayerInput::from_keys(&keys, false).crouch_pressed);
    }
}
