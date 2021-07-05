/*
 * File: autd_event_handler.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 05/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::sound_source::SoundSource;
use acoustic_field_viewer::view::UpdateHandler;

use std::f32::consts::PI;
use std::sync::mpsc;

use crate::parser::{AUTDData, Geometry};
use autd3_core::hardware_defined::{NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM};

pub struct AUTDEventHandler {
    rx_from_autd: mpsc::Receiver<Vec<u8>>,
}

impl AUTDEventHandler {
    pub fn new(rx_from_autd: mpsc::Receiver<Vec<u8>>) -> AUTDEventHandler {
        AUTDEventHandler { rx_from_autd }
    }

    fn make_autd_transducers(geo: Geometry) -> Vec<SoundSource> {
        let mut transducers = Vec::new();
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if autd3_core::hardware_defined::is_missing_transducer(x, y) {
                    continue;
                }
                let x_dir = vecmath::vec3_scale(geo.right, TRANS_SPACING_MM as f32 * x as f32);
                let y_dir = vecmath::vec3_scale(geo.up, TRANS_SPACING_MM as f32 * y as f32);
                let zdir = vecmath::vec3_cross(geo.right, geo.up);
                let pos = geo.origin;
                let pos = vecmath::vec3_add(pos, x_dir);
                let pos = vecmath::vec3_add(pos, y_dir);
                transducers.push(SoundSource::new(pos, zdir, PI));
            }
        }
        transducers
    }

    pub fn update(&self, update_handler: &mut UpdateHandler) {
        if let Ok(d) = self.rx_from_autd.try_recv() {
            let data = crate::parser::parse(d);
            for d in data {
                match d {
                    AUTDData::Geometries(geometries) => {
                        update_handler.sources.borrow_mut().clear();
                        for geometry in geometries {
                            let transducers = Self::make_autd_transducers(geometry);
                            for trans in transducers {
                                update_handler.sources.borrow_mut().push(trans);
                            }
                        }
                        update_handler.update_position();
                        update_handler.update_phase();
                    }
                    AUTDData::Gain(gain) => {
                        for (&phase, source) in gain
                            .phases
                            .iter()
                            .zip(update_handler.sources.borrow_mut().iter_mut())
                        {
                            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
                        }
                        update_handler.update_phase();
                    }
                    AUTDData::Clear => {
                        for source in update_handler.sources.borrow_mut().iter_mut() {
                            source.phase = 0.;
                        }
                    }
                    AUTDData::Pause => {}
                    AUTDData::Resume => {}
                    _ => (),
                }
            }
        }
    }
}
