use bevy::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

#[derive(Resource)]
pub struct WebCanvasRenderer {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    frame: Vec<u8>,
    width: u32,
    height: u32,
}

impl WebCanvasRenderer {
    fn new() -> Option<Self> {
        let window = web_sys::window()?;
        let document = window.document()?;
        let canvas = document
            .query_selector("canvas")
            .ok()??
            .dyn_into::<HtmlCanvasElement>()
            .ok()?;
        let context = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<CanvasRenderingContext2d>()
            .ok()?;
        context.set_image_smoothing_enabled(false);
        Some(Self {
            canvas,
            context,
            frame: Vec::new(),
            width: 0,
            height: 0,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.frame.resize(width as usize * height as usize * 4, 0);
        self.context.set_image_smoothing_enabled(false);
    }

    pub fn frame_mut(&mut self) -> &mut [u8] {
        &mut self.frame
    }

    pub fn present(&self) -> Result<(), String> {
        let image = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(self.frame.as_slice()),
            self.width,
            self.height,
        )
        .map_err(|error| format!("failed to create image data: {error:?}"))?;

        self.context
            .put_image_data(&image, 0.0, 0.0)
            .map_err(|error| format!("failed to draw image data: {error:?}"))
    }
}

pub fn ensure_web_canvas_system(mut commands: Commands, renderer: Option<Res<WebCanvasRenderer>>) {
    if renderer.is_some() {
        return;
    }

    let Some(renderer) = WebCanvasRenderer::new() else {
        return;
    };
    commands.insert_resource(renderer);
}
