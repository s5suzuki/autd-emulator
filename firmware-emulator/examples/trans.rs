/*
* File: trans.rs
* Project: examples
* Created Date: 17/09/2021
* Author: Shun Suzuki
* -----
* Last Modified: 17/09/2021
* Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
* -----
* Copyright (c) 2021 Hapis Lab. All rights reserved.
*
*/

use firmware_emulator::Transducer;

#[allow(clippy::many_single_char_names)]
fn main() {
    let time_step = 0.5e-6; // 0.5us

    let l = 80e-3;
    let c = 200e-12;
    let r = 0.7e3;
    let c_p = 2700e-12;

    let mut trans = Transducer::new(150.0, c_p, c, l, r, time_step);

    for k in 0..2000 {
        let t = k as f64 * time_step;
        let v = if k % 50 < 25 { 0. } else { 24. };
        let i = trans.update(v);

        println!("{}, {}, {}", t, v, i);
    }
}
