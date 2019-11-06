#![cfg(test)]

mod helpers;

use async_std::task::block_on;
use elaine::{recog, recognize};
use helpers::AsyncBuffer;

#[test]
fn test_recog_single_block() {
  let mut buffer = AsyncBuffer::new(format!("{}{}", "AAAA", "\r\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAA"]);
}

#[test]
fn test_recog_single_dangle_one() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}", "AAAA", "A\r\n\r", "\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAAA"]);
}

#[test]
fn test_recog_single_dangle_two() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}", "AAAA", "AA\r\n", "\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAAAA"]);
}

#[test]
fn test_recog_single_dangle_three() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}", "AAAA", "AAA\r", "\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAAAAA"]);
}

#[test]
fn test_recog_multi_block_start() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}", "AAAA", "\r\nBB", "\r\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAA", "BB"]);
}

#[test]
fn test_recog_multi_block_start_dangle_one() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AAAA", "\r\nBB", "B\r\n\r", "\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAA", "BBB"]);
}

#[test]
fn test_recog_multi_block_start_dangle_two() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AAAA", "\r\nBB", "BB\r\n", "\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAA", "BBBB"]);
}

#[test]
fn test_recog_multi_block_start_dangle_three() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AAAA", "\r\nBB", "BBB\r", "\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AAAA", "BBBBB"]);
}

#[test]
fn test_recog_multi_block_end() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}", "AA\r\n", "BBBB", "\r\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AA", "BBBB"]);
}

#[test]
fn test_recog_multi_block_end_dangle_one() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AA\r\n", "BBBB", "B\r\n\r", "\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AA", "BBBBB"]);
}

#[test]
fn test_recog_multi_block_end_dangle_two() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AA\r\n", "BBBB", "BB\r\n", "\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AA", "BBBBBB"]);
}

#[test]
fn test_recog_multi_block_end_dangle_three() {
  let mut buffer = AsyncBuffer::new(format!("{}{}{}{}", "AA\r\n", "BBBB", "BBB\r", "\n\r\n"));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["AA", "BBBBBBB"]);
}

#[test]
fn test_recog_http_example() {
  let mut buffer = AsyncBuffer::new(format!(
    "{}{}{}{}{}{}{}{}{}{}{}{}",
    "GET ", "/hel", "lo-w", "orld", " HTT", "P/1.", "1\r\nC", "onte", "nt-L", "engt", "h: 3", "\r\n\r\n"
  ));
  let result = block_on(async { recog(&mut buffer).await });
  assert_eq!(result.unwrap(), vec!["GET /hello-world HTTP/1.1", "Content-Length: 3"]);
}

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
