/*
 * File: viewer_controller.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::view::{UpdateFlag, ViewWindow};
use piston_window::{Button, Event, Key, PressEvent};

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

    pub fn update(
        &mut self,
        view_window: &mut ViewWindow,
        e: &Event,
        update_flag: &mut UpdateFlag,
    ) {
        let travel = 5.0;
        match e.press_args() {
            Some(Button::Keyboard(Key::Up)) => {
                Self::camera_move(view_window, [0., travel, 0.], update_flag);
            }
            Some(Button::Keyboard(Key::Down)) => {
                Self::camera_move(view_window, [0., -travel, 0.], update_flag);
            }
            _ => (),
        }

        if let Ok(d) = self.from_ui.try_recv() {
            match d {
                UICommand::CameraMove(t) => {
                    Self::camera_move(view_window, vecmath::vec3_cast(t), update_flag)
                }
                UICommand::CameraMoveTo(t) => Self::camera_move_to(view_window, t, update_flag),
                UICommand::CameraRotate(t) => Self::camera_rotate(view_window, t, update_flag),
                UICommand::CameraSetPosture { right, up } => {
                    Self::camera_set_posture(view_window, right, up, update_flag)
                }
                UICommand::SliceMove(t) => Self::slice_move(view_window, t, update_flag),
                UICommand::CameraUpdate => self.is_init = true,
                UICommand::SliceRotate(t) => Self::slice_rotate(view_window, t, update_flag),
                UICommand::SliceSetPosture { right, up } => {
                    Self::slice_set_posture(view_window, right, up, update_flag)
                }
                _ => (),
            }
        }

        if self.is_init {
            self.to_ui
                .send(UICommand::CameraPos(view_window.camera.position))
                .unwrap();
            self.to_ui
                .send(UICommand::SlicePos(
                    view_window.field_slice_viewer.position(),
                ))
                .unwrap();
            self.to_ui
                .send(UICommand::SliceSetPosture {
                    right: view_window.field_slice_viewer.right(),
                    up: view_window.field_slice_viewer.up(),
                })
                .unwrap();
            self.to_ui
                .send(UICommand::CameraSetPosture {
                    right: view_window.camera.right,
                    up: view_window.camera.up,
                })
                .unwrap();
            self.is_init = false;
        }
    }

    fn camera_move(view_window: &mut ViewWindow, t: Vector3, update_flag: &mut UpdateFlag) {
        camera_helper::camera_move(&mut view_window.camera, t);
        *update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
    }

    fn camera_move_to(view_window: &mut ViewWindow, t: Vector3, update_flag: &mut UpdateFlag) {
        camera_helper::camera_move_to(&mut view_window.camera, t);
        *update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
    }

    fn camera_rotate(view_window: &mut ViewWindow, t: Vector3, update_flag: &mut UpdateFlag) {
        camera_helper::camera_rotate(&mut view_window.camera, t, 0.01);
        *update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
    }

    fn camera_set_posture(
        view_window: &mut ViewWindow,
        right: Vector3,
        up: Vector3,
        update_flag: &mut UpdateFlag,
    ) {
        view_window.camera.right = right;
        view_window.camera.up = up;
        view_window.camera.forward = vecmath::vec3_cross(right, up);
        *update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
    }

    fn slice_move(view_window: &mut ViewWindow, t: Vector3, update_flag: &mut UpdateFlag) {
        view_window.field_slice_viewer.translate(t);
        *update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    }

    fn slice_rotate(view_window: &mut ViewWindow, axis: Vector3, update_flag: &mut UpdateFlag) {
        view_window.field_slice_viewer.rotate(axis, 0.05);
        *update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    }

    fn slice_set_posture(
        view_window: &mut ViewWindow,
        right: Vector3,
        up: Vector3,
        update_flag: &mut UpdateFlag,
    ) {
        view_window.field_slice_viewer.set_posture(right, up);
        *update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    }
}
