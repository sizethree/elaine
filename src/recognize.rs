use async_std::io::Read;
use async_std::prelude::*;
use std::io::{Error, ErrorKind};
use std::marker::Unpin;

use crate::head::{Builder, Head};

#[derive(Debug)]
enum Capacity {
  One,
  Two,
  Three,
  Four,
}

#[derive(Debug)]
struct Stack(Option<String>, Option<String>);

impl Default for Stack {
  fn default() -> Stack {
    Stack(None, None)
  }
}

impl Stack {
  fn push(&mut self, content: String) {
    if self.1.is_some() {
      self.0 = self.1.to_owned()
    }
    self.1 = Some(content);
  }

  fn last_mut(&mut self) -> Option<&mut String> {
    if let Some(value) = self.1.as_mut() {
      return Some(value);
    }

    None
  }

  fn fin(mut self) -> Option<String> {
    if let Some(value) = self.1.as_mut() {
      return Some(value.to_owned());
    }
    None
  }

  fn pop(&mut self) -> Option<String> {
    if self.0.is_none() {
      return None;
    }

    if let Some(value) = self.0.as_mut() {
      let out = value.to_owned();
      self.0 = None;
      return Some(out);
    }

    None
  }
}

async fn fill_utf8<R>(original: &[u8], reader: R) -> Result<String, Error>
where
  R: Read + Unpin,
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

fn invalid_read<H>(mut stack: Stack) -> Result<H, Error> {
  let message = match stack.last_mut() {
    Some(value) => format!(". Last read '{}'", value),
    None => String::from("Reader exhausted before terminating HTTP head"),
  };
  Err(Error::new(ErrorKind::Other, message))
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
/// [read]: https://docs.rs/async-std/0.99.12/async_std/io/trait.Read.html
pub async fn recognize<R>(mut reader: R) -> Result<Head, Error>
where
  R: Read + Unpin,
{
  let mut marker = Capacity::Four;
  let mut stack = Stack::default();
  let mut builder = Builder::new();

  loop {
    let mut buf: Vec<u8> = match marker {
      Capacity::Four => vec![0x13, 0x10, 0x13, 0x10],
      Capacity::Three => vec![0x10, 0x13, 0x10],
      Capacity::Two => vec![0x13, 0x10],
      Capacity::One => vec![0x10],
    };

    let size = reader.read(&mut buf).await?;
    let chunk = fill_utf8(&buf[0..size], &mut reader).await?;

    let mut chars = chunk.chars();

    match (marker, chars.next(), chars.next(), chars.next(), chars.next()) {
      // clean terminal
      (_, Some('\r'), Some('\n'), Some('\r'), Some('\n')) => break,
      // terminal from previous '\r\n\r'
      (Capacity::One, Some('\n'), _, _, _) => break,
      // terminal from previous '\r\n'
      (Capacity::Two, Some('\r'), Some('\n'), _, _) => break,
      // non-terminal: had a cr lf but now working with something else
      (Capacity::Two, Some(one), Some(two), _, _) => {
        stack.push([one, two].iter().collect::<String>());
        marker = Capacity::Four;
      }
      // terminal from previous '\r'
      (Capacity::Three, Some('\n'), Some('\r'), Some('\n'), _) => break,
      (Capacity::Three, Some('\n'), Some(one), None, None) => {
        stack.push(format!("{}", one));
        marker = Capacity::Four;
      }
      (Capacity::Three, Some('\n'), Some(one), Some(two), None) => {
        stack.push(format!("{}{}", one, two));
        marker = Capacity::Four;
      }

      // any char followed by '\r\n\r' - queue up single read
      (_, Some(one), Some('\r'), Some('\n'), Some('\r')) => {
        match stack.last_mut() {
          Some(header) => header.push(one),
          None => stack.push(one.to_string()),
        }
        marker = Capacity::One;
      }

      // any chars followed by '\r\n' - queue up double read
      (_, Some(one), Some(two), Some('\r'), Some('\n')) => {
        let mem = format!("{}{}", one, two);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Two;
      }

      // any chars followed by '\r' - queue up triple read
      (_, Some(one), Some(two), Some(three), Some('\r')) => {
        let mem = format!("{}{}{}", one, two, three);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Three;
      }

      (_, Some(one), Some('\r'), None, None) => {
        match stack.last_mut() {
          Some(header) => {
            header.push(one);
          }
          None => stack.push([one].iter().collect::<String>()),
        }
        marker = Capacity::Three;
      }

      (_, Some(one), Some(two), Some('\r'), None) => {
        let mem = format!("{}{}", one, two);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Three;
      }

      (_, Some('\r'), Some('\n'), Some(one), Some(two)) => {
        stack.push([one, two].iter().collect::<String>());
        marker = Capacity::Four;
      }

      (_, Some(one), Some('\r'), Some('\n'), Some(two)) => {
        match stack.last_mut() {
          Some(header) => {
            header.push(one);
          }
          None => stack.push(one.to_string()),
        }
        stack.push(two.to_string());
        marker = Capacity::Four;
      }

      (_, Some(one), Some(two), Some(three), Some(four)) => {
        let mem = format!("{}{}{}{}", one, two, three, four);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Four;
      }
      (_, Some(one), Some(two), Some(three), None) => {
        let mem = format!("{}{}{}", one, two, three);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Four;
      }
      (_, Some(one), Some(two), None, None) => {
        let mem = format!("{}{}", one, two);
        match stack.last_mut() {
          Some(header) => header.push_str(mem.as_str()),
          None => stack.push(mem),
        }
        marker = Capacity::Four;
      }
      (_, Some(one), None, None, None) => {
        match stack.last_mut() {
          Some(header) => header.push(one),
          None => stack.push(format!("{}", one)),
        }
        marker = Capacity::Four;
      }
      (_, Some(_), None, Some(_), Some(_)) => return invalid_read(stack),
      (_, Some(_), None, Some(_), None) => return invalid_read(stack),
      (_, Some(_), None, None, Some(_)) => return invalid_read(stack),
      (_, Some(_), Some(_), None, Some(_)) => return invalid_read(stack),
      (_, None, _, _, _) => {
        let err = match stack.last_mut() {
          Some(partial) => format!(
            "Reader exhausted with non-terminated HTTP head. Last header line attempt: '{}'",
            partial
          ),
          None => String::from("Reader exhausted before any recognizable line was parsed."),
        };
        return Err(Error::new(ErrorKind::UnexpectedEof, err));
      }
    }

    if let Some(complete) = stack.pop() {
      builder.insert(complete)?;
    }
  }

  if let Some(last) = stack.fin() {
    builder.insert(last)?;
  }

  Ok(builder.collect::<Head>())
}
