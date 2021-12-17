/*
 * File: autd_data.rs
 * Project: src
 * Created Date: 07/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 17/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use autd3_core::{
    hardware_defined::{
        CPUControlFlags, FPGAControlFlags, NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM,
    },
    sequence::GainMode,
};

use crate::Vector3;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Modulation {
    pub mod_data: Vec<u8>,
    pub mod_div: u32,
}

#[derive(Debug, Clone)]
pub struct Gain {
    pub amps: Vec<u8>,
    pub phases: Vec<u8>,
}

pub(crate) struct SeqFocus {
    buf: [u16; 4],
}

impl SeqFocus {
    pub(crate) fn x(&self, wavelength: u16) -> f32 {
        let v: u32 = self.buf[0] as u32;
        let v: u32 = v | (((self.buf[1] as u32) & 0x0001) << 16);
        let v: u32 = v | (((self.buf[1] as u32) & 0x0002) << 30);
        unsafe {
            let v: i32 = *(&v as *const _ as *const i32);
            v as f32 * wavelength as f32 / 1000. / 256.0
        }
    }
    pub(crate) fn y(&self, wavelength: u16) -> f32 {
        let v: u32 = (self.buf[1] as u32) >> 2;
        let v: u32 = v | (((self.buf[2] as u32) & 0x0007) << 14);
        let v: u32 = v | (((self.buf[2] as u32) & 0x0008) << 28);
        unsafe {
            let v: i32 = *(&v as *const _ as *const i32);
            v as f32 * wavelength as f32 / 1000. / 256.0
        }
    }

    pub(crate) fn z(&self, wavelength: u16) -> f32 {
        let v: u32 = (self.buf[2] as u32) >> 4;
        let v: u32 = v | (((self.buf[3] as u32) & 0x001F) << 12);
        let v: u32 = v | (((self.buf[3] as u32) & 0x0020) << 26);
        unsafe {
            let v: i32 = *(&v as *const _ as *const i32);
            v as f32 * wavelength as f32 / 1000. / 256.0
        }
    }
    pub(crate) fn amp(&self) -> u8 {
        ((self.buf[3] & 0x3FC0) >> 6) as u8
    }
}

#[derive(Debug)]
pub struct PointSequence {
    pub seq_div: u32,
    pub seq_data: Vec<(Vector3, u8)>,
    pub wavelength: u16,
}

#[derive(Debug)]
pub struct GainSequence {
    pub gain_mode: GainMode,
    pub seq_div: u32,
    pub seq_data: Vec<Gain>,
}

#[derive(Debug)]
pub struct DelayOffset {
    pub delay_offset: Vec<(u8, u8)>,
}

#[derive(Debug)]
pub struct Geometry {
    pub origin: Vector3,
    pub right: Vector3,
    pub up: Vector3,
}

#[derive(Debug)]
pub enum AutdData {
    Modulation(Modulation),
    Gain(Gain),
    Geometries(Vec<Geometry>),
    CtrlFlag(FPGAControlFlags, CPUControlFlags),
    Clear,
    RequestFpgaVerMsb,
    RequestFpgaVerLsb,
    RequestCpuVerMsb,
    RequestCpuVerLsb,
    PointSequence(PointSequence),
    GainSequence(GainSequence),
    DelayOffset(DelayOffset),
}

impl Geometry {
    pub fn make_autd_transducers(&self) -> Vec<(Vector3, Vector3)> {
        let mut transducers = Vec::new();
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if autd3_core::hardware_defined::is_missing_transducer(x, y) {
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
