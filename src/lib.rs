//! ETF Erlang External Term Format codec compatible with `discord/erlpack` !

#![forbid(unsafe_code)]

pub mod decode;
pub mod encode;
pub mod term;

#[cfg(feature = "convert-json")]
pub mod convert;

#[cfg(feature = "splice")]
pub mod splice;

pub use decode::{DecodeConfig, DecodeError, decode, decode_with};
pub use encode::{EncodeError, encode};
pub use term::Term;

#[cfg(feature = "convert-json")]
pub use convert::{ConvertError, encode_value, encode_value_into, from_value, to_value};

#[cfg(feature = "splice")]
pub use splice::{SplicedValue, splice_map};
