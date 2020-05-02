/*
 * File: ui_view.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 02/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::vec_utils::Vector3;
use conrod_core::Colorable;
use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};
use piston_window::texture::UpdateTexture;
use piston_window::*;
use piston_window::{Flip, G2d, G2dTexture, Texture, TextureSettings};
use piston_window::{PistonWindow, UpdateEvent, Window, WindowSettings};

use std::sync::mpsc;

use crate::color;
use crate::custom::TestUI;
use crate::ui_command::UICommand;

pub const WIN_W: u32 = 600;
pub const WIN_H: u32 = 800;
const MARGIN: conrod_core::Scalar = 30.0;

pub struct DemoApp {
    camera_move_xy: [f64; 2],
    camera_move_z: f64,
    camera_rot_pitch_yaw: [f64; 2],
    camera_rot_roll: f64,
    release_mouse_left: bool,
    tx_command: mpsc::Sender<UICommand>,
    test: TestUI,
}

impl DemoApp {
    pub fn new(tx_command: mpsc::Sender<UICommand>) -> Self {
        DemoApp {
            camera_move_xy: [0., 0.],
            camera_move_z: 0.,
            camera_rot_pitch_yaw: [0., 0.],
            camera_rot_roll: 0.,
            release_mouse_left: false,
            tx_command,
            test: TestUI {},
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
        canvas_main,
        tabs,
        canvas_camera,
        canvas_slice,
        canvas_config,
        camera_position_title,
        camera_xy_pad,
        camera_xy_grip,
        camera_z_pad,
        camera_z_grip,
        camera_position_label[],
        camera_position_textbox[],
        camera_sep,
        camera_rotation_title,
        camera_pitch_yaw_pad,
        camera_pitch_yaw_grip,
        camera_roll_pad,
        camera_roll_grip,
        camera_xy_button,
    }
}

pub fn camera_position_gui(ui: &mut conrod_core::UiCell, ids: &Ids, app: &mut DemoApp) {
    widget::Text::new("Position")
        .mid_top_of(ids.canvas_camera)
        .align_left_of(ids.canvas_camera)
        .set(ids.camera_position_title, ui);

    let grip_z = app.camera_move_z * 100.0;
    widget::Circle::fill(10.0)
        .color(color::WHITE)
        .x_y_relative_to(ids.camera_z_pad, 0., grip_z)
        .set(ids.camera_z_grip, ui);

    let grip_z_range = 1.0;
    let min_z = -grip_z_range / 2.0;
    let max_z = grip_z_range / 2.0;
    for (_, y) in widget::XYPad::new(0., 0., 0., app.camera_move_z, min_z, max_z)
        .color(conrod_core::color::rgba(1., 1., 1., 0.))
        .label("\n\nZ")
        .label_color(conrod_core::color::rgba(1., 1.0, 1.0, 0.2))
        .line_thickness(0.)
        .value_font_size(0)
        .w_h(1., 100.)
        .down_from(ids.camera_position_title, MARGIN)
        .right_from(ids.camera_xy_pad, 50. + 20.)
        .set(ids.camera_z_pad, ui)
    {
        app.camera_move_z = y;
    }

    let ball_x = app.camera_move_xy[0] * 100.0;
    let ball_y = app.camera_move_xy[1] * 100.0;
    widget::Circle::fill(10.0)
        .color(color::WHITE)
        .x_y_relative_to(ids.camera_xy_pad, ball_x, ball_y)
        .set(ids.camera_xy_grip, ui);

    let ball_x_range = 1.0;
    let ball_y_range = 1.0;
    let min_x = -ball_x_range / 2.0;
    let max_x = ball_x_range / 2.0;
    let min_y = -ball_y_range / 2.0;
    let max_y = ball_y_range / 2.0;
    for (x, y) in widget::XYPad::new(
        app.camera_move_xy[0],
        min_x,
        max_x,
        app.camera_move_xy[1],
        min_y,
        max_y,
    )
    .color(conrod_core::color::rgba(1., 1.0, 1.0, 0.))
    .label("\n\nXY")
    .label_color(conrod_core::color::rgba(1., 1.0, 1.0, 0.2))
    .line_thickness(0.)
    .value_font_size(0)
    .w_h(100., 100.)
    .down_from(ids.camera_position_title, MARGIN)
    .set(ids.camera_xy_pad, ui)
    {
        app.camera_move_xy = [x, y];
    }

    if app.release_mouse_left {
        app.camera_move_xy = [0., 0.];
        app.camera_move_z = 0.;
    }
    if app.camera_move_xy[0] != 0.0 || app.camera_move_xy[1] != 0.0 || app.camera_move_z != 0.0 {
        app.tx_command
            .send(UICommand::CameraMove([
                app.camera_move_xy[0] as f32,
                app.camera_move_xy[1] as f32,
                -app.camera_move_z as f32,
            ]))
            .unwrap();
    }

    widget::Rectangle::fill_with([WIN_W as f64 - MARGIN * 2.0, 2.], color::WHITE)
        .align_left_of(ids.canvas_camera)
        .down_from(ids.camera_xy_pad, MARGIN)
        .set(ids.camera_sep, ui);

    for a in widget::TextBox::new("0.000")
        .w_h(80., 60.)
        .right_from(ids.camera_z_pad, MARGIN)
        .down_from(ids.camera_position_title, MARGIN)
        .set(ids.camera_position_textbox[0], ui)
    {
        match a {
            widget::text_box::Event::Update(s) => println!("{}", s),
            Enter => println!("ENTER"),
        }
    }
}

pub fn camera_rotation_gui(ui: &mut conrod_core::UiCell, ids: &Ids, app: &mut DemoApp) {
    widget::Text::new("Rotation")
        .down_from(ids.camera_sep, MARGIN)
        .align_left_of(ids.canvas_camera)
        .set(ids.camera_rotation_title, ui);

    let grip_roll = app.camera_rot_roll * 100.0;
    widget::Circle::fill(10.0)
        .color(color::WHITE)
        .x_y_relative_to(ids.camera_roll_pad, grip_roll, 0.)
        .set(ids.camera_roll_grip, ui);

    let grip_roll_range = 1.0;
    let min_roll = -grip_roll_range / 2.0;
    let max_roll = grip_roll_range / 2.0;
    for (x, _) in widget::XYPad::new(app.camera_rot_roll, min_roll, max_roll, 0., 0., 0.)
        .color(conrod_core::color::rgba(1., 1., 1., 0.))
        .label("\n\nRoll")
        .label_color(conrod_core::color::rgba(1., 1.0, 1.0, 0.2))
        .line_thickness(0.)
        .value_font_size(0)
        .w_h(100., 1.)
        .down_from(ids.camera_pitch_yaw_pad, MARGIN)
        .set(ids.camera_roll_pad, ui)
    {
        app.camera_rot_roll = x;
    }

    let grip_rot_x = app.camera_rot_pitch_yaw[0] * 100.0;
    let grip_rot_y = app.camera_rot_pitch_yaw[1] * 100.0;
    widget::Circle::fill(10.0)
        .color(color::WHITE)
        .x_y_relative_to(ids.camera_pitch_yaw_pad, grip_rot_x, grip_rot_y)
        .set(ids.camera_pitch_yaw_grip, ui);

    let ball_x_range = 1.0;
    let ball_y_range = 1.0;
    let min_x = -ball_x_range / 2.0;
    let max_x = ball_x_range / 2.0;
    let min_y = -ball_y_range / 2.0;
    let max_y = ball_y_range / 2.0;
    for (x, y) in widget::XYPad::new(
        app.camera_rot_pitch_yaw[0],
        min_x,
        max_x,
        app.camera_rot_pitch_yaw[1],
        min_y,
        max_y,
    )
    .color(conrod_core::color::rgba(1., 1.0, 1.0, 0.))
    .label("\n\nPitch-Yaw")
    .label_color(conrod_core::color::rgba(1., 1.0, 1.0, 0.2))
    .line_thickness(0.)
    .value_font_size(0)
    .w_h(100., 100.)
    .down_from(ids.camera_rotation_title, 20.)
    .set(ids.camera_pitch_yaw_pad, ui)
    {
        app.camera_rot_pitch_yaw = [x, y];
    }

    if app.release_mouse_left {
        app.camera_rot_pitch_yaw = [0., 0.];
        app.camera_rot_roll = 0.;
    }
    if app.camera_rot_pitch_yaw[0] != 0.0
        || app.camera_rot_pitch_yaw[1] != 0.0
        || app.camera_rot_roll != 0.0
    {
        app.tx_command
            .send(UICommand::CameraRotate([
                app.camera_rot_pitch_yaw[1] as f32,
                app.camera_rot_roll as f32,
                -app.camera_rot_pitch_yaw[0] as f32,
            ]))
            .unwrap();
    }

    for _ in widget::Button::new()
        .label("zx")
        .down_from(ids.camera_roll_pad, MARGIN)
        .w_h(80.0, 60.)
        .set(ids.camera_xy_button, ui)
    {
        app.tx_command
            .send(UICommand::CameraSetPosture([0., -1., 0.], [0., 0., 1.]))
            .unwrap();
    }
}

pub fn gui(
    ui: &mut conrod_core::UiCell,
    ids: &Ids,
    ids_sub: &crate::custom::Ids,
    app: &mut DemoApp,
) {
    widget::Canvas::new()
        .scroll_kids_vertically()
        .color(color::BLACK)
        .set(ids.canvas_main, ui);

    widget::tabs::Tabs::new(&[
        (ids.canvas_camera, "Camera"),
        (ids.canvas_slice, "Slice"),
        (ids_sub.canvas, "SUB"),
    ])
    .layout_horizontally()
    .wh_of(ids.canvas_main)
    .color(color::BLACK)
    .middle_of(ids.canvas_main)
    .pad(20.0)
    .set(ids.tabs, ui);

    // for _press in widget::Button::new()
    //     .label("PRESS ME")
    //     .mid_top_of(ids.canvas)
    //     .w_h(80., 60.)
    //     .set(ids.button, ui)
    // {
    //     app.tx_command
    //         .send(UICommand::CameraRotate([0., 0., 0.]))
    //         .unwrap();
    // }

    app.test.gui(ui, ids_sub);
    camera_rotation_gui(ui, ids, app);
    camera_position_gui(ui, ids, app);

    if app.release_mouse_left {
        app.release_mouse_left = false;
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

    let mut ids = Ids::new(ui.widget_id_generator());
    let ids_sub = crate::custom::Ids::new(ui.widget_id_generator());
    ids.camera_position_textbox
        .resize(3, &mut ui.widget_id_generator());

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
            gui(&mut ui, &ids, &ids_sub, &mut app);
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
