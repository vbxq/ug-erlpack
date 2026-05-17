//! ETF Erlang External Term Format codec compatible with `discord/erlpack` !

#![forbid(unsafe_code)]

pub mod decode;
pub mod encode;
pub mod term;

#[cfg(feature = "convert-json")]
pub mod convert;

pub use decode::{DecodeConfig, DecodeError, decode, decode_with};
pub use encode::{EncodeError, encode};
pub use term::Term;

#[cfg(feature = "convert-json")]
pub use convert::{ConvertError, from_value, to_value};
