/*
 * File: parser.rs
 * Project: src
 * Created Date: 29/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 29/04/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::vec_utils::Vector3;

const HEADER_SIZE: usize = 128;
const HEADER_MOD_SIZE_IDX: usize = 3;
const HEADER_MOD_BASE_IDX: usize = 4;

const GEOMETRY_HEADER: u32 = 0xFFFFF0FF;

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
}

pub fn parse(raw_buf: Vec<u8>) -> Vec<AUTDData> {
    let mut res = Vec::new();
    if is_geometry(&raw_buf) {
        let geo = parse_as_geometry(&raw_buf[4..]);
        res.push(AUTDData::Geometries(geo));
    } else if is_header(&raw_buf) {
        let modulation = parse_as_modulation(&raw_buf);
        res.push(AUTDData::Modulation(modulation));

        if include_gain(&raw_buf) {
            let gain = parse_as_gain(&raw_buf[HEADER_SIZE..]);
            res.push(AUTDData::Gain(gain));
        }
    }

    res
}

pub fn is_geometry(buf: &[u8]) -> bool {
    let header = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
    header == GEOMETRY_HEADER
}

pub fn is_header(buf: &[u8]) -> bool {
    buf.len() >= HEADER_SIZE
}

pub fn include_gain(buf: &[u8]) -> bool {
    buf.len() - HEADER_SIZE > 0
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
    let mod_size = buf[HEADER_MOD_SIZE_IDX];
    Modulation {
        mod_data: buf[HEADER_MOD_BASE_IDX..(HEADER_MOD_BASE_IDX + mod_size as usize)].to_vec(),
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
