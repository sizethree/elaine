//! The primary export of this crate is [`recognize`], a lightweight and potentially incomplete http head parser
//! implementation for readers that satisfy the [`async_std::io::Read`][read] trait.
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
//!
//! [`recognize`]: fn.recognize.html
//! [read]: https://docs.rs/async-std/0.99.12/async_std/io/trait.Read.html
extern crate async_std;

mod head;
pub use head::{Builder, Head, RequestMethod, RequestVersion};

mod recognize;
pub use recognize::recognize;
