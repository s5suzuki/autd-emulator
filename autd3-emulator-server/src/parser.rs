/*
 * File: parser.rs
 * Project: src
 * Created Date: 29/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 13/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::{mem::size_of, vec};

use autd3_core::hardware_defined::{CPUControlFlags, GainMode, GlobalHeader, NUM_TRANS_IN_UNIT};

use crate::{
    autd_data::{AutdData, Gain, Geometry, Modulation},
    DelayOffset, GainSequence, PointSequence, SeqFocus, Vector3,
};

pub struct Parser {
    mod_div: u16,
    mod_buf: Option<Vec<u8>>,
    wavelength: u16,
    point_seq_buf: Option<Vec<(Vector3, u8)>>,
    point_seq_div: u16,
    gain_seq_buf: Option<Vec<Gain>>,
    gain_seq_div: u16,
    seq_gain_mode: GainMode,
    gain_seq_size: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            mod_div: 10,
            mod_buf: None,
            wavelength: 8500,
            point_seq_buf: None,
            point_seq_div: 0,
            gain_seq_buf: None,
            gain_seq_div: 0,
            seq_gain_mode: GainMode::DutyPhaseFull,
            gain_seq_size: 0,
        }
    }

    pub fn parse(&mut self, raw_buf: Vec<u8>) -> Vec<AutdData> {
        let mut res = Vec::new();

        let (msg_id, fpga_flag, cpu_flag) = unsafe {
            let header = raw_buf.as_ptr() as *const GlobalHeader;
            ((*header).msg_id, (*header).fpga_flag, (*header).cpu_flag)
        };

        res.push(AutdData::CtrlFlag(fpga_flag, cpu_flag));
        match msg_id {
            autd3_core::hardware_defined::MSG_CLEAR => res.push(AutdData::Clear),
            autd3_core::hardware_defined::MSG_RD_CPU_V_LSB => res.push(AutdData::RequestCpuVerLsb),
            autd3_core::hardware_defined::MSG_RD_CPU_V_MSB => res.push(AutdData::RequestCpuVerMsb),
            autd3_core::hardware_defined::MSG_RD_FPGA_V_LSB => {
                res.push(AutdData::RequestFpgaVerLsb)
            }
            autd3_core::hardware_defined::MSG_RD_FPGA_V_MSB => {
                res.push(AutdData::RequestFpgaVerMsb)
            }
            autd3_core::hardware_defined::MSG_EMU_GEOMETRY_SET => {
                let geo = Self::parse_as_geometry(&raw_buf[size_of::<GlobalHeader>()..]);
                res.push(AutdData::Geometries(geo))
            }
            _ => {
                if let Some(modulation) = self.parse_as_modulation(&raw_buf) {
                    res.push(AutdData::Modulation(modulation));
                }

                if cpu_flag.contains(autd3_core::hardware_defined::CPUControlFlags::DELAY_OFFSET) {
                    let offset_delay =
                        Self::parse_as_offset_delay(&raw_buf[size_of::<GlobalHeader>()..]);
                    res.push(AutdData::DelayOffset(offset_delay));
                } else if fpga_flag
                    .contains(autd3_core::hardware_defined::FPGAControlFlags::OP_MODE)
                {
                    if fpga_flag.contains(autd3_core::hardware_defined::FPGAControlFlags::SEQ_MODE)
                    {
                        if let Some(seq) = self.parse_as_gain_sequence(&raw_buf) {
                            res.push(AutdData::GainSequence(seq));
                        }
                    } else if let Some(seq) = self.parse_as_point_sequence(&raw_buf) {
                        res.push(AutdData::PointSequence(seq));
                    }
                } else if raw_buf.len() > size_of::<GlobalHeader>() {
                    let gain = Self::parse_as_gain(
                        &raw_buf[size_of::<GlobalHeader>()..],
                        GainMode::DutyPhaseFull,
                    );

                    for g in gain {
                        res.push(AutdData::Gain(g));
                    }
                }
            }
        }

        res
    }

    fn parse_as_geometry(buf: &[u8]) -> Vec<Geometry> {
        let mut res = Vec::new();
        for bytes in buf.chunks_exact(std::mem::size_of::<u16>() * NUM_TRANS_IN_UNIT) {
            let origin = to_vec3(&bytes[0..12]);
            let right = to_vec3(&bytes[12..24]);
            let up = to_vec3(&bytes[24..36]);
            res.push(Geometry { origin, right, up });
        }
        res
    }

    fn parse_as_point_sequence(&mut self, buf: &[u8]) -> Option<PointSequence> {
        unsafe {
            let header = buf.as_ptr() as *const GlobalHeader;
            let seq_begin = (*header).cpu_flag.contains(CPUControlFlags::SEQ_BEGIN);
            let seq_end = (*header).cpu_flag.contains(CPUControlFlags::SEQ_END);
            let cursor = buf.as_ptr().add(size_of::<GlobalHeader>()) as *const u16;
            let seq_size = cursor.read();
            let offset = if seq_begin {
                self.point_seq_buf = Some(vec![]);
                self.point_seq_div = cursor.add(1).read();
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
                if let Some(buf) = &mut self.point_seq_buf {
                    buf.push(([x, y, z], duty));
                }
                cursor = cursor.add(1);
            }

            if seq_end {
                Some(PointSequence {
                    seq_div: self.point_seq_div as u32 + 1,
                    seq_data: self.point_seq_buf.take().unwrap(),
                    wavelength: self.wavelength,
                })
            } else {
                None
            }
        }
    }

    fn parse_as_gain_sequence(&mut self, buf: &[u8]) -> Option<GainSequence> {
        unsafe {
            let header = buf.as_ptr() as *const GlobalHeader;
            let seq_begin = (*header).cpu_flag.contains(CPUControlFlags::SEQ_BEGIN);
            let seq_end = (*header).cpu_flag.contains(CPUControlFlags::SEQ_END);
            let cursor = buf.as_ptr().add(size_of::<GlobalHeader>()) as *const u16;
            if seq_begin {
                self.gain_seq_buf = Some(vec![]);
                self.seq_gain_mode = match cursor.read() {
                    1 => GainMode::DutyPhaseFull,
                    2 => GainMode::PhaseFull,
                    4 => GainMode::PhaseHalf,
                    _ => GainMode::DutyPhaseFull,
                };
                self.gain_seq_div = cursor.add(1).read();
                self.gain_seq_size = cursor.add(2).read() as _;
                return None;
            }

            if let Some(b) = &mut self.gain_seq_buf {
                b.append(&mut Self::parse_as_gain(
                    &buf[size_of::<GlobalHeader>()..],
                    self.seq_gain_mode,
                ))
            }

            if seq_end {
                if let Some(b) = &mut self.gain_seq_buf {
                    b.resize(
                        self.gain_seq_size,
                        Gain {
                            amps: vec![],
                            phases: vec![],
                        },
                    );
                }
                Some(GainSequence {
                    gain_mode: self.seq_gain_mode,
                    seq_div: self.gain_seq_div as u32 + 1,
                    seq_data: self.gain_seq_buf.take().unwrap(),
                })
            } else {
                None
            }
        }
    }

    fn parse_as_modulation(&mut self, buf: &[u8]) -> Option<Modulation> {
        unsafe {
            let header = buf.as_ptr() as *const GlobalHeader;
            let mod_size = (*header).mod_size as usize;
            let offset = if (*header).cpu_flag.contains(CPUControlFlags::MOD_BEGIN) {
                self.mod_buf = Some(vec![]);
                self.mod_div = u16::from_ne_bytes([(*header).mod_data[0], (*header).mod_data[1]]);
                2
            } else {
                0
            };
            if let Some(buf) = &mut self.mod_buf {
                buf.extend_from_slice(&(*header).mod_data[offset..(offset + mod_size)]);
            }

            if (*header).cpu_flag.contains(CPUControlFlags::MOD_END) {
                Some(Modulation {
                    mod_div: self.mod_div as u32 + 1,
                    mod_data: self.mod_buf.take().unwrap(),
                })
            } else {
                None
            }
        }
    }

    fn parse_as_gain(buf: &[u8], gain_mode: GainMode) -> Vec<Gain> {
        match gain_mode {
            GainMode::DutyPhaseFull => {
                let mut amps = Vec::with_capacity(buf.len() / 2);
                let mut phases = Vec::with_capacity(buf.len() / 2);
                for amp_phase in buf.chunks_exact(2) {
                    phases.push(amp_phase[0]);
                    amps.push(amp_phase[1]);
                }
                vec![Gain { amps, phases }]
            }
            GainMode::PhaseFull => {
                let mut phases1 = Vec::with_capacity(buf.len() / 2);
                let mut phases2 = Vec::with_capacity(buf.len() / 2);
                for phase in buf.chunks_exact(2) {
                    phases1.push(phase[0]);
                    phases2.push(phase[1]);
                }
                vec![
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases1,
                    },
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases2,
                    },
                ]
            }
            GainMode::PhaseHalf => {
                let mut phases1 = Vec::with_capacity(buf.len() / 2);
                let mut phases2 = Vec::with_capacity(buf.len() / 2);
                let mut phases3 = Vec::with_capacity(buf.len() / 2);
                let mut phases4 = Vec::with_capacity(buf.len() / 2);
                for phase in buf.chunks_exact(2) {
                    let p = phase[0] & 0x0F;
                    let p = p << 4 | p;
                    phases1.push(p);
                    let p = phase[0] & 0xF0;
                    let p = p | p >> 4;
                    phases2.push(p);
                    let p = phase[1] & 0x0F;
                    let p = p << 4 | p;
                    phases3.push(p);
                    let p = phase[1] & 0xF0;
                    let p = p | p >> 4;
                    phases4.push(p);
                }
                vec![
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases1,
                    },
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases2,
                    },
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases3,
                    },
                    Gain {
                        amps: vec![0xFF; buf.len() / 2],
                        phases: phases4,
                    },
                ]
            }
        }
    }

    fn parse_as_offset_delay(buf: &[u8]) -> DelayOffset {
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
