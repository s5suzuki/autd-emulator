/*
 * File: server.rs
 * Project: src
 * Created Date: 09/05/2022
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2022 Hapis Lab. All rights reserved.
 *
 */

use std::sync::mpsc::{self, Receiver};

use acoustic_field_viewer::Vector3;
use autd3_core::{
    Body, GlobalHeader, TxDatagram, MSG_CLEAR, MSG_EMU_GEOMETRY_SET, MSG_RD_CPU_VERSION,
    MSG_RD_FPGA_FUNCTION, MSG_RD_FPGA_VERSION, NUM_TRANS_IN_UNIT, NUM_TRANS_X, NUM_TRANS_Y,
    TRANS_SPACING_MM,
};
use autd3_firmware_emulator::Emulator;

use crate::interface::Interface;

pub struct Geometry {
    pub origin: Vector3,
    pub right: Vector3,
    pub up: Vector3,
}

impl Geometry {
    pub fn make_autd_transducers(&self) -> Vec<(Vector3, Vector3)> {
        let mut transducers = Vec::new();
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if autd3_core::is_missing_transducer(x, y) {
                    continue;
                }
                let x_dir = vecmath::vec3_scale(self.right, TRANS_SPACING_MM as f32 * x as f32);
                let y_dir = vecmath::vec3_scale(self.up, TRANS_SPACING_MM as f32 * y as f32);
                let zdir = vecmath::vec3_cross(self.right, self.up);
                let pos = self.origin;
                let pos = vecmath::vec3_add(pos, x_dir);
                let pos = vecmath::vec3_add(pos, y_dir);
                transducers.push((pos, zdir));
            }
        }
        transducers
    }
}

pub enum AUTDEvent {
    Clear,
    RequestFpgaVersion,
    RequestFpgaFunctions,
    RequestCpuVersion,
    Normal,
    Geometries(Vec<Geometry>),
}

pub struct AUTDServer {
    _interface: Interface,
    rx: Receiver<Vec<u8>>,
    emulator: Emulator,
    tx_buf: TxDatagram,
}

impl AUTDServer {
    pub fn new(addr: &str) -> Result<Self, std::io::Error> {
        let (tx, rx) = mpsc::channel();
        let mut interface = Interface::open(addr)?;
        interface.start(tx)?;

        Ok(Self {
            _interface: interface,
            rx,
            emulator: Emulator::new(0),
            tx_buf: TxDatagram::new(0),
        })
    }

    fn set_device_num(&mut self, n: usize) {
        self.emulator = Emulator::new(n);
        self.tx_buf = TxDatagram::new(n);
    }

    fn to_vec3(buf: &[u8]) -> Vector3 {
        let x = Self::to_f32(&buf[0..4]);
        let y = Self::to_f32(&buf[4..8]);
        let z = Self::to_f32(&buf[8..12]);
        [x, y, z]
    }

    fn to_f32(buf: &[u8]) -> f32 {
        f32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]])
    }

    fn parse_as_geometry(buf: &[u8]) -> Vec<Geometry> {
        let mut res = Vec::new();
        for bytes in buf.chunks_exact(std::mem::size_of::<u16>() * NUM_TRANS_IN_UNIT) {
            let origin = Self::to_vec3(&bytes[0..12]);
            let right = Self::to_vec3(&bytes[12..24]);
            let up = Self::to_vec3(&bytes[24..36]);
            res.push(Geometry { origin, right, up });
        }
        res
    }

    pub fn update<F: FnOnce(AUTDEvent, &Emulator)>(&mut self, f: F) {
        if let Ok(raw_buf) = self.rx.try_recv() {
            unsafe {
                if raw_buf.len() >= std::mem::size_of::<GlobalHeader>() {
                    self.tx_buf
                        .header_mut()
                        .clone_from((raw_buf.as_ptr() as *const GlobalHeader).as_ref().unwrap());

                    let event = match self.tx_buf.header().msg_id {
                        MSG_EMU_GEOMETRY_SET => {
                            let geometries = Self::parse_as_geometry(
                                &raw_buf[std::mem::size_of::<GlobalHeader>()..],
                            );
                            self.set_device_num(geometries.len());
                            AUTDEvent::Geometries(geometries)
                        }
                        MSG_CLEAR => AUTDEvent::Clear,
                        MSG_RD_CPU_VERSION => AUTDEvent::RequestCpuVersion,
                        MSG_RD_FPGA_VERSION => AUTDEvent::RequestFpgaVersion,
                        MSG_RD_FPGA_FUNCTION => AUTDEvent::RequestFpgaFunctions,
                        _ => {
                            let num_bodies = (raw_buf.len() - std::mem::size_of::<GlobalHeader>())
                                / std::mem::size_of::<Body>();
                            let src = raw_buf.as_ptr().add(std::mem::size_of::<GlobalHeader>());
                            self.tx_buf
                                .body_mut()
                                .iter_mut()
                                .take(num_bodies)
                                .for_each(|b| {
                                    b.data.clone_from_slice(std::slice::from_raw_parts(
                                        src as *const u16,
                                        NUM_TRANS_IN_UNIT,
                                    ))
                                });
                            self.tx_buf.num_bodies = num_bodies;
                            self.emulator.send(&self.tx_buf);
                            AUTDEvent::Normal
                        }
                    };

                    f(event, &self.emulator);
                }
            }
        }
    }

    pub fn close(&mut self) {
        self._interface.close()
    }
}
