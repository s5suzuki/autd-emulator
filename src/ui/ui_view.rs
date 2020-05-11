/*
 * File: ui_view.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 11/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::Colorable;
use conrod_core::{widget, Positionable, Sizeable, Widget};
use piston_window::texture::UpdateTexture;
use piston_window::*;
use piston_window::{G2d, G2dTexture, TextureSettings};
use piston_window::{PistonWindow, UpdateEvent, Window, WindowSettings};

use std::sync::mpsc::{Receiver, Sender};

use super::camera_control_tab::CameraControlTab;
use super::slice_control_tab::SliceControlTab;
use crate::color;
use crate::ui::UICommand;

pub const WIN_W: u32 = 600;
pub const WIN_H: u32 = 800;
pub const MARGIN: conrod_core::Scalar = 30.0;

widget_ids! {
    pub struct Ids {
        canvas,
        tabs,
    }
}

pub struct App {
    pub camera_tab: CameraControlTab,
    pub slice_tab: SliceControlTab,
    pub from_cnt: Receiver<UICommand>,
    pub release_mouse_left: bool,
}

impl App {
    pub fn new(
        camera_tab: CameraControlTab,
        slice_tab: SliceControlTab,
        from_cnt: Receiver<UICommand>,
    ) -> Self {
        App {
            camera_tab,
            slice_tab,
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

    widget::tabs::Tabs::new(&[
        (app.camera_tab.ids().canvas, "Camera"),
        (app.slice_tab.ids().canvas, "Slice"),
    ])
    .layout_horizontally()
    .wh_of(ids.canvas)
    .color(color::DARK)
    .middle_of(ids.canvas)
    .pad(MARGIN)
    .set(ids.tabs, ui);

    app.camera_tab.gui(ui);
    app.slice_tab.gui(ui);

    if app.release_mouse_left {
        app.camera_tab.release_mouse_left();
        app.slice_tab.release_mouse_left();
        app.release_mouse_left = false;
    }
}

pub fn window_2d(from_cnt: Receiver<UICommand>, to_cnt: Sender<UICommand>) {
    const WIDTH: u32 = WIN_W;
    const HEIGHT: u32 = WIN_H;

    let mut window: PistonWindow =
        WindowSettings::new("All Widgets - Piston Backend", [WIDTH, HEIGHT])
            .opengl(OpenGL::V3_2)
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
    let font_path = assets.join("fonts/NotoSans-Regular.ttf");
    ui.fonts.insert_from_file(font_path).unwrap();

    let mut text_vertex_data = Vec::new();
    let (mut glyph_cache, mut text_texture_cache) = {
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
        let factory = &mut window.factory;
        let texture =
            G2dTexture::from_memory_alpha(factory, &init, WIDTH, HEIGHT, &settings).unwrap();
        (cache, texture)
    };

    let ids = Ids::new(ui.widget_id_generator());

    let image_map = conrod_core::image::Map::new();
    let camera_tab = CameraControlTab::new(to_cnt.clone(), &mut ui);
    let slice_tab = SliceControlTab::new(to_cnt.clone(), &mut ui);
    let mut app = App::new(camera_tab, slice_tab, from_cnt);

    while let Some(event) = window.next() {
        if let Some(Button::Mouse(button)) = event.release_args() {
            match button {
                MouseButton::Left => app.release_mouse_left = true,
                _ => (),
            }
        }

        let size = window.size();
        let (win_w, win_h) = (
            size.width as conrod_core::Scalar,
            size.height as conrod_core::Scalar,
        );
        if let Some(e) = conrod_piston::event::convert(event.clone(), win_w, win_h) {
            ui.handle_event(e);
        }

        event.update(|_| {
            let mut ui = ui.set_widgets();
            gui(&mut ui, &ids, &mut app);
            if let Ok(d) = app.from_cnt.try_recv() {
                match d {
                    UICommand::CameraPos(p) => app.camera_tab.set_camera_pos(p),
                    UICommand::SlicePos(p) => app.slice_tab.set_pos(p),
                    _ => (),
                }
            }
        });

        window.draw_2d(&event, |context, graphics| {
            if let Some(primitives) = ui.draw_if_changed() {
                let cache_queued_glyphs = |graphics: &mut G2d,
                                           cache: &mut G2dTexture,
                                           rect: conrod_core::text::rt::Rect<u32>,
                                           data: &[u8]| {
                    let offset = [rect.min.x, rect.min.y];
                    let size = [rect.width(), rect.height()];
                    let format = piston_window::texture::Format::Rgba8;
                    let encoder = &mut graphics.encoder;
                    text_vertex_data.clear();
                    text_vertex_data.extend(data.iter().flat_map(|&b| vec![255, 255, 255, b]));
                    UpdateTexture::update(
                        cache,
                        encoder,
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
                    &mut text_texture_cache,
                    &mut glyph_cache,
                    &image_map,
                    cache_queued_glyphs,
                    texture_from_image,
                );
            }
        });
    }
}
