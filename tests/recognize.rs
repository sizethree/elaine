#![cfg(test)]

mod helpers;

use async_std::prelude::*;
use async_std::task::block_on;
use elaine::{recognize, RequestMethod};
use helpers::AsyncBuffer;

fn buffer_from(source: &[u8]) -> AsyncBuffer {
  AsyncBuffer::new(format!(
    "GET /first HTTP/1.1\r\nComplex: {}",
    std::str::from_utf8(source).unwrap()
  ))
}

#[test]
fn test_invalid_header_line() {
  let mut buff = AsyncBuffer::new("FOOBAR\r\nHost: 0.0.0.0:8080\r\n\r\n");
  let result = block_on(async { recognize(&mut buff).await });
  assert!(result.is_err());
}

#[test]
fn test_invalid_method() {
  let mut buff = AsyncBuffer::new("FOOBAR /hello HTTP/1.1\r\nHost: 0.0.0.0:8080\r\n\r\n");
  let result = block_on(async { recognize(&mut buff).await });
  assert!(result.is_err());
}

#[test]
fn test_invalid_version() {
  let mut buff = AsyncBuffer::new("FOOBAR /hello GARBAGE\r\nHost: 0.0.0.0:8080\r\n\r\n");
  let result = block_on(async { recognize(&mut buff).await });
  assert!(result.is_err());
}

#[test]
fn test_with_auth() {
  let mut buff = AsyncBuffer::new("POST / HTTP/1.1\r\nHost: 0.0.0.0:8080\r\n\r\n");
  let result = block_on(async { recognize(&mut buff).await });
  assert!(result.is_ok());
}

#[test]
fn test_recognize_two() {
  let tail: &[u8] = &[0x61, 0x61, 0x61, 0xC9, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = AsyncBuffer::new(format!("GET /first HTTP/1.1\r\n{}", std::str::from_utf8(tail).unwrap()));
  let result = block_on(async { recognize(&mut full).await });
  assert!(result.is_ok())
}

#[test]
fn test_recognize_three() {
  let tail: &[u8] = &[0x61, 0x61, 0xE0, 0xA1, 0x98, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = AsyncBuffer::new(format!("GET /first HTTP/1.1\r\n{}", std::str::from_utf8(tail).unwrap()));
  let result = block_on(async { recognize(&mut full).await });
  assert!(result.is_ok())
}

#[test]
fn test_recognize_full() {
  let tail: &[u8] = &[0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = AsyncBuffer::new(format!("GET /first HTTP/1.1\r\n{}", std::str::from_utf8(tail).unwrap()));
  let result = block_on(async { recognize(&mut full).await });
  assert!(result.is_ok())
}

#[test]
fn test_recognize_four() {
  let tail: &[u8] = &[0x61, 0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = AsyncBuffer::new(format!("GET /first HTTP/1.1\r\n{}", std::str::from_utf8(tail).unwrap()));
  let result = block_on(async { recognize(&mut full).await });
  assert!(result.is_ok())
}

#[test]
fn test_recognize_after() {
  let mut full = AsyncBuffer::new("GET /foo HTTP/1.1\r\nHost: 8080\r\nContent-Length: 3\r\n\r\nhey");
  let result = block_on(async { recognize(&mut full).await });
  assert!(result.is_ok());
  assert_eq!(format!("{}", full), format!("{}", AsyncBuffer::new("hey")));
}

#[test]
fn test_recognize_utf8_boundary_dangle_two() {
  let buf: &[u8] = &[0x61, 0x61, 0x61, 0xE0, 0xA1, 0x98, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  let complex = result.unwrap().find_header("Complex");
  assert_eq!(complex, Some("aaaࡘ".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_dangle_three() {
  let buf: &[u8] = &[0x61, 0x61, 0x61, 0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("aaa𐆒".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_half_debt_one() {
  let buf: &[u8] = &[0x61, 0x61, 0xC9, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("aaɒ".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_half_debt_two() {
  let buf: &[u8] = &[0x61, 0xC9, 0x92, 0x61, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("aɒa".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_half_dangle_one() {
  let buf: &[u8] = &[0x61, 0x61, 0xE0, 0xA1, 0x98, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("aaࡘ".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_half_dangle_three() {
  let buf: &[u8] = &[0x61, 0x61, 0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("aa𐆒".to_string()));
}

#[test]
fn test_recognize_utf8_boundary_half_debt_one_four() {
  let buf: &[u8] = &[0x61, 0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("a𐆒".to_string()));
}

#[test]
fn test_single_char_utf8() {
  let buf: &[u8] = &[0xF0, 0x90, 0x86, 0x92, 0x0d, 0x0a, 0x0d, 0x0a];
  let mut full = buffer_from(buf);
  let result = block_on(async { recognize(&mut full).await });
  assert_eq!(result.unwrap().find_header("Complex"), Some("𐆒".to_string()));
}

#[test]
fn test_recognize_http_example() {
  let mut buffer = AsyncBuffer::new(format!(
    "{}{}{}{}{}{}{}{}{}{}{}{}",
    "GET ", "/hel", "lo-w", "orld", " HTT", "P/1.", "1\r\nC", "onte", "nt-L", "engt", "h: 3", "\r\n\r\n"
  ));
  let result = block_on(async { recognize(&mut buffer).await });
  assert_eq!(result.unwrap().method(), Some(RequestMethod::GET));
}

#[test]
fn recognize_valid_without_body() {
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_ok());
  let head = result.unwrap();
  assert_eq!(head.method(), Some(RequestMethod::GET));
  assert_eq!(head.len(), None);
}

#[test]
fn recognize_valid_with_len() {
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\nContent-Length: 10\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_ok());
  let head = result.unwrap();
  assert_eq!(head.method(), Some(RequestMethod::GET));
  assert_eq!(head.len(), Some(10));
}

#[test]
fn recognize_bad_content_length() {
  let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\nContent-Length: bad\r\n\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_err());
}

#[test]
fn recognize_fail_bad_start() {
  let mut buffer = AsyncBuffer::new("\r\n");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_err());
}

#[test]
fn recognize_and_read_after() {
  let mut buffer = AsyncBuffer::new("POST /create HTTP/1.1\r\nContent-Length: 3\r\n\r\nhey");
  let result = block_on(async { recognize(&mut buffer).await });
  assert!(result.is_ok());
  let mut rem: Vec<u8> = vec![0x00, 0x00, 0x00];
  let result = block_on(async { buffer.read(&mut rem).await });
  assert_eq!(result.unwrap(), 3);
  assert_eq!(String::from_utf8(rem).unwrap(), "hey");
}
