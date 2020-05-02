use conrod_core::Colorable;
use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};
use piston_window::texture::UpdateTexture;
use piston_window::*;
use piston_window::{Flip, G2d, G2dTexture, Texture, TextureSettings};
use piston_window::{PistonWindow, UpdateEvent, Window, WindowSettings};

widget_ids! {
    pub struct Ids {
        canvas,
        text,
    }
}

pub struct TestUI {}

impl TestUI {
    pub fn gui(&self, ui: &mut conrod_core::UiCell, ids: &Ids) {
        widget::Text::new("AAAAAAAAAAAA")
            .middle_of(ids.canvas)
            .color(crate::color::WHITE)
            .set(ids.text, ui);
    }
}
