/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, Write};
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::{BufMut, Bytes};
use h2::RecvStream;
use thiserror::Error;
use tokio::io::AsyncWrite;

#[derive(Debug, Error)]
pub enum H2StreamToChunkedTransferError {
    #[error("write error: {0:?}")]
    WriteError(io::Error),
    #[error("recv data failed: {0}")]
    RecvDataFailed(h2::Error),
    #[error("recv trailer failed: {0}")]
    RecvTrailerFailed(h2::Error),
}

#[derive(PartialEq, Eq)]
enum TransferStage {
    End,
    Data,
    Trailer,
}

struct ChunkedEncodeTransferInternal {
    yield_size: usize,
    this_chunk_size: usize,
    chunk: Option<Bytes>,
    static_header: Vec<u8>,
    static_offset: usize,
    total_write: u64,
    read_data_finished: bool,
    active: bool,
    trailer_bytes: Vec<u8>,
    trailer_offset: usize,
    transfer_stage: TransferStage,
}

impl ChunkedEncodeTransferInternal {
    fn new(yield_size: usize) -> Self {
        ChunkedEncodeTransferInternal {
            yield_size,
            this_chunk_size: 0,
            chunk: None,
            static_header: Vec::with_capacity(16),
            static_offset: 0,
            total_write: 0,
            read_data_finished: false,
            active: false,
            trailer_bytes: Vec::new(),
            trailer_offset: 0,
            transfer_stage: TransferStage::Data,
        }
    }

    fn with_chunk(yield_size: usize, chunk: Bytes) -> Self {
        let mut static_header = Vec::with_capacity(16);
        let _ = write!(&mut static_header, "{:x}\r\n", chunk.len());
        ChunkedEncodeTransferInternal {
            yield_size,
            this_chunk_size: chunk.len(),
            chunk: Some(chunk),
            static_header,
            static_offset: 0,
            total_write: 0,
            read_data_finished: false,
            active: false,
            trailer_bytes: Vec::new(),
            trailer_offset: 0,
            transfer_stage: TransferStage::Data,
        }
    }

    fn without_data() -> Self {
        ChunkedEncodeTransferInternal {
            yield_size: 0,
            this_chunk_size: 0,
            chunk: None,
            static_header: Vec::new(),
            static_offset: 0,
            total_write: 0,
            read_data_finished: true,
            active: false,
            trailer_bytes: Vec::new(),
            trailer_offset: 0,
            transfer_stage: TransferStage::Trailer,
        }
    }

    #[inline]
    fn finished(&self) -> bool {
        self.transfer_stage == TransferStage::End
    }

    #[inline]
    fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    fn is_active(&self) -> bool {
        self.active
    }

    fn reset_active(&mut self) {
        self.active = false;
    }

    fn no_cached_data(&self) -> bool {
        match self.transfer_stage {
            TransferStage::Data => {
                self.static_offset >= self.static_header.len() && self.chunk.is_none()
            }
            TransferStage::Trailer => self.trailer_offset >= self.trailer_bytes.len(),
            TransferStage::End => true,
        }
    }

    fn poll_transfer_trailers<W>(
        &mut self,
        cx: &mut Context<'_>,
        recv_stream: &mut RecvStream,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<u64, H2StreamToChunkedTransferError>>
    where
        W: AsyncWrite + Unpin,
    {
        if !self.trailer_bytes.is_empty() {
            while self.trailer_offset < self.trailer_bytes.len() {
                let nw = ready!(
                    writer
                        .as_mut()
                        .poll_write(cx, &self.trailer_bytes[self.trailer_offset..])
                )
                .map_err(H2StreamToChunkedTransferError::WriteError)?;
                self.active = true;
                self.trailer_offset += nw;
                self.total_write += nw as u64;
            }
            self.transfer_stage = TransferStage::End;
            self.poll_transfer(cx, recv_stream, writer)
        } else {
            match ready!(recv_stream.poll_trailers(cx))
                .map_err(H2StreamToChunkedTransferError::RecvTrailerFailed)?
            {
                Some(trailer) => {
                    self.active = true;
                    let mut buf = Vec::with_capacity(128);
                    for (name, value) in trailer.iter() {
                        buf.put_slice(name.as_str().as_bytes());
                        buf.put_slice(b": ");
                        buf.put_slice(value.as_bytes());
                        buf.put_slice(b"\r\n");
                    }
                    buf.put_slice(b"\r\n");
                    self.trailer_bytes = buf;
                    self.poll_transfer_trailers(cx, recv_stream, writer)
                }
                None => {
                    self.active = true;
                    let buf = vec![b'\r', b'\n'];
                    self.trailer_bytes = buf;
                    self.poll_transfer_trailers(cx, recv_stream, writer)
                }
            }
        }
    }

    fn poll_transfer_data<W>(
        &mut self,
        cx: &mut Context<'_>,
        recv_stream: &mut RecvStream,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<u64, H2StreamToChunkedTransferError>>
    where
        W: AsyncWrite + Unpin,
    {
        let mut copy_this_round = 0usize;

        loop {
            if self.chunk.is_none() && !self.read_data_finished {
                match ready!(recv_stream.poll_data(cx)) {
                    Some(Ok(chunk)) => {
                        self.active = true;
                        if chunk.is_empty() {
                            continue;
                        }
                        let nr = chunk.len();
                        recv_stream
                            .flow_control()
                            .release_capacity(nr)
                            .map_err(H2StreamToChunkedTransferError::RecvDataFailed)?;
                        self.static_header.clear();
                        if self.total_write == 0 {
                            let _ = write!(&mut self.static_header, "{nr:x}\r\n");
                        } else {
                            let _ = write!(&mut self.static_header, "\r\n{nr:x}\r\n");
                        }
                        self.static_offset = 0;
                        self.this_chunk_size = nr;
                        self.chunk = Some(chunk);
                    }
                    Some(Err(e)) => {
                        return Poll::Ready(Err(H2StreamToChunkedTransferError::RecvDataFailed(e)));
                    }
                    None => {
                        self.read_data_finished = true;
                        self.active = true;
                        self.static_header.clear();
                        if self.total_write == 0 {
                            self.static_header.extend_from_slice(b"0\r\n");
                        } else {
                            self.static_header.extend_from_slice(b"\r\n0\r\n");
                        }
                        self.static_offset = 0;
                        self.this_chunk_size = 0;
                    }
                }
            }

            while self.static_offset < self.static_header.len() {
                let nw = ready!(
                    writer
                        .as_mut()
                        .poll_write(cx, &self.static_header[self.static_offset..])
                )
                .map_err(H2StreamToChunkedTransferError::WriteError)?;
                self.active = true;
                self.static_offset += nw;
                self.total_write += nw as u64;
            }
            if self.read_data_finished {
                self.transfer_stage = TransferStage::Trailer;
                return self.poll_transfer(cx, recv_stream, writer);
            }

            while let Some(mut chunk) = self.chunk.take() {
                match writer.as_mut().poll_write(cx, &chunk) {
                    Poll::Ready(Ok(nw)) => {
                        let left_chunk = chunk.split_off(nw);
                        self.total_write += nw as u64;
                        copy_this_round += nw;
                        self.active = true;
                        if left_chunk.is_empty() {
                            break;
                        } else {
                            self.chunk = Some(left_chunk);
                        }
                    }
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(H2StreamToChunkedTransferError::WriteError(e)));
                    }
                    Poll::Pending => {
                        self.chunk = Some(chunk);
                        return Poll::Pending;
                    }
                }
            }

            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
    }

    fn poll_transfer<W>(
        &mut self,
        cx: &mut Context<'_>,
        recv_stream: &mut RecvStream,
        writer: Pin<&mut W>,
    ) -> Poll<Result<u64, H2StreamToChunkedTransferError>>
    where
        W: AsyncWrite + Unpin,
    {
        match self.transfer_stage {
            TransferStage::Data => self.poll_transfer_data(cx, recv_stream, writer),
            TransferStage::Trailer => self.poll_transfer_trailers(cx, recv_stream, writer),
            TransferStage::End => {
                ready!(writer.poll_flush(cx))
                    .map_err(H2StreamToChunkedTransferError::WriteError)?;
                Poll::Ready(Ok(self.total_write))
            }
        }
    }
}

pub struct H2StreamToChunkedTransfer<'a, W> {
    recv_stream: &'a mut RecvStream,
    writer: &'a mut W,
    internal: ChunkedEncodeTransferInternal,
}

impl<'a, W> H2StreamToChunkedTransfer<'a, W> {
    pub fn new(recv_stream: &'a mut RecvStream, writer: &'a mut W, yield_size: usize) -> Self {
        H2StreamToChunkedTransfer {
            recv_stream,
            writer,
            internal: ChunkedEncodeTransferInternal::new(yield_size),
        }
    }

    pub fn with_chunk(
        recv_stream: &'a mut RecvStream,
        writer: &'a mut W,
        yield_size: usize,
        chunk: Bytes,
    ) -> Self {
        H2StreamToChunkedTransfer {
            recv_stream,
            writer,
            internal: ChunkedEncodeTransferInternal::with_chunk(yield_size, chunk),
        }
    }

    pub fn without_data(recv_stream: &'a mut RecvStream, writer: &'a mut W) -> Self {
        H2StreamToChunkedTransfer {
            recv_stream,
            writer,
            internal: ChunkedEncodeTransferInternal::without_data(),
        }
    }

    pub fn finished(&self) -> bool {
        self.internal.finished()
    }

    pub fn is_idle(&self) -> bool {
        self.internal.is_idle()
    }

    pub fn is_active(&self) -> bool {
        self.internal.is_active()
    }

    pub fn reset_active(&mut self) {
        self.internal.reset_active()
    }

    pub fn no_cached_data(&self) -> bool {
        self.internal.no_cached_data()
    }
}

impl<W> Future for H2StreamToChunkedTransfer<'_, W>
where
    W: AsyncWrite + Unpin,
{
    type Output = Result<u64, H2StreamToChunkedTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.internal
            .poll_transfer(cx, me.recv_stream, Pin::new(&mut me.writer))
    }
}
