/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
#[cfg(unix)]
use std::path::PathBuf;
use std::time::Duration;

use g3_types::metrics::NodeName;

use crate::{StatsdClient, StatsdMetricsSink};

#[cfg(feature = "yaml")]
mod yaml;

const UDP_DEFAULT_PORT: u16 = 8125;

#[derive(Debug, Clone)]
pub enum StatsdBackend {
    Udp(SocketAddr, Option<IpAddr>),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl Default for StatsdBackend {
    fn default() -> Self {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), UDP_DEFAULT_PORT);
        StatsdBackend::Udp(addr, None)
    }
}

#[derive(Debug, Clone)]
pub struct StatsdClientConfig {
    backend: StatsdBackend,
    prefix: NodeName,
    cache_size: usize,
    max_segment_size: Option<usize>,
    pub emit_interval: Duration,
}

impl Default for StatsdClientConfig {
    fn default() -> Self {
        StatsdClientConfig::with_prefix(NodeName::default())
    }
}

impl StatsdClientConfig {
    pub fn with_prefix(prefix: NodeName) -> Self {
        StatsdClientConfig {
            backend: StatsdBackend::default(),
            prefix,
            cache_size: 256 * 1024,
            max_segment_size: None,
            emit_interval: Duration::from_millis(200),
        }
    }

    pub fn set_backend(&mut self, target: StatsdBackend) {
        self.backend = target;
    }

    pub fn set_prefix(&mut self, prefix: NodeName) {
        self.prefix = prefix;
    }

    pub fn build(&self) -> io::Result<StatsdClient> {
        let sink = match &self.backend {
            StatsdBackend::Udp(addr, bind) => {
                let bind_ip = bind.unwrap_or_else(|| match addr {
                    SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                });
                let socket = UdpSocket::bind(SocketAddr::new(bind_ip, 0))?;
                StatsdMetricsSink::udp_with_capacity(
                    *addr,
                    socket,
                    self.cache_size,
                    self.max_segment_size,
                )
            }
            #[cfg(unix)]
            StatsdBackend::Unix(path) => {
                let socket = UnixDatagram::unbound()?;
                StatsdMetricsSink::unix_with_capacity(
                    path.clone(),
                    socket,
                    self.cache_size,
                    self.max_segment_size,
                )
            }
        };

        Ok(StatsdClient::new(self.prefix.clone(), sink))
    }
}
