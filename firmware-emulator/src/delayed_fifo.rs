/*
* File: delayed_fifo.rs
* Project: src
* Created Date: 19/09/2021
* Author: Shun Suzuki
* -----
* Last Modified: 19/09/2021
* Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
* -----
* Copyright (c) 2021 Hapis Lab. All rights reserved.
*
*/

use std::collections::VecDeque;

#[derive(Clone)]
pub struct DelayedFifo {
    delay: usize,
    duty: VecDeque<u8>,
}

impl DelayedFifo {
    pub fn new() -> Self {
        Self {
            delay: 0,
            duty: VecDeque::new(),
        }
    }

    pub fn set(&mut self, delay: u8) {
        self.delay = delay as _;
    }

    pub fn update(&mut self, duty: u8) -> u8 {
        while self.duty.len() > self.delay {
            self.duty.pop_front();
        }
        self.duty.push_back(duty);

        if self.duty.len() <= self.delay {
            0
        } else {
            *self.duty.front().unwrap()
        }
    }
}
