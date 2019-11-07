#![cfg(test)]

use async_std::io::Read;
use async_std::task::{Context, Poll};
use std::collections::VecDeque;
use std::io::Error;
use std::pin::Pin;

pub struct AsyncBuffer {
  source: VecDeque<u8>,
}

impl AsyncBuffer {
  pub fn new<S>(inner: S) -> Self
  where
    S: Into<String>,
  {
    AsyncBuffer {
      source: VecDeque::from(inner.into().as_bytes().to_vec()),
    }
  }
}

impl std::fmt::Display for AsyncBuffer {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(
      formatter,
      "contents: {:?}",
      String::from_utf8(self.source.clone().into())
    )
  }
}

impl Read for AsyncBuffer {
  fn poll_read(mut self: Pin<&mut Self>, _cx: &mut Context, dest: &mut [u8]) -> Poll<Result<usize, Error>> {
    let mut written = 0;

    for b in &mut *dest {
      match self.source.pop_front() {
        Some(byte) => {
          *b = byte;
          written += 1;
        }
        None => break,
      }
    }

    Poll::Ready(Ok(written))
  }
}
