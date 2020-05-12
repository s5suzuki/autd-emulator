/*
 * File: autd_event_handler.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 12/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::sound_source::SoundSource;
use acoustic_field_viewer::view::UpdateHandler;
use vecmath_utils::vec3;

use std::f32::consts::PI;
use std::sync::mpsc;

use crate::consts::{NUM_TRANS_X, NUM_TRANS_Y, TRANS_SIZE};
use crate::parser::{AUTDData, Geometry};

pub struct AUTDEventHandler {
    rx_from_autd: mpsc::Receiver<Vec<u8>>,
}

impl AUTDEventHandler {
    pub fn new(rx_from_autd: mpsc::Receiver<Vec<u8>>) -> AUTDEventHandler {
        AUTDEventHandler { rx_from_autd }
    }

    fn is_missing_transducer(x: usize, y: usize) -> bool {
        y == 1 && (x == 1 || x == 2 || x == 16)
    }

    fn make_autd_transducers(geo: Geometry) -> Vec<SoundSource> {
        let mut transducers = Vec::new();
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if Self::is_missing_transducer(x, y) {
                    continue;
                }
                let x_dir = vec3::mul(geo.right, TRANS_SIZE * x as f32);
                let y_dir = vec3::mul(geo.up, TRANS_SIZE * y as f32);
                let zdir = vec3::cross(geo.right, geo.up);
                let pos = geo.origin;
                let pos = vec3::add(pos, x_dir);
                let pos = vec3::add(pos, y_dir);
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
                        update_handler.sources.borrow_mut().clear();
                        update_handler.update_position();
                        update_handler.update_phase();
                    }
                    _ => (),
                }
            }
        }
    }
}
