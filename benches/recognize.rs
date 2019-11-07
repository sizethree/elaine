#![feature(test)]
#![cfg(test)]

extern crate test;

#[path = "../tests/helpers/mod.rs"]
mod helpers;

use async_std::task::block_on;
use elaine::{recognize, Head};
use helpers::AsyncBuffer;
use test::Bencher;

async fn run(mut buffer: AsyncBuffer) -> Result<Head, std::io::Error> {
  recognize(&mut buffer).await
}

#[bench]
fn recognize_content(bencher: &mut Bencher) {
  bencher.iter(|| {
    let buff = AsyncBuffer::new("GET /hello-world HTTP/1.1\r\nContent-Length: 3\r\n\r\n");
    let result = block_on(run(buff));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), Some(3));
  })
}
