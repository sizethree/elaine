## Elaine

[![ci.img]][ci.url] [![docs.img]][docs.url] [![crates.img]][crates.url]

This crate provides a lightweight and potentially incomplete http head parser implementation
for async-std readers.

## Example

```rust
use std::boxed::Box;
use std::error::Error;

use elaine::recognize;
use async_std::task::block_on;

fn main() -> Result<(), Box<dyn Error>> {
  block_on(async {
    let mut req: &[u8] = b"GET /elaine HTTP/1.1\r\nContent-Length: 3\r\n\r\nhey";
    let result = recognize(&mut req).await.unwrap();
    assert_eq!(result.method(), Some("GET".to_string()));
    assert_eq!(result.len(), Some(3));
    assert_eq!(std::str::from_utf8(req), Ok("hey"));
  });
  Ok(())
}
```


| elaine |
| --- |
| ![elaine][elaine] |

## Contributing

See [CONTRIBUTING](/CONTRIBUTING.md).

[ci.img]: https://github.com/sizethree/elaine/workflows/gh.build/badge.svg?flat
[ci.url]: https://github.com/sizethree/elaine/actions?workflow=gh.build
[redis]: https://redis.io/topics/protocol
[async-std]: https://github.com/async-rs/async-std
[tcp-stream]: https://docs.rs/async-std/0.99.11/async_std/net/struct.TcpStream.html
[docs.img]: https://docs.rs/elaine/badge.svg
[docs.url]: https://docs.rs/elaine/latest
[crates.url]: https://crates.io/crates/elaine
[crates.img]: https://img.shields.io/crates/v/elaine
[elaine]: https://user-images.githubusercontent.com/1545348/68368941-1cee4e80-0107-11ea-8e87-47cb29cf8e15.gif
