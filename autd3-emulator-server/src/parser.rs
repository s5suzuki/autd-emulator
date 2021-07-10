/*
 * File: parser.rs
 * Project: src
 * Created Date: 29/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::mem::size_of;

use autd3_core::hardware_defined::{RxGlobalControlFlags, RxGlobalHeader};

use crate::{
    autd_data::{AutdData, Gain, Geometry, Modulation},
    DelayOffset, SeqFocus, Sequence, Vector3,
};

pub struct Parser {
    mod_div: u16,
    mod_buf: Option<Vec<u8>>,
    wavelength: u16,
    seq_buf: Option<Vec<(Vector3, u8)>>,
    seq_div: u16,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            mod_div: 10,
            mod_buf: None,
            wavelength: 8500,
            seq_buf: None,
            seq_div: 0,
        }
    }

    pub fn parse(&mut self, raw_buf: Vec<u8>) -> Vec<AutdData> {
        let mut res = Vec::new();

        let (cmd, ctrl_flag) = unsafe {
            let header = raw_buf.as_ptr() as *const RxGlobalHeader;
            ((*header).command, (*header).ctrl_flag)
        };

        res.push(AutdData::CtrlFlag(ctrl_flag));
        match cmd {
            autd3_core::hardware_defined::CommandType::Clear => res.push(AutdData::Clear),
            autd3_core::hardware_defined::CommandType::Op => {
                res.push(AutdData::Resume);
                if let Some(modulation) = self.parse_as_modulation(&raw_buf) {
                    res.push(AutdData::Modulation(modulation));
                }

                if raw_buf.len() > size_of::<RxGlobalHeader>() {
                    let gain = Self::parse_as_gain(&raw_buf[size_of::<RxGlobalHeader>()..]);
                    res.push(AutdData::Gain(gain));
                }
            }
            autd3_core::hardware_defined::CommandType::ReadCpuVerLsb => {
                res.push(AutdData::RequestCpuVerLsb)
            }
            autd3_core::hardware_defined::CommandType::ReadCpuVerMsb => {
                res.push(AutdData::RequestCpuVerMsb)
            }
            autd3_core::hardware_defined::CommandType::ReadFpgaVerLsb => {
                res.push(AutdData::RequestFpgaVerLsb)
            }
            autd3_core::hardware_defined::CommandType::ReadFpgaVerMsb => {
                res.push(AutdData::RequestFpgaVerMsb)
            }
            autd3_core::hardware_defined::CommandType::SeqMode => {
                if let Some(sequence) = self.parse_as_sequence(&raw_buf) {
                    res.push(AutdData::Sequence(sequence));
                    res.push(AutdData::Resume);
                }
            }
            autd3_core::hardware_defined::CommandType::SetDelay => {
                let delay_enable =
                    Self::parse_as_delay_enable(&raw_buf[size_of::<RxGlobalHeader>()..]);
                res.push(AutdData::DelayOffset(delay_enable));
            }
            autd3_core::hardware_defined::CommandType::Pause => res.push(AutdData::Pause),
            autd3_core::hardware_defined::CommandType::Resume => res.push(AutdData::Resume),
            autd3_core::hardware_defined::CommandType::EmulatorSetGeometry => {
                let geo = Self::parse_as_geometry(&raw_buf[size_of::<RxGlobalHeader>()..]);
                res.push(AutdData::Geometries(geo))
            }
        }

        res
    }

    fn parse_as_geometry(buf: &[u8]) -> Vec<Geometry> {
        let mut res = Vec::new();
        for bytes in buf.chunks_exact(std::mem::size_of::<Geometry>()) {
            let origin = to_vec3(&bytes[0..12]);
            let right = to_vec3(&bytes[12..24]);
            let up = to_vec3(&bytes[24..36]);
            res.push(Geometry { origin, right, up });
        }
        res
    }

    fn parse_as_sequence(&mut self, buf: &[u8]) -> Option<Sequence> {
        unsafe {
            let header = buf.as_ptr() as *const RxGlobalHeader;
            let seq_begin = (*header)
                .ctrl_flag
                .contains(RxGlobalControlFlags::SEQ_BEGIN);
            let seq_end = (*header).ctrl_flag.contains(RxGlobalControlFlags::SEQ_END);
            let cursor = buf.as_ptr().add(size_of::<RxGlobalHeader>()) as *const u16;
            let seq_size = cursor.read();
            let offset = if seq_begin {
                self.seq_buf = Some(vec![]);
                self.seq_div = cursor.add(1).read();
                self.wavelength = cursor.add(2).read();
                5
            } else {
                1
            };
            let mut cursor = cursor.add(offset) as *const SeqFocus;
            for _ in 0..seq_size {
                let x = (*cursor).x(self.wavelength);
                let y = (*cursor).y(self.wavelength);
                let z = (*cursor).z(self.wavelength);
                let duty = (*cursor).amp();
                if let Some(buf) = &mut self.seq_buf {
                    buf.push(([x, y, z], duty));
                }
                cursor = cursor.add(1);
            }

            if seq_end {
                Some(Sequence {
                    seq_div: self.seq_div,
                    seq_data: self.seq_buf.take().unwrap(),
                })
            } else {
                None
            }
        }
    }

    fn parse_as_modulation(&mut self, buf: &[u8]) -> Option<Modulation> {
        unsafe {
            let header = buf.as_ptr() as *const RxGlobalHeader;
            let mod_size = (*header).mod_size as usize;
            let offset = if (*header)
                .ctrl_flag
                .contains(RxGlobalControlFlags::MOD_BEGIN)
            {
                self.mod_buf = Some(vec![]);
                self.mod_div = u16::from_ne_bytes([(*header).mod_data[0], (*header).mod_data[1]]);
                2
            } else {
                0
            };
            if let Some(buf) = &mut self.mod_buf {
                buf.extend_from_slice(&(*header).mod_data[offset..(offset + mod_size)]);
            }

            if (*header).ctrl_flag.contains(RxGlobalControlFlags::MOD_END) {
                Some(Modulation {
                    mod_div: self.mod_div,
                    mod_data: self.mod_buf.take().unwrap(),
                })
            } else {
                None
            }
        }
    }

    fn parse_as_gain(buf: &[u8]) -> Gain {
        let mut amps = Vec::with_capacity(buf.len() / 2);
        let mut phases = Vec::with_capacity(buf.len() / 2);
        for amp_phase in buf.chunks_exact(2) {
            phases.push(amp_phase[0]);
            amps.push(amp_phase[1]);
        }
        Gain { amps, phases }
    }

    fn parse_as_delay_enable(buf: &[u8]) -> DelayOffset {
        let mut delay_offset = Vec::with_capacity(buf.len() / 2);
        for d in buf.chunks_exact(2) {
            delay_offset.push((d[0], d[1]));
        }
        DelayOffset { delay_offset }
    }
}

fn to_vec3(buf: &[u8]) -> Vector3 {
    let x = to_f32(&buf[0..4]);
    let y = to_f32(&buf[4..8]);
    let z = to_f32(&buf[8..12]);
    [x, y, z]
}

fn to_f32(buf: &[u8]) -> f32 {
    f32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]])
}
