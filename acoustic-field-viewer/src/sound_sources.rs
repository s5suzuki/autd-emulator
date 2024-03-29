/*
 * File: sound_sources.rs
 * Project: src
 * Created Date: 29/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::f32::consts::PI;

use bytemuck::{Pod, Zeroable};

use crate::{Vector3, Vector4};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct Drive {
    pub amp: f32,
    pub phase: f32,
    pub enable: f32,
    pub wave_num: f32,
}

impl Drive {
    pub fn new(amp: f32, phase: f32, enable: f32, frequency: f32, sound_speed: f32) -> Self {
        Self {
            amp,
            phase,
            enable,
            wave_num: Self::to_wave_number(frequency, sound_speed),
        }
    }

    pub fn set_wave_number(&mut self, frequency: f32, sound_speed: f32) {
        self.wave_num = Self::to_wave_number(frequency, sound_speed);
    }

    fn to_wave_number(frequency: f32, sound_speed: f32) -> f32 {
        2.0 * PI * frequency / sound_speed
    }
}

pub struct SoundSources {
    pos: Vec<Vector4>,
    dir: Vec<Vector3>,
    drive: Vec<Drive>,
    visibilities: Vec<f32>,
}

impl SoundSources {
    pub fn new() -> Self {
        Self {
            pos: vec![],
            dir: vec![],
            drive: vec![],
            visibilities: vec![],
        }
    }

    pub fn add(&mut self, pos: Vector3, dir: Vector3, drive: Drive, visibility: f32) {
        self.pos.push(vecmath_util::to_vec4(pos));
        self.dir.push(dir);
        self.drive.push(drive);
        self.visibilities.push(visibility);
    }

    pub fn clear(&mut self) {
        self.pos.clear();
        self.dir.clear();
        self.drive.clear();
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn positions(&self) -> impl ExactSizeIterator<Item = &Vector4> {
        self.pos.iter()
    }

    pub fn position_dirs(&self) -> impl ExactSizeIterator<Item = (&Vector4, &Vector3)> {
        self.pos.iter().zip(self.dir.iter())
    }

    pub fn drives(&self) -> impl ExactSizeIterator<Item = &Drive> {
        self.drive.iter()
    }

    pub fn drives_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Drive> {
        self.drive.iter_mut()
    }

    pub fn visibilities(&self) -> impl ExactSizeIterator<Item = &f32> {
        self.visibilities.iter()
    }

    pub fn visibilities_mut(&mut self) -> impl ExactSizeIterator<Item = &mut f32> {
        self.visibilities.iter_mut()
    }

    pub fn positions_drives_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (&Vector4, &mut Drive)> {
        self.pos.iter().zip(self.drive.iter_mut())
    }
}

impl Default for SoundSources {
    fn default() -> Self {
        Self::new()
    }
}
