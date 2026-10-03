use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sector::game::PlayerInput;

const DEAD_ZONE: f32 = 0.2;
const TURN_SPEED: f32 = 2.5;
const JUMP: u32 = 1;
const CROUCH: u32 = 2;
const SWAP_DPAD: u32 = 4;
const TOGGLE_CROUCH: u32 = 8;
const AUTOMAP: u32 = 16;
const LEFT_BUMPER: u32 = 32;
const RIGHT_BUMPER: u32 = 64;

#[derive(Default, Clone, Copy)]
struct ControllerSnapshot {
    id: u64,
    left_stick: Vec2,
    right_x: f32,
    dpad: Vec2,
    buttons: u32,
}

impl ControllerSnapshot {
    fn has_input(self) -> bool {
        self.buttons != 0
            || self.left_stick.length() > DEAD_ZONE
            || self.right_x.abs() > DEAD_ZONE
            || self.dpad != Vec2::ZERO
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn from_gamepad(entity: Entity, pad: &Gamepad) -> Self {
        let button = |button, mask| if pad.pressed(button) { mask } else { 0 };
        Self {
            id: entity.to_bits(),
            left_stick: pad.left_stick(),
            right_x: pad.right_stick().x,
            dpad: Vec2::new(
                f32::from(pad.pressed(GamepadButton::DPadRight))
                    - f32::from(pad.pressed(GamepadButton::DPadLeft)),
                f32::from(pad.pressed(GamepadButton::DPadUp))
                    - f32::from(pad.pressed(GamepadButton::DPadDown)),
            ),
            buttons: button(GamepadButton::South, JUMP)
                | button(GamepadButton::East, CROUCH)
                | button(GamepadButton::North, SWAP_DPAD)
                | button(GamepadButton::LeftThumb, TOGGLE_CROUCH)
                | button(GamepadButton::Select, AUTOMAP)
                | button(GamepadButton::LeftTrigger, LEFT_BUMPER)
                | button(GamepadButton::RightTrigger, RIGHT_BUMPER),
        }
    }
}

#[derive(Resource, Default)]
pub struct ControllerInput {
    pub active: bool,
    pub map_pending: bool,
    pub dpad_turns: bool,
    id: Option<u64>,
    last_buttons: u32,
    movement: Vec2,
    turn: f32,
    jump_pending: bool,
    crouch_toggle: bool,
    crouch_held: bool,
    jump_held: bool,
    waiting_for_neutral: bool,
}

impl ControllerInput {
    fn clear(&mut self) {
        let dpad_turns = self.dpad_turns;
        *self = Self {
            dpad_turns,
            ..default()
        };
    }

    fn sample(&mut self, snapshot: Option<ControllerSnapshot>) {
        let Some(snapshot) = snapshot else {
            self.clear();
            return;
        };
        if self.waiting_for_neutral {
            if snapshot.has_input() {
                return;
            }
            self.waiting_for_neutral = false;
        }
        if self.id != Some(snapshot.id) {
            self.clear();
            self.id = Some(snapshot.id);
        }
        let pressed = snapshot.buttons & !self.last_buttons;
        self.last_buttons = snapshot.buttons;
        self.active |= snapshot.has_input();
        if pressed & SWAP_DPAD != 0 {
            self.dpad_turns = !self.dpad_turns;
        }
        if pressed & TOGGLE_CROUCH != 0 {
            self.crouch_toggle = !self.crouch_toggle;
        }
        self.jump_pending |= pressed & JUMP != 0;
        self.map_pending |= pressed & AUTOMAP != 0;
        self.crouch_held = snapshot.buttons & CROUCH != 0;
        self.jump_held = snapshot.buttons & JUMP != 0;
        let bumper = f32::from(snapshot.buttons & RIGHT_BUMPER != 0)
            - f32::from(snapshot.buttons & LEFT_BUMPER != 0);
        let stick = radial_dead_zone(snapshot.left_stick);
        let (strafe, turn) = if self.dpad_turns {
            (bumper, snapshot.dpad.x)
        } else {
            (snapshot.dpad.x, bumper)
        };
        self.movement = (stick + Vec2::new(strafe, snapshot.dpad.y)).clamp_length_max(1.0);
        self.turn = (axis_dead_zone(snapshot.right_x) + turn).clamp(-1.0, 1.0);
    }

    pub fn turn_delta(&self, dt: f32) -> f32 {
        self.turn * TURN_SPEED * dt
    }

    pub fn merge(&mut self, mut input: PlayerInput) -> PlayerInput {
        input.movement_axes += self.movement;
        input.jump_pressed |= std::mem::take(&mut self.jump_pending);
        input.crouch_pressed |= self.crouch_held || self.crouch_toggle;
        input.ascend |= self.jump_held;
        input.descend |= self.crouch_held;
        input
    }
}

fn axis_dead_zone(axis: f32) -> f32 {
    if !axis.is_finite() {
        return 0.0;
    }
    axis.signum() * ((axis.abs().min(1.0) - DEAD_ZONE).max(0.0) / (1.0 - DEAD_ZONE))
}

fn radial_dead_zone(stick: Vec2) -> Vec2 {
    let length = stick.length();
    if !length.is_finite() || length <= DEAD_ZONE {
        return Vec2::ZERO;
    }
    stick / length * axis_dead_zone(length)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = sectorGamepadPoll)]
    fn poll_gamepad() -> u32;
    #[wasm_bindgen(js_namespace = window, js_name = sectorGamepadId)]
    fn gamepad_id() -> u32;
    #[wasm_bindgen(js_namespace = window, js_name = sectorGamepadAxis)]
    fn gamepad_axis(index: u32) -> f32;
}

pub fn sample_controller_input(
    mut input: ResMut<ControllerInput>,
    windows: Query<&Window, With<PrimaryWindow>>,
    #[cfg(not(target_arch = "wasm32"))] gamepads: Query<(Entity, &Gamepad)>,
) {
    if windows.iter().any(|window| !window.focused) {
        input.clear();
        input.waiting_for_neutral = true;
        return;
    }
    #[cfg(target_arch = "wasm32")]
    let snapshot = {
        let buttons = poll_gamepad();
        (buttons & 128 != 0).then(|| ControllerSnapshot {
            id: u64::from(gamepad_id()),
            buttons: buttons & 127,
            left_stick: Vec2::new(gamepad_axis(0), gamepad_axis(1)),
            right_x: gamepad_axis(2),
            dpad: Vec2::new(gamepad_axis(3), gamepad_axis(4)),
        })
    };
    #[cfg(not(target_arch = "wasm32"))]
    let snapshot = {
        let mut retained = None;
        let mut candidate = None;
        for (entity, pad) in &gamepads {
            let snapshot = ControllerSnapshot::from_gamepad(entity, pad);
            if input.id == Some(snapshot.id) {
                retained = Some(snapshot);
            }
            if candidate.is_none()
                || snapshot.has_input() && !candidate.is_some_and(ControllerSnapshot::has_input)
            {
                candidate = Some(snapshot);
            }
        }
        retained
            .filter(|snapshot| snapshot.has_input())
            .or(candidate.filter(|snapshot| snapshot.has_input()))
            .or(retained)
            .or(candidate)
    };
    input.sample(snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sector::game::{desired_horizontal_velocity, Player, PLAYER_WALK_SPEED_MPS};

    fn snapshot(buttons: u32, dpad: Vec2) -> ControllerSnapshot {
        ControllerSnapshot {
            id: 1,
            buttons,
            dpad,
            ..default()
        }
    }

    #[test]
    fn stickless_dpad_moves_and_bumpers_turn_then_y_swaps_horizontal_roles() {
        let mut input = ControllerInput::default();
        input.sample(Some(snapshot(RIGHT_BUMPER, Vec2::new(1.0, 1.0))));
        assert!(input.active);
        assert_eq!(input.movement, Vec2::ONE.normalize());
        assert_eq!(input.turn_delta(1.0), TURN_SPEED);
        input.sample(Some(snapshot(
            SWAP_DPAD | RIGHT_BUMPER,
            Vec2::new(-1.0, 1.0),
        )));
        assert!(input.dpad_turns);
        assert_eq!(input.movement, Vec2::ONE.normalize());
        assert_eq!(input.turn_delta(1.0), -TURN_SPEED);
        input.sample(Some(snapshot(SWAP_DPAD, Vec2::ZERO)));
        assert!(input.dpad_turns, "holding Y must not toggle every frame");
        input.sample(Some(snapshot(0, Vec2::ZERO)));
        input.sample(Some(snapshot(SWAP_DPAD, Vec2::ZERO)));
        assert!(!input.dpad_turns);
    }

    #[test]
    fn analog_movement_keeps_partial_speed_and_digital_diagonals_are_capped() {
        let mut input = ControllerInput::default();
        input.sample(Some(ControllerSnapshot {
            id: 1,
            left_stick: Vec2::new(0.0, 0.6),
            ..default()
        }));
        let movement = input.merge(PlayerInput::default());
        let velocity = desired_horizontal_velocity(&Player::default(), movement);
        assert!((velocity.length() - PLAYER_WALK_SPEED_MPS * 0.5).abs() < 0.0001);
        input.sample(Some(snapshot(0, Vec2::ONE)));
        let velocity = desired_horizontal_velocity(
            &Player::default(),
            input.merge(PlayerInput {
                forward: true,
                strafe_right: true,
                ..default()
            }),
        );
        assert!((velocity.length() - PLAYER_WALK_SPEED_MPS).abs() < 0.0001);
    }

    #[test]
    fn sticks_are_unaffected_by_y_and_turning_is_independent_of_frame_rate() {
        let mut input = ControllerInput::default();
        input.sample(Some(ControllerSnapshot {
            id: 1,
            buttons: SWAP_DPAD,
            left_stick: Vec2::X,
            right_x: 1.0,
            ..default()
        }));
        assert_eq!(input.movement, Vec2::X);
        let turn_30: f32 = (0..30).map(|_| input.turn_delta(1.0 / 30.0)).sum();
        let turn_120: f32 = (0..120).map(|_| input.turn_delta(1.0 / 120.0)).sum();
        assert!((turn_30 - turn_120).abs() < 0.0001);
    }

    #[test]
    fn dead_zones_ignore_drift_and_non_finite_values() {
        assert_eq!(radial_dead_zone(Vec2::new(0.1, 0.1)), Vec2::ZERO);
        assert_eq!(radial_dead_zone(Vec2::new(f32::NAN, 0.0)), Vec2::ZERO);
        assert_eq!(axis_dead_zone(f32::INFINITY), 0.0);
        assert_eq!(axis_dead_zone(0.1), 0.0);
        assert_eq!(axis_dead_zone(-1.0), -1.0);
    }

    #[test]
    fn jump_and_automap_edges_wait_for_consumption_without_repeating() {
        let mut input = ControllerInput::default();
        input.sample(Some(snapshot(JUMP | AUTOMAP, Vec2::ZERO)));
        input.sample(Some(snapshot(JUMP | AUTOMAP, Vec2::ZERO)));
        assert!(input.map_pending);
        assert!(input.merge(PlayerInput::default()).jump_pressed);
        assert!(!input.merge(PlayerInput::default()).jump_pressed);
        input.map_pending = false;
        input.sample(Some(snapshot(JUMP | AUTOMAP, Vec2::ZERO)));
        assert!(!input.map_pending);
        input.sample(Some(snapshot(0, Vec2::ZERO)));
        input.sample(Some(snapshot(JUMP, Vec2::ZERO)));
        assert!(input.merge(PlayerInput::default()).jump_pressed);
    }

    #[test]
    fn crouch_hold_and_toggle_clear_on_disconnect_while_layout_stays_selected() {
        let mut input = ControllerInput::default();
        input.sample(Some(snapshot(CROUCH | TOGGLE_CROUCH | SWAP_DPAD, Vec2::X)));
        assert!(input.merge(PlayerInput::default()).crouch_pressed);
        input.sample(Some(snapshot(0, Vec2::ZERO)));
        assert!(input.merge(PlayerInput::default()).crouch_pressed);
        input.sample(None);
        assert!(!input.active);
        assert!(input.dpad_turns);
        assert_eq!(input.merge(PlayerInput::default()), PlayerInput::default());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refocusing_requires_release_before_held_buttons_can_trigger_again() {
        let mut app = App::new();
        app.init_resource::<ControllerInput>()
            .add_systems(Update, sample_controller_input);
        let mut pad = Gamepad::default();
        pad.digital_mut().press(GamepadButton::South);
        let pad_entity = app.world_mut().spawn(pad).id();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.update();
        assert!(app.world().resource::<ControllerInput>().jump_pending);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.update();
        assert!(!app.world().resource::<ControllerInput>().active);
        assert!(!app.world().resource::<ControllerInput>().jump_pending);
        app.world_mut()
            .get_mut::<Gamepad>(pad_entity)
            .unwrap()
            .digital_mut()
            .release_all();
        app.update();
        app.world_mut()
            .get_mut::<Gamepad>(pad_entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::South);
        app.update();
        assert!(app.world().resource::<ControllerInput>().jump_pending);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_gamepad_component_drives_the_shared_input_pipeline() {
        let mut app = App::new();
        app.init_resource::<ControllerInput>()
            .add_systems(Update, sample_controller_input);
        let mut pad = Gamepad::default();
        pad.digital_mut().press(GamepadButton::DPadUp);
        pad.digital_mut().press(GamepadButton::DPadLeft);
        pad.digital_mut().press(GamepadButton::RightTrigger);
        let entity = app.world_mut().spawn(pad).id();
        app.update();
        let input = app.world().resource::<ControllerInput>();
        assert_eq!(input.movement, Vec2::new(-1.0, 1.0).normalize());
        assert_eq!(input.turn, 1.0);
        app.world_mut().despawn(entity);
        app.update();
        assert!(!app.world().resource::<ControllerInput>().active);
    }
}
