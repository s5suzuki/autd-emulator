/*
 * File: viewer_controller.rs
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
use acoustic_field_viewer::view::event::*;
use acoustic_field_viewer::view::UpdateHandler;
use vecmath_utils::*;

use crate::camera_helper;
use crate::ui::UICommand;

use std::sync::mpsc::Receiver;

pub struct ViewController {
    rx_ui_command: Receiver<UICommand>,
}

impl ViewController {
    pub fn new(rx_ui_command: Receiver<UICommand>) -> ViewController {
        ViewController { rx_ui_command }
    }

    pub fn update(&self, update_handler: &mut UpdateHandler, button: Option<Button>) {
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

        if let Ok(d) = self.rx_ui_command.try_recv() {
            match d {
                UICommand::CameraMove(t) => Self::camera_move(update_handler, vec3_cast(t)),
                UICommand::CameraRotate(t) => Self::camera_rotate(update_handler, vec3_cast(t)),
                UICommand::CameraSetPosture(f, u) => {
                    Self::camera_set_posture(update_handler, vec3_cast(f), vec3_cast(u))
                }
            }
        }
    }

    pub fn camera_move(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_move(&mut update_handler.camera, t);
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
}
