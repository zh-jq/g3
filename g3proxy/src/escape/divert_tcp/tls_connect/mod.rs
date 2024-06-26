/*
 * Copyright 2024 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::sync::Arc;

use anyhow::anyhow;
use tokio::net::tcp;

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_io_ext::{AggregatedIo, LimitedReader, LimitedWriter};
use g3_openssl::{SslConnector, SslStream};
use g3_types::net::{Host, OpensslClientConfig};

use super::DivertTcpEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl DivertTcpEscaper {
    pub(super) async fn tls_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
        tls_application: TlsApplication,
    ) -> Result<
        SslStream<
            AggregatedIo<LimitedReader<tcp::OwnedReadHalf>, LimitedWriter<tcp::OwnedWriteHalf>>,
        >,
        TcpConnectError,
    > {
        let stream = self.tcp_connect_to(tcp_notes, task_notes).await?;
        let (ups_r, ups_w) = stream.into_split();

        // set limit config and add escaper stats, do not count in task stats
        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let ups_r = LimitedReader::new(
            ups_r,
            limit_config.shift_millis,
            limit_config.max_south,
            self.stats.clone() as _,
        );
        let mut ups_w = LimitedWriter::new(
            ups_w,
            limit_config.shift_millis,
            limit_config.max_north,
            self.stats.clone() as _,
        );

        self.send_pp2_header(&mut ups_w, tcp_notes, task_notes, Some(tls_name))
            .await?;

        let ssl = tls_config
            .build_ssl(tls_name, tcp_notes.upstream.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let connector = SslConnector::new(
            ssl,
            AggregatedIo {
                reader: ups_r,
                writer: ups_w,
            },
        )
        .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(tls_config.handshake_timeout, connector.connect()).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => {
                let e = anyhow::Error::new(e);
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &tcp_notes.upstream,
                    tls_application,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::UpstreamTlsHandshakeFailed(e))
            }
            Err(_) => {
                let e = anyhow!("upstream tls handshake timed out");
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &tcp_notes.upstream,
                    tls_application,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::UpstreamTlsHandshakeTimeout)
            }
        }
    }

    pub(super) async fn tls_new_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
    ) -> TcpConnectResult {
        let tls_stream = self
            .tls_connect_to(
                tcp_notes,
                task_notes,
                tls_config,
                tls_name,
                TlsApplication::TcpStream,
            )
            .await?;

        let (ups_r, ups_w) = tokio::io::split(tls_stream);

        // add task and user stats
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        wrapper_stats.push_other_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let ups_r = LimitedReader::new_unlimited(ups_r, wrapper_stats.clone() as _);
        let ups_w = LimitedWriter::new_unlimited(ups_w, wrapper_stats as _);

        Ok((Box::new(ups_r), Box::new(ups_w)))
    }
}
