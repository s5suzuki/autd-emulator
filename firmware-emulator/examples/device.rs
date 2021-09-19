/*
* File: device.rs
* Project: examples
* Created Date: 19/09/2021
* Author: Shun Suzuki
* -----
* Last Modified: 19/09/2021
* Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
* -----
* Copyright (c) 2021 Hapis Lab. All rights reserved.
*
*/

use autd3_core::hardware_defined::NUM_TRANS_IN_UNIT;
use firmware_emulator::Device;

fn main() {
    let mut device = Device::new(1000);

    let duties = vec![0xFF; NUM_TRANS_IN_UNIT];
    let mut phases = vec![0; NUM_TRANS_IN_UNIT];
    let mut delay = vec![0; NUM_TRANS_IN_UNIT];

    phases[1] = 0x7F;
    delay[2] = 5;

    device.set_duties(&duties);
    device.set_phases(&phases);
    device.set_delay(&delay);

    device.set_silent_mode(false);

    for _ in 0..100000 {
        device.update();
        println!(
            "{}, {}, {}",
            device.output_when(0, 0.).unwrap(),
            device.output_when(1, 0.).unwrap(),
            device.output_when(2, 0.).unwrap()
        );
    }

    let duties = vec![0; NUM_TRANS_IN_UNIT];
    device.set_duties(&duties);
    for _ in 0..100000 {
        device.update();
        println!(
            "{}, {}, {}",
            device.output_when(0, 0.).unwrap(),
            device.output_when(1, 0.).unwrap(),
            device.output_when(2, 0.).unwrap()
        );
    }
}
