/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;
use std::io::IoSlice;
use std::net::SocketAddr;

use crate::RawSocketAddr;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
    target_os = "macos",
))]
mod buf;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
pub use buf::with_sendmmsg_buf;
#[cfg(target_os = "macos")]
pub use buf::with_sendmsg_x_buf;

pub struct SendMsgHdr<'a, const C: usize> {
    pub iov: [IoSlice<'a>; C],
    c_addr: Option<UnsafeCell<RawSocketAddr>>,
    pub n_send: usize,
}

impl<'a, const C: usize> SendMsgHdr<'a, C> {
    pub fn new(iov: [IoSlice<'a>; C], addr: Option<SocketAddr>) -> Self {
        let c_addr = addr.map(|addr| UnsafeCell::new(RawSocketAddr::from(addr)));
        SendMsgHdr {
            iov,
            c_addr,
            n_send: 0,
        }
    }
}

impl<'a, const C: usize> AsRef<[IoSlice<'a>]> for SendMsgHdr<'a, C> {
    fn as_ref(&self) -> &[IoSlice<'a>] {
        self.iov.as_ref()
    }
}
