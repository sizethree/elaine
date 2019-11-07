extern crate async_std;

use async_std::io::Read;
use async_std::prelude::*;
use std::io::{Error, ErrorKind};

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
  _headers: Vec<Header>,
  _req: Option<RequestLine>,
  _len: Option<usize>,
  _auth: Option<String>,
}

impl Head {
  pub fn path(&self) -> Option<String> {
    self._req.as_ref().map(|r| r.path.clone())
  }

  pub fn version(&self) -> Option<String> {
    self._req.as_ref().map(|r| r.version.clone())
  }

  pub fn method(&self) -> Option<String> {
    self._req.as_ref().map(|r| r.method.clone())
  }

  pub fn len(&self) -> Option<usize> {
    self._len
  }

  pub fn find_header<S>(&self, target: S) -> Option<String>
  where
    S: std::fmt::Display,
  {
    for Header(key, value) in self._headers.iter() {
      if *key == format!("{}", target) {
        return Some(value.to_string());
      }
    }
    None
  }

  fn add_header(&mut self, header: Header) -> Result<(), Error> {
    let Header(key, value) = header;

    if key == "Content-Length" {
      match value.parse::<usize>() {
        Ok(value) => self._len = Some(value),
        Err(e) => {
          return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid content length: {:?}", e),
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
      "{} {} {}\r\nContent-Length: {:?}\r\n\r\n",
      self.method().unwrap_or(String::from("<UNKNOWN>")),
      self.path().unwrap_or(String::from("<UNKNOWN>")),
      self.version().unwrap_or(String::from("<UNKNOWN>")),
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

pub async fn recognize<R>(reader: &mut R) -> Result<Head, Error>
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
        match headers.last_mut() {
          Some(header) => {
            header.reserve(2);
            header.push(one);
            header.push(two);
          }
          None => headers.push([one, two].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Two;
      }

      // any chars followed by '\r' - queue up triple read
      (_, Some(one), Some(two), Some(three), Some('\r')) => {
        match headers.last_mut() {
          Some(header) => {
            header.reserve(3);
            header.push(one);
            header.push(two);
            header.push(three);
          }
          None => headers.push([one, two, three].iter().collect::<String>()),
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
        match headers.last_mut() {
          Some(header) => {
            header.reserve(2);
            header.push(one);
            header.push(two);
          }
          None => headers.push([one, two].iter().collect::<String>()),
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
        match headers.last_mut() {
          Some(header) => {
            header.reserve(4);
            header.push(one);
            header.push(two);
            header.push(three);
            header.push(four);
          }
          None => headers.push([one, two, three, four].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), Some(three), None) => {
        match headers.last_mut() {
          Some(header) => {
            header.reserve(3);
            header.push(one);
            header.push(two);
            header.push(three);
          }
          None => headers.push([one, two, three].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), Some(two), None, None) => {
        match headers.last_mut() {
          Some(header) => {
            header.reserve(2);
            header.push(one);
            header.push(two);
          }
          None => headers.push([one, two].iter().collect::<String>()),
        }
        marker.capacity = Capacity::Four;
      }
      (_, Some(one), None, None, None) => {
        match headers.last_mut() {
          Some(header) => {
            header.push(one);
          }
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
