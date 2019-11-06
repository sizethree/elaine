#![cfg(test)]

use async_std::io::Read;
use async_std::task::{Context, Poll};
use std::io::Error;
use std::pin::Pin;

pub struct AsyncBuffer {
  source: String,
}

impl AsyncBuffer {
  pub fn new<S>(inner: S) -> Self
  where
    S: Into<String>,
  {
    AsyncBuffer { source: inner.into() }
  }
}

impl std::fmt::Display for AsyncBuffer {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(formatter, "contents: {}", self.source)
  }
}

impl Read for AsyncBuffer {
  fn poll_read(mut self: Pin<&mut Self>, _cx: &mut Context, dest: &mut [u8]) -> Poll<Result<usize, Error>> {
    let mut written = 0;

    for b in &mut *dest {
      let (start, end) = self.source.split_at(1);

      if start.len() == 0 {
        break;
      }

      let byte = start.bytes().nth(0);

      match byte {
        Some(byte) => {
          self.source = String::from(end);
          *b = byte as u8;
          written += 1;
        }
        None => break,
      }
    }

    Poll::Ready(Ok(written))
  }
}
