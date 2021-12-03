/*
 * File: sound_sources.rs
 * Project: src
 * Created Date: 29/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use crate::{Vector3, Vector4};

#[derive(Clone, Copy, Debug)]
pub struct Drive {
    pub amp: f32,
    pub phase: f32,
    pub enable: f32,
    pub visible: f32,
}

impl Drive {
    pub fn new(amp: f32, phase: f32, enable: f32, visible: f32) -> Self {
        Self {
            amp,
            phase,
            enable,
            visible,
        }
    }
}

pub struct SoundSources {
    pos: Vec<Vector4>,
    dir: Vec<Vector3>,
    drive: Vec<Drive>,
}

impl SoundSources {
    pub fn new() -> Self {
        Self {
            pos: vec![],
            dir: vec![],
            drive: vec![],
        }
    }

    pub fn add(&mut self, pos: Vector3, dir: Vector3, drive: Drive) {
        self.pos.push(vecmath_util::to_vec4(pos));
        self.dir.push(dir);
        self.drive.push(drive);
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
