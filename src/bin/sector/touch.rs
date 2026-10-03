use bevy::prelude::*;
use sector::game::PlayerInput;

/// Touch jump edges survive render frames until one fixed simulation tick consumes them.
#[derive(Resource, Default)]
pub struct TouchInput {
    pub active: bool,
    buttons: u32,
    pub look_delta: f32,
    jump_pending: bool,
    pub map_pending: bool,
}

impl TouchInput {
    pub fn sample(&mut self, buttons: u32, look_delta: f32) {
        self.active = buttons & 64 != 0;
        self.buttons = buttons;
        self.look_delta = look_delta;
        self.map_pending = self.active && (self.map_pending || buttons & 128 != 0);
        self.jump_pending = self.active && (self.jump_pending || buttons & 32 != 0);
    }

    pub fn merge(&mut self, mut input: PlayerInput) -> PlayerInput {
        input.forward |= self.buttons & 1 != 0;
        input.backward |= self.buttons & 2 != 0;
        input.strafe_left |= self.buttons & 4 != 0;
        input.strafe_right |= self.buttons & 8 != 0;
        input.crouch_pressed |= self.buttons & 16 != 0;
        input.jump_pressed |= std::mem::take(&mut self.jump_pending);
        input
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = sectorTouchButtons)]
    fn touch_buttons() -> u32;
    #[wasm_bindgen(js_namespace = window, js_name = sectorTouchLook)]
    fn touch_look() -> f32;
}

pub fn sample_touch_input(mut touch: ResMut<TouchInput>) {
    #[cfg(target_arch = "wasm32")]
    touch.sample(touch_buttons(), touch_look());
    #[cfg(not(target_arch = "wasm32"))]
    touch.sample(0, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_jump_survives_frames_and_fires_once_across_fixed_ticks() {
        let mut touch = TouchInput::default();
        touch.sample(64 | 32, 0.0);
        touch.sample(64, 0.0);
        assert!(touch.merge(PlayerInput::default()).jump_pressed);
        assert!(!touch.merge(PlayerInput::default()).jump_pressed);
    }

    #[test]
    fn touch_combines_with_keyboard_and_clears_on_focus_loss() {
        let mut touch = TouchInput::default();
        touch.sample(64 | 1 | 8 | 16 | 32, 4.0);
        let input = touch.merge(PlayerInput {
            strafe_left: true,
            ..default()
        });
        assert!(input.forward && input.strafe_left && input.strafe_right && input.crouch_pressed);
        touch.sample(0, 0.0);
        assert!(!touch.active);
        let input = touch.merge(PlayerInput::default());
        assert!(!input.forward && !input.crouch_pressed && !input.jump_pressed);
    }
}
