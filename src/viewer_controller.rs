/*
 * File: viewer_controller.rs
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

use crate::Vector3;
use acoustic_field_viewer::view::event::*;
use acoustic_field_viewer::view::UpdateHandler;
use vecmath_utils::*;

use crate::camera_helper;
use crate::ui::UICommand;

use std::sync::mpsc::{Receiver, Sender};

pub struct ViewController {
    from_ui: Receiver<UICommand>,
    to_ui: Sender<UICommand>,
    is_init: bool,
}

impl ViewController {
    pub fn new(from_ui: Receiver<UICommand>, to_ui: Sender<UICommand>) -> ViewController {
        ViewController {
            from_ui,
            to_ui,
            is_init: true,
        }
    }

    pub fn update(&mut self, update_handler: &mut UpdateHandler, button: Option<Button>) {
        let travel = 5.0;
        match button {
            Some(Button::Keyboard(Key::Up)) => {
                Self::camera_move(update_handler, [0., travel, 0.]);
            }
            Some(Button::Keyboard(Key::Down)) => {
                Self::camera_move(update_handler, [0., -travel, 0.]);
            }
            _ => (),
        }

        if let Ok(d) = self.from_ui.try_recv() {
            match d {
                UICommand::CameraMove(t) => Self::camera_move(update_handler, vec3::cast(t)),
                UICommand::CameraMoveTo(t) => Self::camera_move_to(update_handler, t),
                UICommand::CameraRotate(t) => Self::camera_rotate(update_handler, vec3::cast(t)),
                UICommand::CameraSetPosture(f, u) => Self::camera_set_posture(update_handler, f, u),
                UICommand::SliceMove(t) => Self::slice_move(update_handler, t),
                UICommand::CameraUpdate => self.is_init = true,
                _ => (),
            }
        }

        if self.is_init {
            self.to_ui
                .send(UICommand::CameraPos(update_handler.camera.position))
                .unwrap();
            self.to_ui
                .send(UICommand::SlicePos(
                    update_handler.field_slice_viewer.position(),
                ))
                .unwrap();
            self.to_ui
                .send(UICommand::SlicePosture(
                    update_handler.field_slice_viewer.right(),
                    update_handler.field_slice_viewer.up(),
                    update_handler.field_slice_viewer.forward(),
                ))
                .unwrap();
            self.is_init = false;
        }
    }

    pub fn camera_move(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_move(&mut update_handler.camera, t);
        update_handler.update_position();
    }

    pub fn camera_move_to(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_move_to(&mut update_handler.camera, t);
        update_handler.update_position();
    }

    pub fn camera_rotate(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_rotate(&mut update_handler.camera, t, 0.01);
        update_handler.update_position();
    }

    pub fn camera_set_posture(update_handler: &mut UpdateHandler, forward: Vector3, up: Vector3) {
        update_handler.camera.forward = forward;
        update_handler.camera.up = up;
        update_handler.camera.right = vecmath::vec3_cross(up, forward);
        update_handler.update_position();
    }

    pub fn slice_move_to(update_handler: &mut UpdateHandler, t: Vector3) {
        let d = vec3::sub(update_handler.field_slice_viewer.position(), t);
        update_handler.field_slice_viewer.translate(d);
        update_handler.update_position();
    }

    pub fn slice_move(update_handler: &mut UpdateHandler, t: Vector3) {
        update_handler.field_slice_viewer.translate(t);
        update_handler.update_position();
    }
}
