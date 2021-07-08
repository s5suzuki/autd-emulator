/*
 * File: interface.rs
 * Project: src
 * Created Date: 29/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::{
    net::UdpSocket,
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
};

const BUF_SIZE: usize = 65536;

macro_rules! if_not_open_or_cannot_read {
    ($is_open:expr, $cnt:stmt) => {
        if let Ok(open) = $is_open.read() {
            if !*open {
                $cnt
            }
        }
    };
}

macro_rules! write_rwlock {
    ($x_rwlock:expr, $value: expr) => {
        if let Ok(mut x) = $x_rwlock.write() {
            *x = $value;
        }
    };
}

pub struct Interface {
    is_open: Arc<RwLock<bool>>,
    socket: UdpSocket,
    th_handle: Option<JoinHandle<()>>,
    addr: String,
}

impl Interface {
    pub fn open(addr: &str) -> Result<Interface, std::io::Error> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Interface {
            is_open: Arc::new(RwLock::new(false)),
            socket,
            th_handle: None,
            addr: addr.to_owned(),
        })
    }

    pub fn start(&mut self, tx: Sender<Vec<u8>>) -> Result<(), std::io::Error> {
        let socket = self.socket.try_clone()?;
        write_rwlock!(self.is_open, true);
        let is_open = self.is_open.clone();
        let mut buf = [0; BUF_SIZE];
        let th_handle = thread::spawn(move || loop {
            if_not_open_or_cannot_read!(is_open, break);
            match socket.recv_from(&mut buf) {
                Ok((amt, _src)) => {
                    let rx_buf = &mut buf[..amt];
                    tx.send(rx_buf.to_vec()).ok();
                }
                Err(e) => eprintln!("{}", e),
            }
        });
        self.th_handle = Some(th_handle);

        Ok(())
    }

    pub fn close(&mut self) {
        if_not_open_or_cannot_read!(self.is_open, return);
        write_rwlock!(self.is_open, false);

        let socket = UdpSocket::bind("0.0.0.0:8080").unwrap();
        socket.send_to(&[0x00], &self.addr).unwrap();

        if let Some(handle) = self.th_handle.take() {
            handle.join().unwrap();
            self.th_handle = None;
        }
    }
}
