use async_std::io::{Read, Write};
use async_std::net::TcpListener;
use async_std::prelude::*;
use async_std::task::block_on;
use elaine::{recognize, Head};
use std::error::Error;

async fn route<R>(conn: Head, stream: &mut R) -> Result<Option<Vec<u8>>, std::io::Error>
where
  R: Read + Write + std::marker::Unpin,
{
  if let Some(len) = conn.len() {
    let buffered = stream.bytes();
    let mut taker = buffered.take(len);
    let mut body = Vec::with_capacity(len);

    while let Some(Ok(byte)) = taker.next().await {
      body.push(byte);
    }

    println!("read body: {:?}", String::from_utf8(body));
    stream.write(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    return Ok(None);
  }

  stream.write(b"HTTP/1.1 200 OK\r\n\r\n").await?;
  Ok(None)
}

fn main() -> Result<(), Box<dyn Error>> {
  let addr = std::env::var("ELAINE_ADDR").unwrap_or(String::from("0.0.0.0:8080"));
  println!("[debug] starting blocking async loop w/ listener on '{}'", addr);

  block_on(async {
    let listener = TcpListener::bind(addr.as_str()).await?;

    loop {
      match listener.incoming().next().await {
        Some(Ok(mut stream)) => {
          let head = recognize(&mut stream).await?;
          println!("responding to {}", head);
          route(head, &mut stream).await?;
          println!("done");
          drop(stream);
        }
        _ => continue,
      }
    }
  })
}
