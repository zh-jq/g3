/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncWrite, BufWriter};

use g3_http::HttpBodyDecodeReader;
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::{IdleCheck, StreamCopyError};
use g3_smtp_proto::io::TextDataEncodeTransfer;

use super::{SmtpAdaptationError, SmtpMessageAdapter};
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> SmtpMessageAdapter<I> {
    pub(super) async fn handle_icap_http_request_without_body(
        mut self,
        _state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError> {
        let _http_req =
            HttpAdaptedRequest::parse(&mut self.icap_connection.reader, http_header_size, true)
                .await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a message body
        Err(SmtpAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_request_with_body_after_transfer<UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError>
    where
        UW: AsyncWrite + Unpin,
    {
        let _http_req =
            HttpAdaptedRequest::parse(&mut self.icap_connection.reader, http_header_size, true)
                .await?;
        // TODO check request content type?

        let mut body_reader =
            HttpBodyDecodeReader::new_chunked(&mut self.icap_connection.reader, 256);
        let mut ups_buf_writer = BufWriter::new(ups_writer);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut body_reader, &mut ups_buf_writer, self.copy_config);

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut msg_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            if body_reader.trailer(128).await.is_ok() {
                                self.icap_connection.mark_reader_finished();
                                if icap_rsp.keep_alive {
                                    self.icap_client.save_connection(self.icap_connection);
                                }
                            }
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                        },
                        Err(StreamCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::IcapServerReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::SmtpUpstreamWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if msg_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if msg_transfer.no_cached_data() {
                                Err(SmtpAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(SmtpAdaptationError::SmtpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        msg_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(SmtpAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
