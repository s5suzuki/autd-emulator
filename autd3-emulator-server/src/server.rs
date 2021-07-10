/*
 * File: server.rs
 * Project: src
 * Created Date: 07/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::sync::mpsc::{self, Receiver};

use crate::{autd_data::AutdData, interface::Interface, parser::Parser};

pub struct AutdServer {
    interface: Interface,
    rx: Receiver<Vec<u8>>,
    parser: Parser,
}

impl AutdServer {
    pub fn new(addr: &str) -> Result<Self, std::io::Error> {
        let (tx, rx) = mpsc::channel();
        let mut interface = Interface::open(addr)?;
        interface.start(tx)?;

        Ok(Self {
            interface,
            rx,
            parser: Parser::new(),
        })
    }

    pub fn update<F: FnOnce(Vec<AutdData>)>(&mut self, f: F) {
        if let Ok(raw_buf) = self.rx.try_recv() {
            let data = self.parser.parse(raw_buf);
            f(data);
        }
    }

    pub fn close(&mut self) {
        self.interface.close()
    }
}
