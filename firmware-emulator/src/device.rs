/*
 * File: device.rs
 * Project: src
 * Created Date: 19/09/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/10/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::collections::VecDeque;

use autd3_core::hardware_defined::NUM_TRANS_IN_UNIT;

use crate::{
    consts::{CYCLE, TIME_STEP},
    delayed_fifo::DelayedFifo,
    modulator::Modulator,
    pwm::Pwm,
    silent_lpf::Lpf,
    transducer::Transducer,
};

pub struct Device {
    max_buf_size: usize,
    time: u16,
    transducers: Vec<Transducer>,
    outputs: Vec<VecDeque<f32>>,
    duties: Vec<u8>,
    offsets: Vec<u8>,
    phases: Vec<u8>,
    current_duties: Vec<u8>,
    current_phases: Vec<u8>,
    modulator: Modulator,
    delayed_fifo: Vec<DelayedFifo>,
    silent_mode: bool,
    silent_lpf: Vec<Lpf>,
}

impl Device {
    pub fn new(max_buf_size: usize) -> Self {
        Self {
            max_buf_size,
            transducers: vec![
                Transducer::new(150.0, 2700e-12, 200e-12, 80e-3, 0.7e3, TIME_STEP);
                NUM_TRANS_IN_UNIT
            ],
            outputs: vec![VecDeque::new(); NUM_TRANS_IN_UNIT],
            time: 0,
            duties: vec![0; NUM_TRANS_IN_UNIT],
            offsets: vec![1; NUM_TRANS_IN_UNIT],
            phases: vec![0; NUM_TRANS_IN_UNIT],
            current_duties: vec![0; NUM_TRANS_IN_UNIT],
            current_phases: vec![0; NUM_TRANS_IN_UNIT],
            modulator: Modulator::new(),
            delayed_fifo: vec![DelayedFifo::new(); NUM_TRANS_IN_UNIT],
            silent_mode: true,
            silent_lpf: vec![Lpf::new(); NUM_TRANS_IN_UNIT],
        }
    }

    pub fn set_silent_mode(&mut self, silent: bool) {
        self.silent_mode = silent;
    }

    pub fn set_duties(&mut self, duties: &[u8]) {
        self.duties.copy_from_slice(&duties[..NUM_TRANS_IN_UNIT]);
    }

    pub fn set_phases(&mut self, phases: &[u8]) {
        self.phases.copy_from_slice(&phases[..NUM_TRANS_IN_UNIT]);
    }

    pub fn set_mod(&mut self, modulation: &[u8], mod_idx_div: u16) {
        self.modulator.set(modulation, mod_idx_div);
    }

    pub fn set_delay(&mut self, delay: &[u8]) {
        for (i, &d) in delay.iter().enumerate().take(NUM_TRANS_IN_UNIT) {
            self.delayed_fifo[i].set(d);
        }
    }

    pub fn clear(&mut self) {
        self.duties.fill(0);
        self.offsets.fill(1);
        self.phases.fill(0);
    }

    pub fn update(&mut self) {
        if self.time == 0 {
            for i in 0..NUM_TRANS_IN_UNIT {
                let duty = self.modulator.modulate(self.duties[i]);
                let (duty, phase) = if self.silent_mode {
                    self.silent_lpf[i].update(duty, self.phases[i])
                } else {
                    (duty, self.phases[i])
                };
                let duty = self.delayed_fifo[i].update(duty);
                self.current_duties[i] = duty;
                self.current_phases[i] = phase;
            }
        }

        for i in 0..NUM_TRANS_IN_UNIT {
            let v = Pwm::output(
                self.time,
                self.current_duties[i],
                self.current_phases[i],
                self.offsets[i],
            );
            let output = self.transducers[i].update(v);
            if self.outputs[i].len() >= self.max_buf_size {
                self.outputs[i].pop_front();
            }
            self.outputs[i].push_back(output as _);
        }

        self.time += 1;
        if self.time >= CYCLE {
            self.time = 0;
            self.modulator.update();
        }
    }

    pub fn output_when(&self, trans_idx: usize, prev: f64) -> Option<f32> {
        let idx = (prev / TIME_STEP).round() as usize;
        if idx >= self.outputs[trans_idx].len() {
            return None;
        }
        let idx = self.outputs[trans_idx].len() - idx - 1;
        Some(self.outputs[trans_idx][idx])
    }
}
