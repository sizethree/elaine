#![feature(test)]
#![cfg(test)]

extern crate test;

use async_std::task::block_on;
use test::Bencher;

#[bench]
fn bench_recognize_happy(bencher: &mut Bencher) {
  bencher.iter(|| block_on(async { format!("whoa") }));
}
