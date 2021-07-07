/*
 * File: viewer_controller.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::view::UpdateHandler;
use piston_window::{Button, Key};

use crate::{camera_helper, ui::UICommand, Vector3};

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
                UICommand::CameraMove(t) => {
                    Self::camera_move(update_handler, vecmath::vec3_cast(t))
                }
                UICommand::CameraMoveTo(t) => Self::camera_move_to(update_handler, t),
                UICommand::CameraRotate(t) => Self::camera_rotate(update_handler, t),
                UICommand::CameraSetPosture { right, up } => {
                    Self::camera_set_posture(update_handler, right, up)
                }
                UICommand::SliceMove(t) => Self::slice_move(update_handler, t),
                UICommand::CameraUpdate => self.is_init = true,
                UICommand::SliceRotate(t) => Self::slice_rotate(update_handler, t),
                UICommand::SliceSetPosture { right, up } => {
                    Self::slice_set_posture(update_handler, right, up)
                }
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
                .send(UICommand::SliceSetPosture {
                    right: update_handler.field_slice_viewer.right(),
                    up: update_handler.field_slice_viewer.up(),
                })
                .unwrap();
            self.to_ui
                .send(UICommand::CameraSetPosture {
                    right: update_handler.camera.right,
                    up: update_handler.camera.up,
                })
                .unwrap();
            self.is_init = false;
        }
    }

    pub fn camera_move(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_move(&mut update_handler.camera, t);
        update_handler.update_camera_pos();
    }

    pub fn camera_move_to(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_move_to(&mut update_handler.camera, t);
        update_handler.update_camera_pos();
    }

    pub fn camera_rotate(update_handler: &mut UpdateHandler, t: Vector3) {
        camera_helper::camera_rotate(&mut update_handler.camera, t, 0.01);
        update_handler.update_camera_pos();
    }

    pub fn camera_set_posture(update_handler: &mut UpdateHandler, right: Vector3, up: Vector3) {
        update_handler.camera.right = right;
        update_handler.camera.up = up;
        update_handler.camera.forward = vecmath::vec3_cross(right, up);
        update_handler.update_camera_pos();
    }

    pub fn slice_move(update_handler: &mut UpdateHandler, t: Vector3) {
        update_handler.field_slice_viewer.translate(t);
    }

    pub fn slice_rotate(update_handler: &mut UpdateHandler, axis: Vector3) {
        update_handler.field_slice_viewer.rotate(axis, 0.05);
    }

    pub fn slice_set_posture(update_handler: &mut UpdateHandler, right: Vector3, up: Vector3) {
        update_handler.field_slice_viewer.set_posture(right, up);
    }
}
