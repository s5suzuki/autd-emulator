/*
 * File: parser.rs
 * Project: src
 * Created Date: 29/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 05/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::mem::size_of;

use autd3_core::hardware_defined::RxGlobalHeader;

use crate::Vector3;

#[allow(dead_code)]
pub struct Modulation {
    pub(crate) mod_data: Vec<u8>,
}

#[derive(Debug)]
pub struct Gain {
    pub(crate) amps: Vec<u8>,
    pub(crate) phases: Vec<u8>,
}

#[derive(Debug)]
pub struct Geometry {
    pub(crate) origin: Vector3,
    pub(crate) right: Vector3,
    pub(crate) up: Vector3,
}

pub enum AUTDData {
    Modulation(Modulation),
    Gain(Gain),
    Geometries(Vec<Geometry>),
    Clear,
    Pause,
    Resume,
}

pub fn parse(raw_buf: Vec<u8>) -> Vec<AUTDData> {
    let mut res = Vec::new();

    let (cmd, _ctrl_flag) = unsafe {
        let header = raw_buf.as_ptr() as *const RxGlobalHeader;
        ((*header).command, (*header).ctrl_flag)
    };

    match cmd {
        autd3_core::hardware_defined::CommandType::Clear => res.push(AUTDData::Clear),
        autd3_core::hardware_defined::CommandType::Op => {
            res.push(AUTDData::Resume);

            let modulation = parse_as_modulation(&raw_buf);
            res.push(AUTDData::Modulation(modulation));

            if raw_buf.len() > size_of::<RxGlobalHeader>() {
                let gain = parse_as_gain(&raw_buf[size_of::<RxGlobalHeader>()..]);
                res.push(AUTDData::Gain(gain));
            }
        }
        autd3_core::hardware_defined::CommandType::ReadCpuVerLsb => {}
        autd3_core::hardware_defined::CommandType::ReadCpuVerMsb => {}
        autd3_core::hardware_defined::CommandType::ReadFpgaVerLsb => {}
        autd3_core::hardware_defined::CommandType::ReadFpgaVerMsb => {}
        autd3_core::hardware_defined::CommandType::SeqMode => {}
        autd3_core::hardware_defined::CommandType::SetDelay => {}
        autd3_core::hardware_defined::CommandType::Pause => res.push(AUTDData::Pause),
        autd3_core::hardware_defined::CommandType::Resume => res.push(AUTDData::Resume),
        autd3_core::hardware_defined::CommandType::EmulatorSetGeometry => {
            let geo = parse_as_geometry(&raw_buf[size_of::<RxGlobalHeader>()..]);
            res.push(AUTDData::Geometries(geo))
        }
    }

    res
}

pub fn parse_as_geometry(buf: &[u8]) -> Vec<Geometry> {
    let mut res = Vec::new();
    for bytes in buf.chunks_exact(std::mem::size_of::<Geometry>()) {
        let mut cursor = 0;
        let origin = to_vec3(&bytes, &mut cursor);
        let right = to_vec3(&bytes, &mut cursor);
        let up = to_vec3(&bytes, &mut cursor);
        res.push(Geometry { origin, right, up });
    }
    res
}

pub fn parse_as_modulation(buf: &[u8]) -> Modulation {
    unsafe {
        let header = buf.as_ptr() as *const RxGlobalHeader;
        let mod_size = (*header).mod_size as usize;
        let mod_data = (*header).mod_data[0..mod_size].to_vec();
        Modulation { mod_data }
    }
}

pub fn parse_as_gain(buf: &[u8]) -> Gain {
    let mut amps = Vec::with_capacity(buf.len() / 2);
    let mut phases = Vec::with_capacity(buf.len() / 2);
    for amp_phase in buf.chunks_exact(2) {
        phases.push(amp_phase[0]);
        amps.push(amp_phase[1]);
    }
    Gain { amps, phases }
}

fn to_vec3(buf: &[u8], cursor: &mut usize) -> Vector3 {
    let x = to_f32(buf, cursor);
    let y = to_f32(buf, cursor);
    let z = to_f32(buf, cursor);
    [x, y, z]
}

fn to_f32(buf: &[u8], cursor: &mut usize) -> f32 {
    let i = *cursor;
    *cursor += 4;
    f32::from_ne_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]])
}
