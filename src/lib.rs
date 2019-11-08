//! This crate provides a lightweight and potentially incomplete http head parser implementation
//! for async-std readers.
//!
//! ## Example
//!
//! ```rust
//! use std::boxed::Box;
//! use std::error::Error;
//!
//! use elaine::{recognize, RequestMethod};
//! use async_std::task::block_on;
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!   block_on(async {
//!     let mut req: &[u8] = b"GET /elaine HTTP/1.1\r\nContent-Length: 3\r\n\r\nhey";
//!     let result = recognize(&mut req).await.unwrap();
//!     assert_eq!(result.method(), Some(RequestMethod::GET));
//!     assert_eq!(result.len(), Some(3));
//!     assert_eq!(std::str::from_utf8(req), Ok("hey"));
//!   });
//!   Ok(())
//! }
//! ```
extern crate async_std;

use async_std::io::Read;
use async_std::prelude::*;
use std::io::{Error, ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestVersion {
  RFC2616,
  RFC1945,
}

impl RequestVersion {
  pub fn parse<S>(input: S) -> Result<Self, Error>
  where
    S: std::fmt::Display,
  {
    match format!("{}", input).as_str() {
      "HTTP/1.1" => Ok(RequestVersion::RFC2616),
      "HTTP/1.0" => Ok(RequestVersion::RFC1945),
      _ => Err(Error::new(
        ErrorKind::InvalidData,
        format!("Unmatched http version: {}", input),
      )),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestMethod {
  CONNECT,
  DELETE,
  GET,
  HEAD,
  OPTIONS,
  POST,
  PUT,
  PATCH,
  TRACE,
}

#[derive(Debug)]
pub struct RequestLine {
  method: RequestMethod,
  path: String,
  version: RequestVersion,
}

#[derive(Debug, Default)]
pub struct Head {
  _headers: Vec<Header>,
  _req: Option<RequestLine>,
  _len: Option<usize>,
  _auth: Option<String>,
}

impl Head {
  pub fn path(&self) -> Option<String> {
    self._req.as_ref().map(|r| r.path.clone())
  }

  pub fn version(&self) -> Option<RequestVersion> {
    self._req.as_ref().map(|r| r.version)
  }

  pub fn method(&self) -> Option<RequestMethod> {
    self._req.as_ref().map(|r| r.method.clone())
  }

  pub fn len(&self) -> Option<usize> {
    self._len
  }

  pub fn find_header<S>(&self, target: S) -> Option<String>
  where
    S: std::fmt::Display,
  {
    self
      ._headers
      .iter()
      .filter_map(|Header(key, value)| {
        if key.as_str() == format!("{}", target).as_str() {
          Some(value.clone())
        } else {
          None
        }
      })
      .nth(0)
  }

  fn add_header(&mut self, header: Header) -> Result<(), Error> {
    let Header(key, value) = header;

    if key == "Content-Length" {
      match value.parse::<usize>() {
        Ok(value) => self._len = Some(value),
        Err(e) => {
          return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid content length ('{}'): {:?}", value, e),
          ))
        }
      }
    }

    self._headers.push(Header(key, value));
    Ok(())
  }
}

#[derive(Debug)]
struct Header(String, String);

impl std::fmt::Display for Head {
  fn fmt(&self, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(
      formatter,
      "{:?} {} {:?}\r\nContent-Length: {:?}\r\n\r\n",
      self.method(),
      self.path().unwrap_or(String::from("<UNKNOWN>")),
      self.version(),
      self._len,
    )
  }
}

fn parse_header_line(input: String) -> Option<Header> {
  let mut splits = input.splitn(2, ": ");
  if let (Some(key), Some(value)) = (splits.next(), splits.next()) {
    return Some(Header(key.to_string(), value.to_string()));
  }
  None
}

fn parse_request_line(input: String) -> Result<RequestLine, Error> {
  let mut splits = input.splitn(3, ' ');
  match (splits.next(), splits.next(), splits.next()) {
    (Some(first), Some(uri), Some(tail)) => {
      let method = match first {
        "CONNECT" => RequestMethod::CONNECT,
        "DELETE" => RequestMethod::DELETE,
        "GET" => RequestMethod::GET,
        "HEAD" => RequestMethod::HEAD,
        "OPTIONS" => RequestMethod::OPTIONS,
        "POST" => RequestMethod::POST,
        "PUT" => RequestMethod::PUT,
        "PATCH" => RequestMethod::PATCH,
        "TRACE" => RequestMethod::TRACE,
        _ => {
          return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Unable to parse request method {}", first),
          ))
        }
      };

      let version = RequestVersion::parse(tail)?;

      Ok(RequestLine {
        method,
        version,
        path: String::from(uri),
      })
    }
    _ => Err(Error::new(
      ErrorKind::InvalidData,
      format!("Invalid request line: '{}'", input),
    )),
  }
}

#[derive(Debug)]
enum Capacity {
  One,
  Two,
  Three,
  Four,
}

#[derive(Debug)]
struct Marker {
  capacity: Capacity,
}

impl Default for Marker {
  fn default() -> Marker {
    Marker {
      capacity: Capacity::Four,
    }
  }
}

async fn fill_utf8<R>(original: &[u8], reader: R) -> Result<String, Error>
where
  R: Read + std::marker::Unpin,
{
  match std::str::from_utf8(original) {
    Ok(c) => Ok(String::from(c)),
    Err(e) => match e.error_len() {
      None => {
        let (valid, after_valid) = original.split_at(e.valid_up_to());
        let stage = std::str::from_utf8(valid).map_err(|e| Error::new(ErrorKind::InvalidData, format!("{:?}", e)))?;
        match after_valid {
          [first, second, third] => {
            let mut bytes = reader.bytes();
            let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
            match std::str::from_utf8(&[*first, *second, *third, fourth]) {
              Ok(rest) => Ok(format!("{}{}", stage, rest)),
              Err(e) => {
                return Err(Error::new(
                  ErrorKind::Other,
                  format!("Failed utf8 decode after third byte: {:?}", e),
                ));
              }
            }
          }
          [first, second] => {
            let mut bytes = reader.bytes();
            let third = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
            match std::str::from_utf8(&[*first, *second, third]) {
              Ok(rest) => Ok(format!("{}{}", stage, rest)),
              Err(_e) => {
                let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
                match std::str::from_utf8(&[*first, *second, third, fourth]) {
                  Ok(rest) => Ok(format!("{}{}", stage, rest)),
                  Err(e) => {
                    return Err(Error::new(
                      ErrorKind::Other,
                      format!("Failed utf8 decode after third byte: {:?}", e),
                    ));
                  }
                }
              }
            }
          }
          [single] => {
            let mut bytes = reader.bytes();
            let second = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
            match std::str::from_utf8(&[*single, second]) {
              Ok(rest) => Ok(format!("{}{}", stage, rest)),
              Err(e) => match e.error_len() {
                None => {
                  let third = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;

                  match std::str::from_utf8(&[*single, second, third]) {
                    Ok(rest) => Ok(format!("{}{}", stage, rest)),
                    Err(_e) => {
                      let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
                      match std::str::from_utf8(&[*single, second, third, fourth]) {
                        Ok(rest) => Ok(format!("{}{}", stage, rest)),
                        Err(e) => {
                          return Err(Error::new(
                            ErrorKind::Other,
                            format!("Failed utf8 decode after third byte: {:?}", e),
                          ));
                        }
                      }
                    }
                  }
                }
                Some(_) => {
                  return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Failed pulling second byte for utf-8 sequence",
                  ));
                }
              },
            }
          }
          _ => {
            return Err(Error::new(
              ErrorKind::InvalidData,
              "Unable to determine correction for utf-8 boundary",
            ))
          }
        }
      }
      Some(_) => {
        return Err(Error::new(
          ErrorKind::InvalidData,
          format!("Invalid utf-8 sequence: {:?}", e),
        ));
      }
    },
  }
}

pub async fn recognize<R>(mut reader: &mut R) -> Result<Head, Error>
where
  R: Read + std::marker::Unpin,
{
  let mut marker = Marker::default();
  let mut headers: Vec<String> = Vec::new();
  let mut head = Head::default();

  loop {
    let mut buf: Vec<u8> = match &marker.capacity {
      Capacity::Four => vec![0x13, 0x10, 0x13, 0x10],
      Capacity::Three => vec![0x10, 0x13, 0x10],
      Capacity::Two => vec![0x13, 0x10],
      Capacity::One => vec![0x10],
    };

    let size = reader.read(&mut buf).await?;
    let chunk = fill_utf8(&buf[0..size], &mut reader).await?;

    let mut chars = chunk.chars();
    let lc = headers.len();

    match (&marker.capacity, chars.next(), chars.next(), chars.next(), chars.next()) {
      // clean terminal
      (_, Some('\r'), Some('\n'), Some('\r'), Some('\n')) => break,
      // terminal from previous '\r\n\r'
      (Capacity::One, Some('\n'), _, _, _) => break,
      // terminal from previous '\r\n'
      (Capacity::Two, Some('\r'), Some('\n'), _, _) => break,
      // non-terminal: had a cr lf but now working with something else
      (Capacity::Two, Some(one), Some(two), _, _) => {
        headers.push([one, two].iter().collect::<String>());
        marker.capacity = Capacity::Four;
      }
      // terminal from previous '\r'
      (Capacity::Three, Some('\n'), Some('\r'), Some('\n'), _) => break,
      (Capacity::Three, Some('\n'), Some(one), None, None) => {
        headers.push(format!("{}", one));
        marker.capacity = Capacity::Four;
      }
      (Capacity::Three, Some('\n'), Some(one), Some(two), None) => {
        headers.push(format!("{}{}", one, two));
        marker.capacity = Capacity::Four;
      }

      // any char followed by '\r\n\r' - queue up single read
      (_, Some(one), Some('\r'), Some('\n'), Some('\r')) => {
        match headers.last_mut() {
          Some(header) => header.push(one),
          None => headers.push(one.to_string()),
        }
        marker.capacity = Capacity::One;
      }

      // any chars followed by '\r\n' - queue up double read
      (_, Some(one), Some(two), Some('\r'), Some('\n')) => {
        let mem = format!("{}{}", one, two);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Two;
      }

      // any chars followed by '\r' - queue up triple read
      (_, Some(one), Some(two), Some(three), Some('\r')) => {
        let mem = format!("{}{}{}", one, two, three);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Three;
      }

      (_, Some(one), Some('\r'), None, None) => {
        match headers.last_mut() {
          Some(header) => {
            header.push(one);
          }
          None => headers.push([one].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Three;
      }

      (_, Some(one), Some(two), Some('\r'), None) => {
        let mem = format!("{}{}", one, two);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Three;
      }

      (_, Some('\r'), Some('\n'), Some(one), Some(two)) => {
        headers.push([one, two].iter().collect::<String>());
        marker.capacity = Capacity::Four;
      }

      (_, Some(one), Some('\r'), Some('\n'), Some(two)) => {
        match headers.last_mut() {
          Some(header) => {
            header.push(one);
          }
          None => headers.push(one.to_string()),
        }
        headers.push(two.to_string());
        marker.capacity = Capacity::Four;
      }

      (_, Some(one), Some(two), Some(three), Some(four)) => {
        let mem = format!("{}{}{}{}", one, two, three, four);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), Some(three), None) => {
        let mem = format!("{}{}{}", one, two, three);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), None, None) => {
        let mem = format!("{}{}", one, two);
        match headers.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => headers.push(mem),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), None, None, None) => {
        match headers.last_mut() {
          Some(header) => header.push(one),
          None => headers.push(format!("{}", one)),
        }
        marker.capacity = Capacity::Four;
      }
      _ => return Err(Error::new(ErrorKind::Other, "Invalid sequence")),
    }

    if headers.len() >= 2 && lc != headers.len() {
      if let (None, Some(line)) = (&head._req, headers.get(0)) {
        head._req = Some(parse_request_line(line.to_string())?)
      }

      if headers.len() == 3 {
        let temp = headers.pop().unwrap_or_default();

        if let Some(complete) = headers.pop().and_then(parse_header_line) {
          head.add_header(complete)?;
        }

        headers.push(temp);
      }
    }
  }

  if let (None, Some(line)) = (&head._req, headers.get(0)) {
    head._req = Some(parse_request_line(line.to_string())?)
  }

  if headers.len() == 2 {
    if let Some(last) = headers.pop().and_then(parse_header_line) {
      head.add_header(last)?;
    }
  }

  Ok(head)
}

#[cfg(test)]
mod tests {
  use super::parse_request_line;

  #[test]
  fn valid_request_lint() {
    let result = parse_request_line(String::from("whoa"));
    assert!(result.is_err());
  }
}
