/*
 * File: camera_control_tab.rs
 * Project: ui
 * Created Date: 02/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 02/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use vecmath_utils::*;

use conrod_core::Colorable;
use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};

use crate::color;
use crate::ui::ui_view::{MARGIN, WIN_W};
use crate::ui::UICommand;

use std::sync::mpsc::{Receiver, Sender};

const CONTROL_PAD_SIZE: f64 = 100.0;
const CONTROL_GRIP_SIZE: f64 = 10.0;

widget_ids! {
    pub struct Ids {
        canvas,
        position_title,
        xy_pad,
        xy_grip,
        z_pad,
        z_grip,
        label_move_speed,
        move_speed,
        position_label[],
        position_textbox[],
        sep,
        rotation_title,
        pitch_yaw_pad,
        pitch_yaw_grip,
        roll_pad,
        roll_grip,
        xy_button,
    }
}

pub struct CameraControlTab {
    camera_pos: Vector3,
    camera_move_xy: [f64; 2],
    camera_move_z: f64,
    camera_move_speed: f64,
    camera_rot_pitch_yaw: [f64; 2],
    camera_rot_roll: f64,
    release_mouse_left: bool,
    from_cnt: Receiver<UICommand>,
    to_cnt: Sender<UICommand>,
    ids: Ids,
}

impl CameraControlTab {
    pub fn new(
        from_cnt: Receiver<UICommand>,
        to_cnt: Sender<UICommand>,
        ui: &mut conrod_core::Ui,
    ) -> Self {
        let mut ids = Ids::new(ui.widget_id_generator());
        ids.position_textbox
            .resize(3, &mut ui.widget_id_generator());
        ids.position_label.resize(3, &mut ui.widget_id_generator());

        CameraControlTab {
            camera_pos: [0., 0., 0.],
            camera_move_xy: vec2_zero(),
            camera_move_z: 0.,
            camera_move_speed: 10.,
            camera_rot_pitch_yaw: vec2_zero(),
            camera_rot_roll: 0.,
            release_mouse_left: false,
            from_cnt,
            to_cnt,
            ids,
        }
    }

    pub fn ids(&self) -> &Ids {
        &self.ids
    }

    pub fn gui(&mut self, ui: &mut conrod_core::UiCell) {
        self.camera_rotation_gui(ui);
        self.camera_position_gui(ui);
        if self.release_mouse_left {
            self.release_mouse_left = false;
        }
    }

    pub fn release_mouse_left(&mut self) {
        self.release_mouse_left = true;
    }

    fn camera_position_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;
        widget::Text::new("Position")
            .mid_top_of(ids.canvas)
            .align_left_of(ids.canvas)
            .set(ids.position_title, ui);

        let grip_z = self.camera_move_z * CONTROL_PAD_SIZE;
        widget::Circle::fill(CONTROL_GRIP_SIZE)
            .color(color::GRAY)
            .x_y_relative_to(ids.z_pad, 0., grip_z)
            .set(ids.z_grip, ui);

        let grip_z_range = 1.0;
        let min_z = -grip_z_range / 2.0;
        let max_z = grip_z_range / 2.0;
        for (_, y) in widget::XYPad::new(0., 0., 0., self.camera_move_z, min_z, max_z)
            .color(color::ALPHA)
            .label("\n\nZ")
            .label_color(color::GRAY)
            .line_thickness(0.)
            .value_font_size(0)
            .w_h(1., CONTROL_PAD_SIZE)
            .down_from(ids.position_title, MARGIN)
            .right_from(ids.xy_pad, MARGIN)
            .set(ids.z_pad, ui)
        {
            self.camera_move_z = y;
        }

        let grip_x = self.camera_move_xy[0] * CONTROL_PAD_SIZE;
        let grip_y = self.camera_move_xy[1] * CONTROL_PAD_SIZE;
        widget::Circle::fill(CONTROL_GRIP_SIZE)
            .color(color::GRAY)
            .x_y_relative_to(ids.xy_pad, grip_x, grip_y)
            .set(ids.xy_grip, ui);

        let grip_x_range = 1.0;
        let grip_y_range = 1.0;
        let min_x = -grip_x_range / 2.0;
        let max_x = grip_x_range / 2.0;
        let min_y = -grip_y_range / 2.0;
        let max_y = grip_y_range / 2.0;
        for (x, y) in widget::XYPad::new(
            self.camera_move_xy[0],
            min_x,
            max_x,
            self.camera_move_xy[1],
            min_y,
            max_y,
        )
        .color(color::ALPHA)
        .label("\n\nXY")
        .label_color(color::GRAY)
        .line_thickness(0.)
        .value_font_size(0)
        .w_h(CONTROL_PAD_SIZE, CONTROL_PAD_SIZE)
        .down_from(ids.position_title, MARGIN)
        .set(ids.xy_pad, ui)
        {
            self.camera_move_xy = [x, y];
        }

        widget::Text::new("X: ")
            .h(24.)
            .down_from(ids.position_title, 5.)
            .right_from(ids.z_pad, CONTROL_PAD_SIZE + MARGIN)
            .set(ids.position_label[0], ui);
        widget::Text::new("Y: ")
            .h(24.)
            .down_from(ids.position_label[0], 5.)
            .right_from(ids.z_pad, CONTROL_PAD_SIZE + MARGIN)
            .set(ids.position_label[1], ui);
        widget::Text::new("Z: ")
            .h(24.)
            .down_from(ids.position_label[1], 5.)
            .right_from(ids.z_pad, CONTROL_PAD_SIZE + MARGIN)
            .set(ids.position_label[2], ui);
        for i in 0..3 {
            for txt in widget::TextBox::new(&self.camera_pos[i].to_string())
                .w_h(120., 24.)
                .right_from(ids.position_label[i], 0.)
                .align_middle_y_of(ids.position_label[i])
                .set(ids.position_textbox[i], ui)
            {
                match txt {
                    widget::text_box::Event::Update(s) => {
                        if let Ok(f) = s.parse() {
                            self.camera_pos[i] = f;
                            self.to_cnt
                                .send(UICommand::CameraMoveTo(self.camera_pos))
                                .unwrap()
                        }
                    }
                    _ => (),
                }
            }
        }

        widget::Text::new("Speed: ")
            .h(24.)
            .down_from(ids.xy_pad, MARGIN)
            .align_left_of(ids.canvas)
            .set(ids.label_move_speed, ui);

        for txt in widget::TextBox::new(&self.camera_move_speed.to_string())
            .w_h(80., 24.)
            .align_middle_y_of(ids.label_move_speed)
            .right_from(ids.label_move_speed, 0.)
            .left_justify()
            .set(ids.move_speed, ui)
        {
            match txt {
                widget::text_box::Event::Update(s) => {
                    if let Ok(f) = s.parse() {
                        self.camera_move_speed = f
                    }
                }
                _ => (),
            }
        }

        widget::Rectangle::fill_with([WIN_W as f64 - MARGIN * 2.0, 2.], color::GRAY)
            .align_left_of(ids.canvas)
            .down_from(ids.label_move_speed, MARGIN)
            .set(ids.sep, ui);

        if self.release_mouse_left {
            self.camera_move_xy = vec2_zero();
            self.camera_move_z = 0.;
        }
        if !vec2_is_zero(self.camera_move_xy) || self.camera_move_z != 0.0 {
            let t = vec3_scale(
                to_vec3(self.camera_move_xy, -self.camera_move_z),
                self.camera_move_speed,
            );
            self.to_cnt.send(UICommand::CameraMove(t)).unwrap();
        }

        if let Ok(d) = self.from_cnt.try_recv() {
            match d {
                UICommand::CameraPos(p) => self.camera_pos = p,
                _ => (),
            }
        }
    }

    fn camera_rotation_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;

        widget::Text::new("Rotation")
            .down_from(ids.sep, MARGIN)
            .align_left_of(ids.canvas)
            .set(ids.rotation_title, ui);

        let grip_roll = self.camera_rot_roll * CONTROL_PAD_SIZE;
        widget::Circle::fill(CONTROL_GRIP_SIZE)
            .color(color::GRAY)
            .x_y_relative_to(ids.roll_pad, grip_roll, 0.)
            .set(ids.roll_grip, ui);
        let grip_roll_range = 1.0;
        let min_roll = -grip_roll_range / 2.0;
        let max_roll = grip_roll_range / 2.0;
        for (x, _) in widget::XYPad::new(self.camera_rot_roll, min_roll, max_roll, 0., 0., 0.)
            .color(color::ALPHA)
            .label("\n\nRoll")
            .label_color(color::GRAY)
            .line_thickness(0.)
            .value_font_size(0)
            .w_h(CONTROL_PAD_SIZE, 1.)
            .down_from(ids.pitch_yaw_pad, MARGIN)
            .set(ids.roll_pad, ui)
        {
            self.camera_rot_roll = x;
        }
        let grip_rot_x = self.camera_rot_pitch_yaw[0] * CONTROL_PAD_SIZE;
        let grip_rot_y = self.camera_rot_pitch_yaw[1] * CONTROL_PAD_SIZE;
        widget::Circle::fill(CONTROL_GRIP_SIZE)
            .color(color::WHITE)
            .x_y_relative_to(ids.pitch_yaw_pad, grip_rot_x, grip_rot_y)
            .set(ids.pitch_yaw_grip, ui);
        let ball_x_range = 1.0;
        let ball_y_range = 1.0;
        let min_x = -ball_x_range / 2.0;
        let max_x = ball_x_range / 2.0;
        let min_y = -ball_y_range / 2.0;
        let max_y = ball_y_range / 2.0;
        for (x, y) in widget::XYPad::new(
            self.camera_rot_pitch_yaw[0],
            min_x,
            max_x,
            self.camera_rot_pitch_yaw[1],
            min_y,
            max_y,
        )
        .color(color::ALPHA)
        .label("\n\nPitch-Yaw")
        .label_color(color::GRAY)
        .line_thickness(0.)
        .value_font_size(0)
        .w_h(CONTROL_PAD_SIZE, CONTROL_PAD_SIZE)
        .down_from(ids.rotation_title, MARGIN)
        .set(ids.pitch_yaw_pad, ui)
        {
            self.camera_rot_pitch_yaw = [x, y];
        }
        if self.release_mouse_left {
            self.camera_rot_pitch_yaw = vec2_zero();
            self.camera_rot_roll = 0.;
        }
        if !vec2_is_zero(self.camera_rot_pitch_yaw) || self.camera_rot_roll != 0.0 {
            self.to_cnt
                .send(UICommand::CameraRotate([
                    self.camera_rot_pitch_yaw[1],
                    self.camera_rot_roll,
                    -self.camera_rot_pitch_yaw[0],
                ]))
                .unwrap();
        }
        for _ in widget::Button::new()
            .label("zx")
            .down_from(ids.roll_pad, MARGIN)
            .w_h(80.0, 60.)
            .set(ids.xy_button, ui)
        {
            self.to_cnt
                .send(UICommand::CameraSetPosture([0., -1., 0.], [0., 0., 1.]))
                .unwrap();
        }
    }
}
