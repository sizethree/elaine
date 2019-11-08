use async_std::io::Read;
use async_std::prelude::*;
use std::io::{Error, ErrorKind};

use crate::head::{Builder, Head};

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

/// Reads from the reader, consuming valid utf-8 charactes in 1-4 byte sized chunks, stopping
/// after successfully reaching a [CR LF sequence][rfc-1945]. The method will return an
/// `std::io::Error` under any of the following conditions:
///
///  - A failed read from the underlying reader.
///  - Invalid first line, per the [request line][req-line] specification.
///  - Invalid utf-8 encoding, at any point.
///
///  # Arguments
///
///  * `reader` - Some implementation of [`async_std::io::Read`][read]
///
/// [rfc-1945]: https://tools.ietf.org/html/rfc1945#section-4.1
/// [req-line]: https://tools.ietf.org/html/rfc1945#section-5.1
/// [read]: https://tools.ietf.org/html/rfc1945#section-5.1
pub async fn recognize<R>(mut reader: R) -> Result<Head, Error>
where
  R: Read + std::marker::Unpin,
{
  let mut marker = Marker::default();
  let mut headers: Vec<String> = Vec::new();
  let mut builder = Builder::new();

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

    if headers.len() == 2 && lc != headers.len() {
      let temp = headers.pop().unwrap_or_default();

      if let Some(complete) = headers.pop() {
        builder.insert(complete)?;
      }

      headers.push(temp);
    }
  }

  if let Some(last) = headers.pop() {
    builder.insert(last)?;
  }

  Ok(builder.collect::<Head>())
}
