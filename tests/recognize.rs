#![cfg(test)]

mod helpers;

use async_std::task::block_on;
use elaine::recognize;
use helpers::AsyncBuffer;

#[test]
fn recognize_valid_without_body() {
  println!("hello world");
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_ok());
  let head = result.unwrap();
  assert_eq!(head.method(), Some(String::from("GET")));
  assert_eq!(head.len(), None);
}

#[test]
fn recognize_valid_with_len() {
  println!("hello world");
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\nContent-Length: 10\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_ok());
  let head = result.unwrap();
  assert_eq!(head.method(), Some(String::from("GET")));
  assert_eq!(head.len(), Some(10));
}

#[test]
fn recognize_bad_content_length() {
  println!("hello world");
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\nContent-Length: bad\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_err());
}

#[test]
fn recognize_fail_bad_start() {
  println!("hello world");
  let mut buffer = AsyncBuffer::new("\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_err());
}
