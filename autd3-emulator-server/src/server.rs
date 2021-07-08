/*
 * File: server.rs
 * Project: src
 * Created Date: 07/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::sync::mpsc::{self, Receiver};

use crate::{autd_data::AUTDData, interface::Interface, parser};

pub struct AUTDServer {
    interface: Interface,
    rx: Receiver<Vec<u8>>,
}

impl AUTDServer {
    pub fn new(addr: &str) -> Result<Self, std::io::Error> {
        let (tx, rx) = mpsc::channel();
        let mut interface = Interface::open(addr)?;
        interface.start(tx)?;

        Ok(Self { interface, rx })
    }

    pub fn update<F: FnOnce(Vec<AUTDData>)>(&mut self, f: F) {
        if let Ok(raw_buf) = self.rx.try_recv() {
            let data = parser::parse(raw_buf);
            f(data);
        }
    }

    pub fn close(&mut self) {
        self.interface.close()
    }
}
