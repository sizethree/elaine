extern crate async_std;

use async_std::io::Read;
use async_std::prelude::*;
use std::io::ErrorKind::UnexpectedEof;
use std::io::{Error, ErrorKind};

const UNEXPECTED_END: &'static str = "Unable to parse understandable HTTP message before maximum bytes read.";

#[derive(PartialEq, Clone, Default, Debug)]
struct Cursor {
  first: Option<()>,
  second: Option<()>,
  count: u8,
  current: u8,
}

#[derive(Debug, Default)]
pub struct RequestLine {
  method: String,
  path: String,
  version: String,
}

#[derive(Debug, Default)]
pub struct Head {
  headers: Vec<Header>,
  req: Option<RequestLine>,
  _len: Option<usize>,
  _auth: Option<String>,
}

impl Head {
  pub fn path(&self) -> Option<String> {
    self.req.as_ref().map(|r| r.path.clone())
  }

  pub fn version(&self) -> Option<String> {
    self.req.as_ref().map(|r| r.version.clone())
  }

  pub fn method(&self) -> Option<String> {
    self.req.as_ref().map(|r| r.method.clone())
  }

  pub fn len(&self) -> Option<usize> {
    self._len
  }
}

#[derive(Debug)]
struct Header(String, String);

impl std::fmt::Display for Head {
  fn fmt(&self, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(
      formatter,
      "{} {} {}\r\nContent-Length: {:?}\r\n\r\n",
      self.method().unwrap_or(String::from("<UNKNOWN>")),
      self.path().unwrap_or(String::from("<UNKNOWN>")),
      self.version().unwrap_or(String::from("<UNKNOWN>")),
      self._len,
    )
  }
}

fn parse_request_line(input: String) -> Result<RequestLine, Error> {
  let mut splits = input.splitn(3, ' ');
  match (splits.next(), splits.next(), splits.next()) {
    (Some(method), Some(uri), Some(version)) => Ok(RequestLine {
      method: String::from(method),
      path: String::from(uri),
      version: String::from(version),
    }),
    _ => Err(Error::new(
      ErrorKind::InvalidData,
      format!("Invalid request line: '{}'", input),
    )),
  }
}

fn normalize_err<E, M>(e: E, message: M) -> Error
where
  E: std::error::Error,
  M: std::fmt::Display,
{
  Error::new(ErrorKind::Other, format!("{}: {}", message, e))
}

pub async fn recognize<R>(reader: &mut R) -> Result<Head, Error>
where
  R: Read + std::marker::Unpin,
{
  let mut current = Cursor::default();

  let mut result = reader.bytes().take(2048).map(|b| match b {
    Ok(byte) => {
      match byte {
        b'\r' => {
          current = Cursor {
            first: Some(()),
            current: byte,
            ..current
          };
        }
        b'\n' => {
          current = Cursor {
            second: Some(()),
            current: byte,
            count: current.count + 1,
            ..current
          };
        }
        _ => {
          current = Cursor {
            current: byte,
            count: 0,
            first: None,
            second: None,
          };
        }
      }
      Ok(current.clone())
    }
    Err(e) => Err(e),
  });

  let mut head = Head::default();
  let mut position = 0u32;

  loop {
    match result.next().await {
      // Terminal: CR LF consecutive
      Some(Ok(Cursor {
        second: Some(_),
        first: Some(_),
        count: 2,
        current: _,
      })) => {
        break;
      }
      // Non-Terminal: CR LF
      Some(Ok(Cursor {
        second: Some(_),
        first: Some(_),
        current: _,
        count: _,
      })) => {
        position = position + 1;
        match head.req {
          None => {
            let req = head.headers.pop().map_or_else(
              || Err(Error::new(ErrorKind::Other, "Request line not provided")),
              |Header(v, _)| parse_request_line(v).map(|v| Some(v)),
            )?;
            head = Head { req, ..head };
          }
          Some(_) => match head.headers.last() {
            Some(Header(key, value)) if key.starts_with("Content-Length: ") => {
              let len = Some(
                value
                  .parse::<usize>()
                  .map_err(|e| normalize_err(e, "Invalid content length"))?,
              );
              head = Head { _len: len, ..head };
              head.headers.push(Header(String::from(""), String::from("")));
            }
            _ => {
              head.headers.push(Header(String::from(""), String::from("")));
            }
          },
        }
      }
      // Non-Terminal: Random byte
      Some(Ok(Cursor {
        second: None,
        first: None,
        count: _,
        current: byte,
      })) => {
        position = position + 1;
        match head.headers.last_mut() {
          Some(Header(start, end)) => {
            if let Some(_) = head.req {
              if start.ends_with(": ") {
                end.push(byte as char);
                continue;
              }

              start.push(byte as char);
              continue;
            }

            if position >= 1024 {
              return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unable to parse request line after {} bytes", position),
              ));
            }

            start.push(byte as char);
          }
          None => {
            let key = String::from_utf8([byte].to_vec()).map_err(|e| normalize_err(e, "Invalid utf8 byte found"))?;
            head.headers.push(Header(key, String::from("")));
            continue;
          }
        }
      }
      // Error: Lost position
      Some(Ok(Cursor {
        second: _,
        first: _,
        count: _,
        current: _,
      })) => continue,
      _ => {
        let err = std::io::Error::new(UnexpectedEof, UNEXPECTED_END);
        return Err(err);
      }
    }
  }

  Ok(head)
}

#[cfg(test)]
mod test_helpers {
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
}

#[cfg(test)]
mod tests {
  use super::recognize;
  use super::test_helpers::AsyncBuffer;
  use async_std::task::block_on;

  #[test]
  fn recognize_valid_without_body() {
    println!("hello world");
    let mut buffer = AsyncBuffer::new("GET /foobar HTTP/1.0\r\n\r\n");
    let result = block_on(async { recognize(&mut buffer).await });
    assert!(result.is_ok());
  }

  #[test]
  fn recognize_fail_bad_start() {
    println!("hello world");
    let mut buffer = AsyncBuffer::new("\r\n");
    let result = block_on(async { recognize(&mut buffer).await });
    assert!(result.is_err());
  }
}
