/*
 * File: ui_view.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::text::GlyphCache;
use conrod_core::{widget, Positionable, Sizeable, Widget};
use conrod_core::{Colorable, Ui};
use piston_window::texture::UpdateTexture;
use piston_window::*;
use piston_window::{G2d, G2dTexture, TextureSettings};
use piston_window::{PistonWindow, UpdateEvent, Window, WindowSettings};

use std::sync::mpsc::{Receiver, Sender};

use super::control_tab::ControlTab;
use crate::color;
use crate::ui::UICommand;

pub const WIN_W: u32 = 800;
pub const WIN_H: u32 = 800;
pub const MARGIN: conrod_core::Scalar = 30.0;

widget_ids! {
    pub struct Ids {
        canvas,
        tabs,
    }
}

pub struct App {
    pub camera_tab: ControlTab,
    pub from_cnt: Receiver<UICommand>,
    pub release_mouse_left: bool,
}

impl App {
    pub fn new(camera_tab: ControlTab, from_cnt: Receiver<UICommand>) -> Self {
        App {
            camera_tab,
            from_cnt,
            release_mouse_left: false,
        }
    }
}

pub fn theme() -> conrod_core::Theme {
    use conrod_core::position::{Align, Direction, Padding, Position, Relative};
    conrod_core::Theme {
        name: "Demo Theme".to_string(),
        padding: Padding::none(),
        x_position: Position::Relative(Relative::Align(Align::Start), None),
        y_position: Position::Relative(Relative::Direction(Direction::Backwards, 20.0), None),
        background_color: color::DARK,
        shape_color: conrod_core::color::LIGHT_CHARCOAL,
        border_color: color::DARK,
        border_width: 0.0,
        label_color: conrod_core::color::WHITE,
        font_id: None,
        font_size_large: 26,
        font_size_medium: 18,
        font_size_small: 12,
        widget_styling: conrod_core::theme::StyleMap::default(),
        mouse_drag_threshold: 0.0,
        double_click_threshold: std::time::Duration::from_millis(500),
    }
}

pub fn gui(ui: &mut conrod_core::UiCell, ids: &Ids, app: &mut App) {
    widget::Canvas::new()
        .scroll_kids_vertically()
        .color(color::DARK)
        .set(ids.canvas, ui);

    widget::tabs::Tabs::new(&[(app.camera_tab.ids().canvas, "Controller")])
        .layout_horizontally()
        .wh_of(ids.canvas)
        .color(color::DARK)
        .middle_of(ids.canvas)
        .pad(MARGIN)
        .set(ids.tabs, ui);

    app.camera_tab.gui(ui);

    if app.release_mouse_left {
        app.camera_tab.release_mouse_left();
        app.release_mouse_left = false;
    }
}

pub struct UiView<'a> {
    app: App,
    ui: Ui,
    ids: Ids,
    text_texture_cache: G2dTexture,
    glyph_cache: GlyphCache<'a>,
    image_map: conrod_core::image::Map<G2dTexture>,
}

impl<'a> UiView<'a> {
    pub fn new(from_cnt: Receiver<UICommand>, to_cnt: Sender<UICommand>) -> (Self, PistonWindow) {
        const WIDTH: u32 = WIN_W;
        const HEIGHT: u32 = WIN_H;

        let mut window: PistonWindow = WindowSettings::new("AUTD3 emulator", [800, 800])
            .graphics_api(OpenGL::V4_5)
            .samples(4)
            .exit_on_esc(true)
            .vsync(true)
            .build()
            .unwrap();

        let mut ui = conrod_core::UiBuilder::new([WIDTH as f64, HEIGHT as f64])
            .theme(theme())
            .build();

        let assets = find_folder::Search::KidsThenParents(3, 5)
            .for_folder("assets")
            .unwrap();
        let font_path = assets.join("fonts").join("NotoSans-Regular.ttf");
        ui.fonts.insert_from_file(font_path).unwrap();

        let mut texture_context = window.create_texture_context();

        let (glyph_cache, text_texture_cache) = {
            const SCALE_TOLERANCE: f32 = 0.1;
            const POSITION_TOLERANCE: f32 = 0.1;
            let cache = conrod_core::text::GlyphCache::builder()
                .dimensions(WIDTH, HEIGHT)
                .scale_tolerance(SCALE_TOLERANCE)
                .position_tolerance(POSITION_TOLERANCE)
                .build();
            let buffer_len = WIDTH as usize * HEIGHT as usize;
            let init = vec![128; buffer_len];
            let settings = TextureSettings::new();
            let texture = G2dTexture::from_memory_alpha(
                &mut texture_context,
                &init,
                WIDTH,
                HEIGHT,
                &settings,
            )
            .unwrap();
            (cache, texture)
        };

        let ids = Ids::new(ui.widget_id_generator());

        let image_map = conrod_core::image::Map::new();
        let camera_tab = ControlTab::new(to_cnt, &mut ui);
        let app = App::new(camera_tab, from_cnt);

        (
            Self {
                app,
                ui,
                ids,
                text_texture_cache,
                glyph_cache,
                image_map,
            },
            window,
        )
    }

    pub fn renderer(&mut self, window: &mut PistonWindow, event: Event) {
        if let Some(Button::Mouse(button)) = event.release_args() {
            if button == MouseButton::Left {
                self.app.release_mouse_left = true;
            }
        }

        let size = window.size();
        let (win_w, win_h) = (
            size.width as conrod_core::Scalar,
            size.height as conrod_core::Scalar,
        );
        if let Some(e) = conrod_piston::event::convert(event.clone(), win_w, win_h) {
            self.ui.handle_event(e);
        }

        event.update(|_| {
            let mut ui = self.ui.set_widgets();
            gui(&mut ui, &self.ids, &mut self.app);
            if let Ok(d) = self.app.from_cnt.try_recv() {
                match d {
                    UICommand::CameraPos(p) => self.app.camera_tab.camera_state.set_position(p),
                    UICommand::SlicePos(p) => self.app.camera_tab.slice_state.set_position(p),
                    UICommand::CameraSetPosture { right, up } => {
                        self.app.camera_tab.camera_state.set_posture(right, up)
                    }
                    UICommand::SliceSetPosture { right, up } => {
                        self.app.camera_tab.slice_state.set_posture(right, up)
                    }
                    _ => (),
                }
            }
        });

        let mut texture_context = window.create_texture_context();
        window.draw_2d(&event, |context, graphics, device| {
            if let Some(primitives) = self.ui.draw_if_changed() {
                let cache_queued_glyphs = |_: &mut G2d,
                                           cache: &mut G2dTexture,
                                           rect: conrod_core::text::rt::Rect<u32>,
                                           data: &[u8]| {
                    let offset = [rect.min.x, rect.min.y];
                    let size = [rect.width(), rect.height()];
                    let format = piston_window::texture::Format::Rgba8;

                    let text_vertex_data: Vec<_> =
                        data.iter().flat_map(|&b| vec![255, 255, 255, b]).collect();
                    UpdateTexture::update(
                        cache,
                        &mut texture_context,
                        format,
                        &text_vertex_data[..],
                        offset,
                        size,
                    )
                    .expect("failed to update texture")
                };

                fn texture_from_image<T>(img: &T) -> &T {
                    img
                }

                conrod_piston::draw::primitives(
                    primitives,
                    context,
                    graphics,
                    &mut self.text_texture_cache,
                    &mut self.glyph_cache,
                    &self.image_map,
                    cache_queued_glyphs,
                    texture_from_image,
                );

                texture_context.encoder.flush(device);
            }
        });
    }
}
