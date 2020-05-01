/*
 * File: ui_view.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::Colorable;
use piston_window::texture::UpdateTexture;
use piston_window::*;
use piston_window::{Flip, G2d, G2dTexture, Texture, TextureSettings};
use piston_window::{PistonWindow, UpdateEvent, Window, WindowSettings};

use std::sync::mpsc;

use crate::ui_command::UICommand;

pub const WIN_W: u32 = 600;
pub const WIN_H: u32 = 420;

pub struct DemoApp {
    caemra_xy: conrod_core::Point,
    release_mouse_left: bool,
    tx_command: mpsc::Sender<UICommand>,
}

impl DemoApp {
    pub fn new(tx_command: mpsc::Sender<UICommand>) -> Self {
        DemoApp {
            caemra_xy: [0., 0.],
            release_mouse_left: false,
            tx_command,
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
        background_color: conrod_core::color::DARK_CHARCOAL,
        shape_color: conrod_core::color::LIGHT_CHARCOAL,
        border_color: conrod_core::color::BLACK,
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

widget_ids! {
    pub struct Ids {
        canvas,
        title,
        button,
        xy_pad,
        z_pad,
        ball,
    }
}

pub fn gui(ui: &mut conrod_core::UiCell, ids: &Ids, app: &mut DemoApp) {
    use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};
    const MARGIN: conrod_core::Scalar = 30.0;

    widget::Canvas::new()
        .pad(MARGIN)
        .scroll_kids_vertically()
        .set(ids.canvas, ui);

    for _press in widget::Button::new()
        .label("PRESS ME")
        .mid_top_of(ids.canvas)
        .w_h(80., 60.)
        .set(ids.button, ui)
    {
        app.tx_command
            .send(UICommand::CameraRotate([0., 0., 0.]))
            .unwrap();
    }

    let ball_x = app.caemra_xy[0] * 100.0;
    let ball_y = app.caemra_xy[1] * 100.0;
    widget::Circle::fill(10.0)
        .color(conrod_core::color::rgba(1., 1., 1., 1.))
        .x_y_relative_to(ids.xy_pad, ball_x, ball_y)
        .set(ids.ball, ui);

    let ball_x_range = 1.0;
    let ball_y_range = 1.0;
    let min_x = -ball_x_range / 2.0;
    let max_x = ball_x_range / 2.0;
    let min_y = -ball_y_range / 2.0;
    let max_y = ball_y_range / 2.0;
    for (x, y) in widget::XYPad::new(
        app.caemra_xy[0],
        min_x,
        max_x,
        app.caemra_xy[1],
        min_y,
        max_y,
    )
    .color(conrod_core::color::rgba(1., 1.0, 1.0, 0.))
    .label("\n\nCamera XY")
    .label_color(conrod_core::color::rgba(1., 1.0, 1.0, 0.2))
    .line_thickness(0.)
    .value_font_size(0)
    .w_h(100., 100.)
    .down_from(ids.button, 60.)
    .align_middle_x_of(ids.canvas)
    .parent(ids.canvas)
    .set(ids.xy_pad, ui)
    {
        app.caemra_xy = [x, y];
    }
    if app.release_mouse_left {
        app.caemra_xy = [0., 0.];
        app.release_mouse_left = false;
    }

    if app.caemra_xy[0] != 0.0 || app.caemra_xy[1] != 0.0 {
        app.tx_command
            .send(UICommand::CameraMove([
                app.caemra_xy[0] as f32,
                app.caemra_xy[1] as f32,
                0.0,
            ]))
            .unwrap();
    }
}

pub fn window_2d(tx_command: mpsc::Sender<UICommand>) {
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

    // Create our `conrod_core::image::Map` which describes each of our widget->image mappings.
    let image_map = conrod_core::image::Map::new();
    // A demonstration of some state that we'd like to control with the App.
    let mut app = DemoApp::new(tx_command);

    // Poll events from the window.
    while let Some(event) = window.next() {
        if let Some(Button::Mouse(button)) = event.release_args() {
            match button {
                MouseButton::Left => app.release_mouse_left = true,
                _ => (),
            }
        }

        // Convert the src event to a conrod event.
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
