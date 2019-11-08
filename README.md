## Elaine

[![ci.img]][ci.url] [![docs.img]][docs.url] [![crates.img]][crates.url]

This crate provides a lightweight and potentially incomplete http head parser implementation
for async-std readers.

## Goals &amp; Stuff

This crate is intended to provide an HTTP head parser for async [`readers`] with a focus on simplicity and safety;
while performance is appreciated, safety and simplicity take priority.
  
### On Safety

The api provided by this crate will _never_ include `unsafe` code directly, including code that would otherwise
improve the performance of the libary. In addition, the main export - [`recognize`][recognize] - provides the
guaruntee that it will _never_ over-read bytes from a reader, again at the potential loss of performance.

### On Simplictity

This crate does not include the [`http`][http-crate] in it's dependencies; though well-maintained and useful as it is,
it would introduce a super set of functionality that is not required for this implementation. This decision is not 
in any way meant to discourage other developers from using that library.

## Example

```rust
use std::boxed::Box;
use std::error::Error;

use elaine::{recognize, RequestMethod};
use async_std::task::block_on;

fn main() -> Result<(), Box<dyn Error>> {
  block_on(async {
    let mut req: &[u8] = b"GET /elaine HTTP/1.1\r\nContent-Length: 3\r\n\r\nhey";
    let result = recognize(&mut req).await.unwrap();
    assert_eq!(result.method(), Some(RequestMethod::GET));
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
[`readers`]: https://docs.rs/async-std/0.99.12/async_std/io/trait.Read.html
[http-crate]: https://crates.io/crates/http
[recognize]: https://docs.rs/elaine/0.1.1/elaine/fn.recognize.html
