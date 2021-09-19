/*
 * File: transducer.rs
 * Project: src
 * Created Date: 17/09/2021
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
pub struct Transducer {
    r_d: f64,
    c_p: f64,
    c_s: f64,
    l: f64,
    r: f64,
    v_buf: VecDeque<f64>,
    q_s: f64,
    i_p: f64,
    i_s: f64,
    h: f64,
}

impl Transducer {
    pub fn new(r_d: f64, c_p: f64, c_s: f64, l: f64, r: f64, h: f64) -> Self {
        let mut v_buf = VecDeque::new();
        v_buf.resize(4, 0.);
        Self {
            r_d,
            c_p,
            c_s,
            l,
            r,
            v_buf,
            i_p: 0.,
            i_s: 0.,
            q_s: 0.,
            h,
        }
    }

    pub fn update(&mut self, v: f64) -> f64 {
        self.v_buf.pop_front();
        self.v_buf.push_back(v);

        // Runge–Kutta
        let k_0_0 = self.h * self.f_0(1, self.q_s, self.i_s, self.i_p);
        let k_0_1 = self.h * self.f_1(1, self.q_s, self.i_s, self.i_p);
        let k_0_2 = self.h * self.f_2(1, self.q_s, self.i_s, self.i_p);

        let k_1_0 = self.h
            * self.f_0(
                2,
                self.q_s + k_0_0 * 0.5,
                self.i_s + k_0_1 * 0.5,
                self.i_p + k_0_2 * 0.5,
            );
        let k_1_1 = self.h
            * self.f_1(
                2,
                self.q_s + k_0_0 * 0.5,
                self.i_s + k_0_1 * 0.5,
                self.i_p + k_0_2 * 0.5,
            );
        let k_1_2 = self.h
            * self.f_2(
                2,
                self.q_s + k_0_0 * 0.5,
                self.i_s + k_0_1 * 0.5,
                self.i_p + k_0_2 * 0.5,
            );

        let k_2_0 = self.h
            * self.f_0(
                2,
                self.q_s + k_1_0 * 0.5,
                self.i_s + k_1_1 * 0.5,
                self.i_p + k_1_2 * 0.5,
            );
        let k_2_1 = self.h
            * self.f_1(
                2,
                self.q_s + k_1_0 * 0.5,
                self.i_s + k_1_1 * 0.5,
                self.i_p + k_1_2 * 0.5,
            );
        let k_2_2 = self.h
            * self.f_2(
                2,
                self.q_s + k_1_0 * 0.5,
                self.i_s + k_1_1 * 0.5,
                self.i_p + k_1_2 * 0.5,
            );

        let k_3_0 = self.h * self.f_0(3, self.q_s + k_2_0, self.i_s + k_2_1, self.i_p + k_2_2);
        let k_3_1 = self.h * self.f_1(3, self.q_s + k_2_0, self.i_s + k_2_1, self.i_p + k_2_2);
        let k_3_2 = self.h * self.f_2(3, self.q_s + k_2_0, self.i_s + k_2_1, self.i_p + k_2_2);

        self.q_s += (k_0_0 + 2. * k_1_0 + 2. * k_2_0 + k_3_0) / 6.;
        self.i_s += (k_0_1 + 2. * k_1_1 + 2. * k_2_1 + k_3_1) / 6.;
        self.i_p += (k_0_2 + 2. * k_1_2 + 2. * k_2_2 + k_3_2) / 6.;

        self.i_s
    }

    fn f_0(&self, _t: usize, _y_0: f64, y_1: f64, _y_2: f64) -> f64 {
        y_1
    }

    fn f_1(&self, t: usize, y_0: f64, y_1: f64, y_2: f64) -> f64 {
        (-1.0 / self.c_s * y_0 - (self.r + self.r_d) * y_1 - self.r_d * y_2 + self.v_buf[t])
            / self.l
    }

    fn f_2(&self, t: usize, y_0: f64, y_1: f64, y_2: f64) -> f64 {
        (1.0 / self.c_s * y_0
            + (self.r + self.r_d) * y_1
            + (self.r_d - self.l / (self.r_d * self.c_p)) * y_2
            + self.l * (self.v_buf[t] - self.v_buf[t - 1]) / (self.h * self.r_d)
            - self.v_buf[t])
            / self.l
    }
}
