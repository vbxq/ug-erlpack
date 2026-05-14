//! ETF Erlang External Term Format codec compatible with `discord/erlpack` !

#![forbid(unsafe_code)]

pub mod term;
pub mod encode;
pub mod decode;

#[cfg(feature = "convert-json")]
pub mod convert;

pub use term::Term;
pub use encode::{encode, EncodeError};
pub use decode::{decode, decode_with, DecodeError, DecodeConfig};

#[cfg(feature = "convert-json")]
pub use convert::{to_value, from_value, ConvertError};
