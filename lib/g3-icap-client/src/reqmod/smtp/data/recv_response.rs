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

use g3_io_ext::IdleCheck;

use super::{HttpAdapterErrorResponse, SmtpAdaptationError, SmtpMessageAdapter};
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodRecvHttpResponseBody};
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> SmtpMessageAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload(
        self,
        icap_rsp: ReqmodResponse,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a payload
        Err(SmtpAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_response_with_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<(HttpAdapterErrorResponse, ReqmodRecvHttpResponseBody), SmtpAdaptationError> {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.reader, http_header_size)
                .await?;
        let recv_body = ReqmodRecvHttpResponseBody {
            icap_client: self.icap_client,
            icap_keepalive: icap_rsp.keep_alive,
            icap_connection: self.icap_connection,
        };
        Ok((http_rsp, recv_body))
    }

    pub(super) async fn handle_icap_http_response_without_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<HttpAdapterErrorResponse, SmtpAdaptationError> {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.reader, http_header_size)
                .await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        Ok(http_rsp)
    }
}
