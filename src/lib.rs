extern crate async_std;

use async_std::io::Read;
use async_std::prelude::*;
use std::collections::VecDeque;
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

pub async fn recog<R>(reader: &mut R) -> Result<VecDeque<String>, Error>
where
  R: Read + std::marker::Unpin,
{
  let mut marker = Marker::default();
  let mut headers: VecDeque<String> = VecDeque::new();

  loop {
    let mut buf: Vec<u8> = match &marker.capacity {
      Capacity::Four => vec![0x13, 0x10, 0x13, 0x10],
      Capacity::Three => vec![0x10, 0x13, 0x10],
      Capacity::Two => vec![0x13, 0x10],
      Capacity::One => vec![0x10],
    };

    let size = reader.read(&mut buf).await?;
    let chunk = match std::str::from_utf8(&buf[0..size]) {
      Ok(c) => String::from(c),
      Err(e) => match e.error_len() {
        None => {
          let (valid, after_valid) = buf.split_at(e.valid_up_to());
          let stage = std::str::from_utf8(valid).map_err(|e| Error::new(ErrorKind::InvalidData, format!("{:?}", e)))?;
          match after_valid {
            [first, second, third] => {
              let mut bytes = reader.bytes();
              let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
              match std::str::from_utf8(&[*first, *second, *third, fourth]) {
                Ok(rest) => format!("{}{}", stage, rest),
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
                Ok(rest) => format!("{}{}", stage, rest),
                Err(_e) => {
                  let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
                  match std::str::from_utf8(&[*first, *second, third, fourth]) {
                    Ok(rest) => format!("{}{}", stage, rest),
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
                Ok(rest) => format!("{}{}", stage, rest),
                Err(e) => match e.error_len() {
                  None => {
                    let third = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;

                    match std::str::from_utf8(&[*single, second, third]) {
                      Ok(rest) => format!("{}{}", stage, rest),
                      Err(_e) => {
                        let fourth = bytes.next().await.ok_or(Error::from(ErrorKind::InvalidData))??;
                        match std::str::from_utf8(&[*single, second, third, fourth]) {
                          Ok(rest) => format!("{}{}", stage, rest),
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
    };
    let mut chars = chunk.chars();

    match (&marker.capacity, chars.next(), chars.next(), chars.next(), chars.next()) {
      // clean terminal
      (_, Some('\r'), Some('\n'), Some('\r'), Some('\n')) => break,
      // terminal from previous '\r\n\r'
      (Capacity::One, Some('\n'), _, _, _) => break,
      // terminal from previous '\r\n'
      (Capacity::Two, Some('\r'), Some('\n'), _, _) => break,
      // non-terminal: had a cr lf but now working with something else
      (Capacity::Two, Some(one), Some(two), _, _) => {
        headers.push_back([one, two].iter().collect::<String>());
        marker.capacity = Capacity::Four;
      }
      // terminal from previous '\r'
      (Capacity::Three, Some('\n'), Some('\r'), Some('\n'), _) => break,

      // any char followed by '\r\n\r' - queue up single read
      (_, Some(one), Some('\r'), Some('\n'), Some('\r')) => {
        match headers.back_mut() {
          Some(header) => header.push(one),
          None => headers.push_back(one.to_string()),
        }
        marker.capacity = Capacity::One;
      }

      // any chars followed by '\r\n' - queue up double read
      (_, Some(one), Some(two), Some('\r'), Some('\n')) => {
        match headers.back_mut() {
          Some(header) => {
            header.reserve(2);
            header.push(one);
            header.push(two);
          }
          None => headers.push_back([one, two].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Two;
      }

      // any chars followed by '\r' - queue up triple read
      (_, Some(one), Some(two), Some(three), Some('\r')) => {
        match headers.back_mut() {
          Some(header) => {
            header.reserve(3);
            header.push(one);
            header.push(two);
            header.push(three);
          }
          None => headers.push_back([one, two, three].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Three;
      }

      (_, Some('\r'), Some('\n'), Some(one), Some(two)) => {
        headers.push_back([one, two].iter().collect::<String>());
        marker.capacity = Capacity::Four;
      }

      (_, Some(one), Some('\r'), Some('\n'), Some(two)) => {
        match headers.back_mut() {
          Some(header) => {
            header.push(one);
          }
          None => headers.push_back(one.to_string()),
        }
        headers.push_back(two.to_string());
        marker.capacity = Capacity::Four;
      }

      (_, Some(one), Some(two), Some(three), Some(four)) => {
        match headers.back_mut() {
          Some(header) => {
            header.reserve(4);
            header.push(one);
            header.push(two);
            header.push(three);
            header.push(four);
          }
          None => headers.push_back([one, two, three, four].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), Some(three), None) => {
        match headers.back_mut() {
          Some(header) => {
            header.reserve(3);
            header.push(one);
            header.push(two);
            header.push(three);
          }
          None => headers.push_back([one, two, three].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), None, None) => {
        match headers.back_mut() {
          Some(header) => {
            header.reserve(2);
            header.push(one);
            header.push(two);
          }
          None => headers.push_back([one, two].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), None, None, None) => {
        match headers.back_mut() {
          Some(header) => {
            header.push(one);
          }
          None => headers.push_back(format!("{}", one)),
        }
        marker.capacity = Capacity::Four;
      }
      _ => return Err(Error::new(ErrorKind::Other, "Invalid sequence")),
    }
  }

  Ok(headers)
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
mod tests {
  use super::parse_request_line;

  #[test]
  fn valid_request_lint() {
    let result = parse_request_line(String::from("whoa"));
    assert!(result.is_err());
  }
}
