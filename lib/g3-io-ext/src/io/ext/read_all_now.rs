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

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io;
use tokio::io::{AsyncRead, ReadBuf};

pub struct ReadAllNow<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<'a, R> ReadAllNow<'a, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        ReadAllNow { reader, buf }
    }
}

fn read_all_now_internal<R: AsyncRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
) -> Poll<io::Result<usize>> {
    let mut buf = ReadBuf::new(buf);
    loop {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(buf.filled().len()));
        }
        match reader.as_mut().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(_)) => {}
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Ready(Ok(buf.filled().len())),
        }
    }
}

impl<R> Future for ReadAllNow<'_, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ReadAllNow { reader, buf } = &mut *self;
        read_all_now_internal(Pin::new(reader), cx, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed() {
        let mut stream = tokio_test::io::Builder::new().read(&[]).build();
        let mut buf = vec![0u8; 1024];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, 0);
    }

    #[tokio::test]
    async fn read_one() {
        let buf1 = b"123456";
        let mut stream = tokio_test::io::Builder::new().read(buf1).build();
        let mut buf = vec![0u8; 1024];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, buf1.len());
        assert_eq!(&buf[..nr], buf1);
    }

    #[tokio::test]
    async fn read_two() {
        let buf1 = b"123456";
        let buf2 = b"abcdef";
        let mut stream = tokio_test::io::Builder::new().read(buf1).read(buf2).build();
        let mut buf = vec![0u8; 1024];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, buf1.len() + buf2.len());
        assert_eq!(&buf[..buf1.len()], buf1);
        assert_eq!(&buf[buf1.len()..nr], buf2);
    }

    #[tokio::test]
    async fn read_one_frag() {
        let buf1 = b"123456";
        let mut stream = tokio_test::io::Builder::new().read(buf1).build();
        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, 4);
        assert_eq!(&buf[..nr], &buf1[..nr]);

        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, 2);
        assert_eq!(&buf[..nr], &buf1[4..]);
    }

    #[tokio::test]
    async fn read_two_frag() {
        let buf1 = b"123456";
        let buf2 = b"abcdef";
        let mut stream = tokio_test::io::Builder::new().read(buf1).read(buf2).build();
        let mut buf = vec![0u8; 10];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, 10);
        assert_eq!(&buf[..nr], b"123456abcd");

        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf).await.unwrap();
        assert_eq!(nr, 2);
        assert_eq!(&buf[..nr], b"ef");
    }
}