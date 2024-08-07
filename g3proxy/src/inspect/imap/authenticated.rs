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

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_imap_proto::command::{Command, ParsedCommand};
use g3_imap_proto::response::{BadResponse, ByeResponse};
use g3_io_ext::LimitedWriteExt;

use super::{
    CommandLineReceiveExt, ImapInterceptObject, ImapRelayBuf, ResponseAction,
    ResponseLineReceiveExt,
};
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskResult};

enum ClientAction {
    Loop,
    Logout,
    Idle,
    SendLiteral(usize),
}

pub(super) enum ForwardStatus {
    ServerClose,
    ClientClose,
}

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn relay_authenticated<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<ForwardStatus>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            relay_buf.cmd_recv_buf.consume_line();
            relay_buf.rsp_recv_buf.consume_line();

            tokio::select! {
                r = relay_buf.cmd_recv_buf.recv_cmd_line(clt_r) => {
                    let line = r?;
                    if let Some(mut cmd) = self.cmd_pipeline.take_ongoing_command() {
                        self.handle_cmd_continue_line(line, &mut cmd, clt_w, ups_w).await?;
                        if let Some(literal) = cmd.literal_arg {
                            self.cmd_pipeline.set_ongoing_command(cmd);
                            if !literal.wait_continuation {
                                self.relay_client_literal(literal.size, clt_r, ups_w, relay_buf).await?;
                            }
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
                        }
                    } else {
                        match self.handle_authenticated_cmd_line(line, clt_w, ups_w).await? {
                            ClientAction::Logout => {
                                return Ok(ForwardStatus::ClientClose);
                            }
                            ClientAction::Idle => {
                                if let Some(status) = self.relay_until_idle_done(
                                    clt_r,
                                    clt_w,
                                    ups_r,
                                    ups_w,
                                    relay_buf,
                                ).await? {
                                    return Ok(status);
                                }
                            }
                            ClientAction::SendLiteral(size) => {
                                self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                            }
                            ClientAction::Loop => {}
                        }
                    }
                }
                r = relay_buf.rsp_recv_buf.recv_rsp_line(ups_r) => {
                    let line = r?;
                    if let Some(mut rsp) = self.cmd_pipeline.take_ongoing_response() {
                        self.handle_rsp_continue_line(line, &mut rsp, clt_w).await?;
                        if let Some(size) = rsp.literal_data {
                            self.cmd_pipeline.set_ongoing_response(rsp);
                            self.relay_server_literal(size, clt_w, ups_r, relay_buf).await?;
                        }
                    } else {
                        match self.handle_rsp_line(line, clt_w).await? {
                            ResponseAction::Loop => {}
                            ResponseAction::Close => return Ok(ForwardStatus::ServerClose),
                            ResponseAction::SendLiteral(size) => {
                                self.relay_server_literal(size, clt_w, ups_r,  relay_buf).await?;
                            }
                            ResponseAction::RecvClientLiteral(size) => {
                                 self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_authenticated_cmd_line<CW, UW>(
        &mut self,
        line: &[u8],
        clt_w: &mut CW,
        ups_w: &mut UW,
    ) -> ServerTaskResult<ClientAction>
    where
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match Command::parse_line(line) {
            Ok(cmd) => {
                let mut action = ClientAction::Loop;
                match cmd.parsed {
                    ParsedCommand::Capability | ParsedCommand::NoOperation | ParsedCommand::Id => {
                        self.cmd_pipeline.insert_completed(cmd);
                    }
                    ParsedCommand::Logout => {
                        self.cmd_pipeline.insert_completed(cmd);
                        action = ClientAction::Logout;
                    }
                    ParsedCommand::StartTls | ParsedCommand::Auth | ParsedCommand::Login => {
                        BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        return Ok(action);
                    }
                    ParsedCommand::Enable
                    | ParsedCommand::Select
                    | ParsedCommand::Examine
                    | ParsedCommand::Namespace
                    | ParsedCommand::Create
                    | ParsedCommand::Delete
                    | ParsedCommand::Rename
                    | ParsedCommand::Subscribe
                    | ParsedCommand::Unsubscribe
                    | ParsedCommand::List
                    | ParsedCommand::Lsub
                    | ParsedCommand::Status
                    | ParsedCommand::Append => {
                        if let Some(literal) = cmd.literal_arg {
                            if literal.wait_continuation {
                                action = ClientAction::SendLiteral(literal.size);
                            }
                            self.cmd_pipeline.set_ongoing_command(cmd);
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
                        }
                    }
                    ParsedCommand::Idle => {
                        self.cmd_pipeline.set_ongoing_command(cmd);
                        action = ClientAction::Idle;
                    }
                    ParsedCommand::Close
                    | ParsedCommand::Unselect
                    | ParsedCommand::Expunge
                    | ParsedCommand::Search
                    | ParsedCommand::Fetch
                    | ParsedCommand::Store
                    | ParsedCommand::Copy
                    | ParsedCommand::Move
                    | ParsedCommand::Uid => {
                        if !self.mailbox_selected {
                            BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            return Ok(action);
                        }
                        if let Some(literal) = cmd.literal_arg {
                            if literal.wait_continuation {
                                action = ClientAction::SendLiteral(literal.size);
                            }
                            self.cmd_pipeline.set_ongoing_command(cmd);
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
                        }
                    }
                    _ => {
                        BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        return Ok(action);
                    }
                }

                ups_w
                    .write_all_flush(line)
                    .await
                    .map_err(ServerTaskError::UpstreamWriteFailed)?;
                Ok(action)
            }
            Err(e) => {
                let _ = ByeResponse::reply_client_protocol_error(clt_w).await;
                Err(ServerTaskError::ClientAppError(anyhow!(
                    "invalid IMAP command line: {e}"
                )))
            }
        }
    }

    async fn relay_until_idle_done<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<Option<ForwardStatus>>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            relay_buf.cmd_recv_buf.consume_line();
            relay_buf.rsp_recv_buf.consume_line();

            tokio::select! {
                r = relay_buf.cmd_recv_buf.recv_cmd_line(clt_r) => {
                    let line = r?;
                    return if line == b"DONE\r\n" {
                        ups_w.write_all_flush(line)
                            .await
                            .map_err(ServerTaskError::UpstreamWriteFailed)?;
                        Ok(None)
                    } else {
                        let _ = ByeResponse::reply_client_protocol_error(clt_w).await;
                        Err(ServerTaskError::ClientAppError(anyhow!(
                            "invalid IMAP IDLE ending line"
                        )))
                    };
                }
                r = relay_buf.rsp_recv_buf.recv_rsp_line(ups_r) => {
                    let line = r?;
                    if let Some(mut rsp) = self.cmd_pipeline.take_ongoing_response() {
                        self.handle_rsp_continue_line(line, &mut rsp, clt_w).await?;
                        if let Some(size) = rsp.literal_data {
                            self.relay_server_literal(size, clt_w, ups_r, relay_buf).await?;
                        }
                    } else {
                        match self.handle_rsp_line(line, clt_w).await? {
                            ResponseAction::Loop => {}
                            ResponseAction::Close => return Ok(Some(ForwardStatus::ServerClose)),
                            ResponseAction::SendLiteral(size) => {
                                self.relay_server_literal(size, clt_w, ups_r,  relay_buf).await?;
                            }
                            ResponseAction::RecvClientLiteral(size) => {
                                 self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                            }
                        }
                    }
                }
            }
        }
    }
}
