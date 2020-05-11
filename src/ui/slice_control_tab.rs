/*
 * File: camera_control_tab.rs
 * Project: ui
 * Created Date: 02/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 11/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::{widget, Labelable, Positionable, Sizeable, Widget};
use vecmath_utils::vec3;

use crate::color;
use crate::ui::ui_view::{MARGIN, WIN_W};
use crate::ui::UICommand;
use crate::Vector3;

use std::sync::mpsc::Sender;

widget_ids! {
    pub struct Ids {
        canvas,
        position_title,
        position_label[],
        position_textbox[],
        auto_view_button,
        sep,
        rotation_title,
    }
}

pub struct SliceControlTab {
    pos: Vector3,
    right: Vector3,
    up: Vector3,
    forward: Vector3,
    release_mouse_left: bool,
    to_cnt: Sender<UICommand>,
    ids: Ids,
}

impl SliceControlTab {
    pub fn new(to_cnt: Sender<UICommand>, ui: &mut conrod_core::Ui) -> Self {
        let mut ids = Ids::new(ui.widget_id_generator());
        ids.position_textbox
            .resize(3, &mut ui.widget_id_generator());
        ids.position_label.resize(3, &mut ui.widget_id_generator());

        Self {
            pos: vec3::zero(),
            right: vec3::zero(),
            up: vec3::zero(),
            forward: vec3::zero(),
            release_mouse_left: false,
            to_cnt,
            ids,
        }
    }

    pub fn ids(&self) -> &Ids {
        &self.ids
    }

    pub fn gui(&mut self, ui: &mut conrod_core::UiCell) {
        self.rotation_gui(ui);
        self.position_gui(ui);
        if self.release_mouse_left {
            self.release_mouse_left = false;
        }
    }

    pub fn release_mouse_left(&mut self) {
        self.release_mouse_left = true;
    }

    pub fn set_pos(&mut self, p: Vector3) {
        self.pos = p;
    }

    pub fn set_posture(&mut self, r: Vector3, u: Vector3, f: Vector3) {
        self.right = r;
        self.up = u;
        self.forward = f;
    }

    fn position_gui(&mut self, ui: &mut conrod_core::UiCell) {
        let ids = &self.ids;
        widget::Text::new("Position")
            .mid_top_of(ids.canvas)
            .align_left_of(ids.canvas)
            .set(ids.position_title, ui);

        widget::Text::new("X: ")
            .h(24.)
            .down_from(ids.position_title, MARGIN)
            .align_left_of(ids.canvas)
            .set(ids.position_label[0], ui);
        widget::Text::new("Y: ")
            .h(24.)
            .down_from(ids.position_label[0], 5.)
            .align_left_of(ids.canvas)
            .set(ids.position_label[1], ui);
        widget::Text::new("Z: ")
            .h(24.)
            .down_from(ids.position_label[1], 5.)
            .align_left_of(ids.canvas)
            .set(ids.position_label[2], ui);
        for i in 0..3 {
            for txt in widget::TextBox::new(&self.pos[i].to_string())
                .w_h(120., 24.)
                .right_from(ids.position_label[i], 0.)
                .align_middle_y_of(ids.position_label[i])
                .set(ids.position_textbox[i], ui)
            {
                match txt {
                    widget::text_box::Event::Update(s) => {
                        if let Ok(f) = s.parse() {
                            let old = self.pos;
                            self.pos[i] = f;
                            self.to_cnt
                                .send(UICommand::SliceMove(vec3::sub(self.pos, old)))
                                .unwrap()
                        }
                    }
                    _ => (),
                }
            }
        }

        for _ in widget::Button::new()
            .label("Auto View")
            .down_from(ids.position_title, MARGIN)
            .right_from(ids.position_textbox[0], MARGIN)
            .w_h(120.0, 40.)
            .set(ids.auto_view_button, ui)
        {
            self.to_cnt
                .send(UICommand::CameraSetPosture(self.forward, self.up))
                .unwrap();
            let d = vec3::mul(self.forward, 250.);
            self.to_cnt
                .send(UICommand::CameraMoveTo(vec3::add(self.pos, d)))
                .unwrap();
            self.to_cnt.send(UICommand::CameraUpdate).unwrap();
        }

        widget::Rectangle::fill_with([WIN_W as f64 - MARGIN * 2.0, 2.], color::GRAY)
            .align_left_of(ids.canvas)
            .down_from(ids.position_label[2], MARGIN)
            .set(ids.sep, ui);
    }

    fn rotation_gui(&mut self, ui: &mut conrod_core::UiCell) {}
}
