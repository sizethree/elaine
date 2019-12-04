use std::io::{Error, ErrorKind};

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

impl RequestMethod {
  pub fn parse<S>(input: S) -> Result<Self, Error>
  where
    S: std::fmt::Display,
  {
    match format!("{}", input).as_str() {
      "CONNECT" => Ok(RequestMethod::CONNECT),
      "DELETE" => Ok(RequestMethod::DELETE),
      "GET" => Ok(RequestMethod::GET),
      "HEAD" => Ok(RequestMethod::HEAD),
      "OPTIONS" => Ok(RequestMethod::OPTIONS),
      "POST" => Ok(RequestMethod::POST),
      "PUT" => Ok(RequestMethod::PUT),
      "PATCH" => Ok(RequestMethod::PATCH),
      "TRACE" => Ok(RequestMethod::TRACE),
      _ => {
        return Err(Error::new(
          ErrorKind::InvalidData,
          format!("Unable to parse request method"),
        ))
      }
    }
  }
}

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
#[derive(Debug)]
struct RequestLine {
  method: RequestMethod,
  path: String,
  version: RequestVersion,
}

#[derive(Debug)]
struct Header(pub String, pub String);

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
      let version = RequestVersion::parse(tail)?;
      let method = RequestMethod::parse(first)?;

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
pub struct Builder {
  inner: Head,
}

impl Builder {
  pub fn new() -> Self {
    Builder { inner: Head::default() }
  }

  pub fn collect<D>(self) -> Head
  where
    D: From<Builder>,
  {
    self.inner
  }

  pub fn len(&self) -> usize {
    self.inner._headers.len()
  }

  pub fn insert(&mut self, line: String) -> Result<(), Error> {
    if self.inner._req.is_none() {
      let req = parse_request_line(line)?;
      self.inner._req = Some(req);
      return Ok(());
    }

    if let Some(header) = parse_header_line(line) {
      return self.inner.add_header(header);
    }

    Ok(())
  }
}

#[derive(Debug, Default)]
pub struct Head {
  _headers: Vec<Header>,
  _req: Option<RequestLine>,
  _len: Option<usize>,
  _auth: Option<String>,
}

impl From<Builder> for Head {
  fn from(builder: Builder) -> Head {
    builder.inner
  }
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
