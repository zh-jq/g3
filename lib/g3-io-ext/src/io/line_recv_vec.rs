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

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};

use super::RecvLineError;

pub struct LineRecvVec {
    length: usize,
    line_start: usize,
    line_end: usize,
    buf: Box<[u8]>,
}

impl LineRecvVec {
    pub fn with_capacity(cap: usize) -> Self {
        LineRecvVec {
            length: 0,
            line_start: 0,
            line_end: 0,
            buf: vec![0; cap].into_boxed_slice(),
        }
    }

    pub fn with_data(data: &[u8], cap: usize) -> Self {
        let mut buf = Vec::with_capacity(cap);
        buf.extend_from_slice(data);
        if cap > data.len() {
            buf.resize(cap, 0);
        }
        LineRecvVec {
            length: data.len(),
            line_start: 0,
            line_end: 0,
            buf: buf.into_boxed_slice(),
        }
    }

    pub async fn read_line_with_timeout<R>(
        &mut self,
        reader: &mut R,
        timeout: Duration,
    ) -> Result<&[u8], RecvLineError>
    where
        R: AsyncRead + Unpin,
    {
        tokio::time::timeout(timeout, self.read_line(reader))
            .await
            .map_err(|_| RecvLineError::Timeout)?
    }

    pub async fn read_line<R>(&mut self, reader: &mut R) -> Result<&[u8], RecvLineError>
    where
        R: AsyncRead + Unpin,
    {
        self.read_line_size(reader).await?;
        Ok(&self.buf[self.line_start..self.line_end])
    }

    async fn read_line_size<R>(&mut self, reader: &mut R) -> Result<(), RecvLineError>
    where
        R: AsyncRead + Unpin,
    {
        if let Some(end) = self.get_line() {
            return Ok(end);
        }
        loop {
            let mut unfilled = &mut self.buf[self.length..];
            if unfilled.is_empty() {
                return self.get_line().ok_or(RecvLineError::LineTooLong);
            }
            let nr = reader.read_buf(&mut unfilled).await?;
            if nr == 0 {
                return self.get_line().ok_or(RecvLineError::IoClosed);
            }
            self.length += nr;
            if let Some(line) = self.get_line() {
                return Ok(line);
            }
        }
    }

    fn get_line(&mut self) -> Option<()> {
        if self.line_end > self.line_start {
            return Some(());
        }
        if self.line_start >= self.length {
            return None;
        }
        match memchr::memchr(b'\n', &self.buf[self.line_start..self.length]) {
            Some(p) => {
                self.line_end = self.line_start + p + 1;
                Some(())
            }
            None => None,
        }
    }

    pub fn buffered_line(&mut self) -> Option<&[u8]> {
        self.get_line()?;
        Some(&self.buf[self.line_start..self.line_end])
    }

    pub fn consume_line(&mut self) {
        self.line_start = self.line_end;
        if self.get_line().is_some() {
            return;
        }
        if self.line_start < self.length {
            self.buf.copy_within(self.line_start..self.length, 0);
            self.length -= self.line_start;
        } else {
            self.length = 0;
        }
        self.line_start = 0;
        self.line_end = 0;
    }

    pub fn consume_left(&mut self, max_size: usize) -> &[u8] {
        if self.line_start >= self.length {
            return &[];
        }
        let left = self.length - self.line_start;
        let start = self.line_start;
        if max_size >= left {
            let end = self.length;
            self.length = 0;
            self.line_start = 0;
            self.line_end = 0;
            &self.buf[start..end]
        } else {
            self.line_start += max_size;
            self.line_end = self.line_start;
            &self.buf[start..self.line_start]
        }
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single_line() {
        let data = b"123 test line\r\n";

        let mut reader = tokio_test::io::Builder::new().read(data).build();

        let mut b = LineRecvVec::with_capacity(512);
        let line = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line, data);
        b.consume_line();
        assert!(b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    async fn multiple_line() {
        let data = b"123 test line\r\n456 second line\r\n789";

        let mut reader = tokio_test::io::Builder::new().read(data).build();

        let mut b = LineRecvVec::with_capacity(512);
        let line1 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line1, b"123 test line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let line2 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line2, b"456 second line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    async fn multiple_line2() {
        let data1 = b"123 test line\r\n";
        let data2 = b"456 second ";
        let data3 = b"line\r\n789";

        let mut reader = tokio_test::io::Builder::new()
            .read(data1)
            .read(data2)
            .read(data3)
            .build();

        let mut b = LineRecvVec::with_capacity(512);
        let line1 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line1, b"123 test line\r\n");
        b.consume_line();
        assert!(b.is_empty());

        let line2 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line2, b"456 second line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    #[should_panic]
    async fn too_long_line() {
        let data = b"123 test line\r\n";

        let mut reader = tokio_test::io::Builder::new().read(data).build();

        let mut b = LineRecvVec::with_capacity(12);
        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::LineTooLong)));
        // not all data read, so drop(reader) should panic
    }
}
