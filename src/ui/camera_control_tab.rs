/*
 * File: camera_control_tab.rs
 * Project: ui
 * Created Date: 02/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 12/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::widget::text_box::Event;
use conrod_core::Colorable;
use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};
use vecmath_utils::Vector2;
use vecmath_utils::{mat4, vec2, vec3};

use crate::color;
use crate::ui::ui_view::{MARGIN, WIN_W};
use crate::ui::UICommand;
use crate::{Matrix4, Vector3};

use std::sync::mpsc::Sender;

const CONTROL_PAD_SIZE: f64 = 100.0;
const CONTROL_GRIP_SIZE: f64 = 10.0;

widget_ids! {
    pub struct Ids {
        canvas,
        sep[],
        camera_toggle,
        slice_toggle,

        position_title,
        xy_pad,
        xy_grip,
        z_pad,
        z_grip,
        label_move_speed,
        move_speed,
        position_label[],
        position_textbox[],
        update_position_button,
        auto_view_button,

        rotation_title,
        pitch_yaw_pad,
        pitch_yaw_grip,
        roll_pad,
        roll_grip,
        posture_label[],
        posture_xyz_label[],
        posture_textbox[],
        xy_button,
        yz_button,
        xz_button,
    }
}

pub struct CameraState {
    pub(crate) pos: Vector3,
    pub(crate) pos_txt: [String; 3],
    pub(crate) right: Vector3,
    pub(crate) up: Vector3,
    pub(crate) right_txt: [String; 3],
    pub(crate) up_txt: [String; 3],
}

impl CameraState {
    pub fn new() -> Self {
        Self {
            pos: vec3::zero(),
            pos_txt: ["".to_string(), "".to_string(), "".to_string()],
            right: vec3::zero(),
            up: vec3::zero(),
            right_txt: ["".to_string(), "".to_string(), "".to_string()],
            up_txt: ["".to_string(), "".to_string(), "".to_string()],
        }
    }

    pub fn set_position(&mut self, pos: Vector3) {
        self.pos = pos;
        self.pos_txt = vec3::map(pos, |v| format!("{:.3}", v));
    }

    pub fn set_posture(&mut self, right: Vector3, up: Vector3) {
        self.right = vec3::normalized(right);
        self.up = vec3::normalized(up);
        self.right_txt = vec3::map(self.right, |v| format!("{:.3}", v));
        self.up_txt = vec3::map(self.up, |v| format!("{:.3}", v));
    }

    pub fn forward(&self) -> Vector3 {
        vec3::cross(self.right, self.up)
    }

    pub fn orthogonal(&self) -> Matrix4 {
        let p = self.pos;
        let r = self.right;
        let u = self.up;
        let f = self.forward();
        [
            [r[0], u[0], f[0], 0.],
            [r[1], u[1], f[1], 0.],
            [r[2], u[2], f[2], 0.],
            [-vec3::dot(r, p), -vec3::dot(u, p), -vec3::dot(f, p), 1.],
        ]
    }
}

pub struct SliceState {
    pub(crate) pos: Vector3,
    pub(crate) pos_txt: [String; 3],
    pub(crate) right: Vector3,
    pub(crate) up: Vector3,
    pub(crate) right_txt: [String; 3],
    pub(crate) up_txt: [String; 3],
}

impl SliceState {
    pub fn new() -> Self {
        Self {
            pos: vec3::zero(),
            pos_txt: ["".to_string(), "".to_string(), "".to_string()],
            right: vec3::zero(),
            up: vec3::zero(),
            right_txt: ["".to_string(), "".to_string(), "".to_string()],
            up_txt: ["".to_string(), "".to_string(), "".to_string()],
        }
    }

    pub fn set_position(&mut self, pos: Vector3) {
        self.pos = pos;
        self.pos_txt = vec3::map(pos, |v| format!("{:.3}", v));
    }

    pub fn set_posture(&mut self, right: Vector3, up: Vector3) {
        self.right = vec3::normalized(right);
        self.up = vec3::normalized(up);
        self.right_txt = vec3::map(self.right, |v| format!("{:.3}", v));
        self.up_txt = vec3::map(self.up, |v| format!("{:.3}", v));
    }
}

pub struct CameraControlTab {
    camera_enabled: bool,
    pub(crate) camera_state: CameraState,
    pub(crate) slice_state: SliceState,
    move_xy: Vector2<f64>,
    move_z: f64,
    move_speed: f64,
    rot_pitch_yaw: Vector2<f64>,
    rot_roll: f64,
    release_mouse_left: bool,
    to_cnt: Sender<UICommand>,
    ids: Ids,
}

impl CameraControlTab {
    pub fn new(to_cnt: Sender<UICommand>, ui: &mut conrod_core::Ui) -> Self {
        let mut ids = Ids::new(ui.widget_id_generator());
        ids.position_textbox
            .resize(3, &mut ui.widget_id_generator());
        ids.position_label.resize(3, &mut ui.widget_id_generator());
        ids.sep.resize(2, &mut ui.widget_id_generator());

        ids.posture_label.resize(3, &mut ui.widget_id_generator());
        ids.posture_xyz_label
            .resize(3, &mut ui.widget_id_generator());
        ids.posture_textbox.resize(9, &mut ui.widget_id_generator());

        CameraControlTab {
            camera_enabled: true,
            camera_state: CameraState::new(),
            slice_state: SliceState::new(),
            move_xy: vec2::zero(),
            move_z: 0.,
            move_speed: 10.,
            rot_pitch_yaw: vec2::zero(),
            rot_roll: 0.,
            release_mouse_left: false,
            to_cnt,
            ids,
        }
    }

    pub fn ids(&self) -> &Ids {
        &self.ids
    }

    pub fn gui(&mut self, ui: &mut conrod_core::UiCell) {
        self.selector_gui(ui);
        self.camera_rotation_gui(ui);
        self.camera_position_gui(ui);
        if self.release_mouse_left {
            self.release_mouse_left = false;
        }
    }

    pub fn release_mouse_left(&mut self) {
        self.release_mouse_left = true;
    }

    fn selector_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;

        let half = (WIN_W as f64 - MARGIN * 3.0) / 2.0;

        for e in widget::Toggle::new(self.camera_enabled)
            .label("Camera")
            .align_left_of(ids.canvas)
            .align_top_of(ids.canvas)
            .w_h(half, 50.)
            .set(ids.camera_toggle, ui)
        {
            if !self.camera_enabled {
                self.camera_enabled = e;
            }
        }

        for e in widget::Toggle::new(!self.camera_enabled)
            .label("Slice")
            .align_top_of(ids.canvas)
            .right_from(ids.camera_toggle, MARGIN)
            .w_h(half, 50.)
            .set(ids.slice_toggle, ui)
        {
            if self.camera_enabled {
                self.camera_enabled = !e;
            }
        }

        widget::Rectangle::fill_with([WIN_W as f64 - MARGIN * 2.0, 2.], color::GRAY)
            .align_left_of(ids.canvas)
            .down_from(ids.camera_toggle, MARGIN)
            .set(ids.sep[0], ui);
    }

    fn camera_position_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;

        // Position Title
        {
            widget::Text::new("Position")
                .align_left_of(ids.canvas)
                .down_from(ids.sep[0], MARGIN)
                .set(ids.position_title, ui);
        }

        // Control Pad
        {
            widget::Circle::fill(CONTROL_GRIP_SIZE)
                .color(color::GRAY)
                .x_y_relative_to(ids.z_pad, 0., self.move_z * CONTROL_PAD_SIZE)
                .set(ids.z_grip, ui);

            let grip_z_range = 1.0;
            for (_, y) in widget::XYPad::new(
                0.,
                0.,
                0.,
                self.move_z,
                -grip_z_range / 2.0,
                grip_z_range / 2.0,
            )
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
                self.move_z = y;
            }

            widget::Circle::fill(CONTROL_GRIP_SIZE)
                .color(color::GRAY)
                .x_y_relative_to(
                    ids.xy_pad,
                    self.move_xy[0] * CONTROL_PAD_SIZE,
                    self.move_xy[1] * CONTROL_PAD_SIZE,
                )
                .set(ids.xy_grip, ui);

            let grip_x_range = 1.0;
            let grip_y_range = 1.0;
            for (x, y) in widget::XYPad::new(
                self.move_xy[0],
                -grip_x_range / 2.0,
                grip_x_range / 2.0,
                self.move_xy[1],
                -grip_y_range / 2.0,
                grip_y_range / 2.0,
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
                self.move_xy = [x, y];
            }
        }

        // Position Values
        {
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
                let display = if self.camera_enabled {
                    &self.camera_state.pos_txt[i]
                } else {
                    &self.slice_state.pos_txt[i]
                };
                for e in widget::TextBox::new(display)
                    .w_h(120., 24.)
                    .right_from(ids.position_label[i], 0.)
                    .align_middle_y_of(ids.position_label[i])
                    .set(ids.position_textbox[i], ui)
                {
                    if let Event::Update(txt) = e {
                        if self.camera_enabled {
                            self.camera_state.pos_txt[i] = txt;
                        } else {
                            self.slice_state.pos_txt[i] = txt;
                        }
                    }
                }
            }
        }

        // Move Speed
        {
            widget::Text::new("Speed: ")
                .h(24.)
                .down_from(ids.xy_pad, MARGIN)
                .align_left_of(ids.canvas)
                .set(ids.label_move_speed, ui);

            for txt in widget::TextBox::new(&self.move_speed.to_string())
                .w_h(80., 24.)
                .align_middle_y_of(ids.label_move_speed)
                .right_from(ids.label_move_speed, 0.)
                .left_justify()
                .set(ids.move_speed, ui)
            {
                match txt {
                    Event::Update(s) => {
                        if let Ok(f) = s.parse() {
                            self.move_speed = f
                        }
                    }
                    _ => (),
                }
            }
        }

        // Update Position Button
        {
            for _ in widget::Button::new()
                .label("Update position")
                .right_from(ids.z_pad, CONTROL_PAD_SIZE + MARGIN)
                .down_from(ids.position_textbox[2], MARGIN)
                .w_h(140.0, 40.)
                .set(ids.update_position_button, ui)
            {
                if self.camera_enabled {
                    let pos_txt = &self.camera_state.pos_txt;
                    match (pos_txt[0].parse(), pos_txt[1].parse(), pos_txt[2].parse()) {
                        (Ok(x), Ok(y), Ok(z)) => {
                            self.camera_state.pos = [x, y, z];
                            self.to_cnt
                                .send(UICommand::CameraMoveTo(self.camera_state.pos))
                                .unwrap()
                        }
                        _ => (),
                    }
                } else {
                    let pos_txt = &self.slice_state.pos_txt;
                    match (pos_txt[0].parse(), pos_txt[1].parse(), pos_txt[2].parse()) {
                        (Ok(x), Ok(y), Ok(z)) => {
                            let old = self.slice_state.pos;
                            self.slice_state.pos = [x, y, z];
                            self.to_cnt
                                .send(UICommand::SliceMove(vec3::sub(self.slice_state.pos, old)))
                                .unwrap()
                        }
                        _ => (),
                    }
                }
            }
        }

        // Auto View
        {
            for _ in widget::Button::new()
                .label("Auto View")
                .right_from(ids.update_position_button, MARGIN)
                .down_from(ids.position_textbox[2], MARGIN)
                .w_h(120.0, 40.)
                .set(ids.auto_view_button, ui)
            {
                let right = self.slice_state.right;
                let up = self.slice_state.up;
                self.camera_state.set_posture(right, up);
                self.to_cnt
                    .send(UICommand::CameraSetPosture { right, up })
                    .unwrap();
                let forward = vec3::cross(self.slice_state.right, self.slice_state.up);
                let d = vec3::mul(forward, 250.);
                self.to_cnt
                    .send(UICommand::CameraMoveTo(vec3::add(self.slice_state.pos, d)))
                    .unwrap();
                self.to_cnt.send(UICommand::CameraUpdate).unwrap();
            }
        }

        widget::Rectangle::fill_with([WIN_W as f64 - MARGIN * 2.0, 2.], color::GRAY)
            .align_left_of(ids.canvas)
            .down_from(ids.label_move_speed, MARGIN)
            .set(ids.sep[1], ui);

        if self.release_mouse_left {
            self.move_xy = vec2::zero();
            self.move_z = 0.;
        }
        if !vec2::is_zero(self.move_xy) || self.move_z != 0.0 {
            if self.camera_enabled {
                let t = vec3::mul(vec2::to_vec3(self.move_xy, -self.move_z), self.move_speed);
                self.to_cnt.send(UICommand::CameraMove(t)).unwrap();
                let t = vec3::cast(t);
                self.camera_state.set_position(vec3::add(
                    self.camera_state.pos,
                    mat4::mul_vec3(self.camera_state.orthogonal(), t),
                ));
            } else {
                let t = vec3::mul(vec2::to_vec3(self.move_xy, self.move_z), self.move_speed);
                let t = vec3::cast(t);
                self.to_cnt.send(UICommand::SliceMove(t)).unwrap();
                let pos = vec3::add(self.slice_state.pos, t);
                self.slice_state.set_position(pos);
            };
        }
    }

    fn camera_rotation_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;

        // Rotation Title
        {
            widget::Text::new("Rotation")
                .down_from(ids.sep[1], MARGIN)
                .align_left_of(ids.canvas)
                .set(ids.rotation_title, ui);
        }

        // Control Pad
        {
            widget::Circle::fill(CONTROL_GRIP_SIZE)
                .color(color::GRAY)
                .x_y_relative_to(ids.roll_pad, self.rot_roll * CONTROL_PAD_SIZE, 0.)
                .set(ids.roll_grip, ui);
            let grip_roll_range = 1.0;
            for (x, _) in widget::XYPad::new(
                self.rot_roll,
                -grip_roll_range / 2.0,
                grip_roll_range / 2.0,
                0.,
                0.,
                0.,
            )
            .color(color::ALPHA)
            .label("\n\nRoll")
            .label_color(color::GRAY)
            .line_thickness(0.)
            .value_font_size(0)
            .w_h(CONTROL_PAD_SIZE, 1.)
            .down_from(ids.pitch_yaw_pad, MARGIN)
            .set(ids.roll_pad, ui)
            {
                self.rot_roll = x;
            }

            widget::Circle::fill(CONTROL_GRIP_SIZE)
                .color(color::WHITE)
                .x_y_relative_to(
                    ids.pitch_yaw_pad,
                    self.rot_pitch_yaw[0] * CONTROL_PAD_SIZE,
                    self.rot_pitch_yaw[1] * CONTROL_PAD_SIZE,
                )
                .set(ids.pitch_yaw_grip, ui);
            let ball_x_range = 1.0;
            let ball_y_range = 1.0;
            for (x, y) in widget::XYPad::new(
                self.rot_pitch_yaw[0],
                -ball_x_range / 2.0,
                ball_x_range / 2.0,
                self.rot_pitch_yaw[1],
                -ball_y_range / 2.0,
                ball_y_range / 2.0,
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
                self.rot_pitch_yaw = [x, y];
            }
        }

        // Posture Values
        {
            widget::Text::new("X: ")
                .w_h(30., 24.)
                .down_from(ids.posture_label[0], 5.)
                .right_from(ids.pitch_yaw_pad, CONTROL_PAD_SIZE + MARGIN)
                .set(ids.posture_xyz_label[0], ui);
            widget::Text::new("Y: ")
                .h(24.)
                .down_from(ids.posture_xyz_label[0], 5.)
                .right_from(ids.pitch_yaw_pad, CONTROL_PAD_SIZE + MARGIN)
                .set(ids.posture_xyz_label[1], ui);
            widget::Text::new("Z: ")
                .h(24.)
                .down_from(ids.posture_xyz_label[1], 5.)
                .right_from(ids.pitch_yaw_pad, CONTROL_PAD_SIZE + MARGIN)
                .set(ids.posture_xyz_label[2], ui);

            // Right
            {
                widget::Text::new("Right")
                    .h(24.)
                    .down_from(ids.rotation_title, 5.)
                    .right_from(ids.pitch_yaw_pad, 30. + CONTROL_PAD_SIZE + MARGIN)
                    .set(ids.posture_label[0], ui);
                for i in 0..3 {
                    let display = if self.camera_enabled {
                        &self.camera_state.right_txt[i]
                    } else {
                        &self.slice_state.right_txt[i]
                    };
                    widget::Text::new(&display)
                        .w_h(120., 24.)
                        .align_middle_y_of(ids.posture_xyz_label[i])
                        .align_left_of(ids.posture_label[0])
                        .set(ids.posture_textbox[i], ui)
                }
            }

            // Up
            {
                widget::Text::new("Up")
                    .h(24.)
                    .down_from(ids.rotation_title, 5.)
                    .right_from(ids.posture_label[0], 120.0 + MARGIN)
                    .set(ids.posture_label[1], ui);
                for i in 0..3 {
                    let display = if self.camera_enabled {
                        &self.camera_state.up_txt[i]
                    } else {
                        &self.slice_state.up_txt[i]
                    };
                    widget::Text::new(&display)
                        .w_h(120., 24.)
                        .align_middle_y_of(ids.posture_xyz_label[i])
                        .align_left_of(ids.posture_label[1])
                        .set(ids.posture_textbox[3 + i], ui)
                }
            }
        }

        if self.release_mouse_left {
            self.rot_pitch_yaw = vec2::zero();
            self.rot_roll = 0.;
        }
        if !vec2::is_zero(self.rot_pitch_yaw) || self.rot_roll != 0.0 {
            if self.camera_enabled {
                let axis = [
                    self.rot_pitch_yaw[1],
                    -self.rot_pitch_yaw[0],
                    -self.rot_roll,
                ];
                let axis = vec3::cast(axis);
                self.to_cnt.send(UICommand::CameraRotate(axis)).unwrap();
                let axis = mat4::mul_vec3(self.camera_state.orthogonal(), axis);
                let rot = quaternion::axis_angle(axis, 0.01);
                let right = quaternion::rotate_vector(rot, self.camera_state.right);
                let up = quaternion::rotate_vector(rot, self.camera_state.up);
                self.camera_state.set_posture(right, up);
            } else {
                let axis = [
                    -self.rot_pitch_yaw[1],
                    self.rot_pitch_yaw[0],
                    -self.rot_roll,
                ];
                let axis = vec3::cast(axis);
                self.to_cnt.send(UICommand::SliceRotate(axis)).unwrap();
                let rot = quaternion::axis_angle(axis, 0.01);
                let right = quaternion::rotate_vector(rot, self.slice_state.right);
                let up = quaternion::rotate_vector(rot, self.slice_state.up);
                self.slice_state.set_posture(right, up);
            }
        }

        {
            for _ in widget::Button::new()
                .label("xy")
                .down_from(ids.roll_pad, 2.0 * MARGIN)
                .w_h(60.0, 40.)
                .set(ids.xy_button, ui)
            {
                let right = [1., 0., 0.];
                let up = [0., 1., 0.];
                if self.camera_enabled {
                    self.camera_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::CameraSetPosture { right, up })
                        .unwrap();
                } else {
                    self.slice_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::SliceSetPosture { right, up })
                        .unwrap();
                }
            }

            for _ in widget::Button::new()
                .label("yz")
                .right_from(ids.xy_button, MARGIN)
                .down_from(ids.roll_pad, 2.0 * MARGIN)
                .w_h(60.0, 40.)
                .set(ids.yz_button, ui)
            {
                let right = [0., -1., 0.];
                let up = [0., 0., 1.];
                if self.camera_enabled {
                    self.camera_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::CameraSetPosture { right, up })
                        .unwrap();
                } else {
                    self.slice_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::SliceSetPosture { right, up })
                        .unwrap();
                }
            }

            for _ in widget::Button::new()
                .label("xz")
                .down_from(ids.roll_pad, 2.0 * MARGIN)
                .right_from(ids.yz_button, MARGIN)
                .w_h(60.0, 40.)
                .set(ids.xz_button, ui)
            {
                let right = [1., 0., 0.];
                let up = [0., 0., 1.];
                if self.camera_enabled {
                    self.camera_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::CameraSetPosture { right, up })
                        .unwrap();
                } else {
                    self.slice_state.set_posture(right, up);
                    self.to_cnt
                        .send(UICommand::SliceSetPosture { right, up })
                        .unwrap();
                }
            }
        }
    }
}
