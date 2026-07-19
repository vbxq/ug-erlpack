//! ETF decoder

use core::str::Utf8Error;

use thiserror::Error;

use crate::term::Term;

const NEW_FLOAT_EXT: u8 = 70;
const BIT_BINARY_EXT: u8 = 77;
const COMPRESSED: u8 = 80;
const SMALL_INTEGER_EXT: u8 = 97;
const INTEGER_EXT: u8 = 98;
const FLOAT_EXT: u8 = 99;
const ATOM_EXT: u8 = 100;
const REFERENCE_EXT: u8 = 101;
const PORT_EXT: u8 = 102;
const PID_EXT: u8 = 103;
const SMALL_TUPLE_EXT: u8 = 104;
const LARGE_TUPLE_EXT: u8 = 105;
const NIL_EXT: u8 = 106;
const STRING_EXT: u8 = 107;
const LIST_EXT: u8 = 108;
const BINARY_EXT: u8 = 109;
const SMALL_BIG_EXT: u8 = 110;
const LARGE_BIG_EXT: u8 = 111;
const NEW_FUN_EXT: u8 = 112;
const EXPORT_EXT: u8 = 113;
const NEW_REFERENCE_EXT: u8 = 114;
const SMALL_ATOM_EXT: u8 = 115;
const MAP_EXT: u8 = 116;
const FUN_EXT: u8 = 117;
const ATOM_UTF8_EXT: u8 = 118;
const SMALL_ATOM_UTF8_EXT: u8 = 119;
const VERSION_BYTE: u8 = 131;

const DEFAULT_BIG_DIGIT_CEILING: u32 = 64;
const DEFAULT_MAX_DEPTH: u32 = 32;

#[derive(Clone, Debug)]
pub struct DecodeConfig {
    pub big_digit_ceiling: u32,
    pub max_depth: u32,
}

impl DecodeConfig {
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn with_big_digit_ceiling(mut self, max_bytes: u32) -> Self {
        self.big_digit_ceiling = max_bytes;
        self
    }
    #[must_use]
    pub fn with_max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth;
        self
    }
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            big_digit_ceiling: DEFAULT_BIG_DIGIT_CEILING,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("bad version byte: expected 131, found {found}")]
    BadVersion { found: u8 },
    #[error("unknown ETF tag {tag} at offset {offset}")]
    UnknownTag { tag: u8, offset: usize },
    #[error("inline COMPRESSED (tag 80) refused at offset {offset}")]
    CompressedNotSupported { offset: usize },
    #[error("BIT_BINARY_EXT (tag 77) not supported at offset {offset}")]
    BitBinaryNotSupported { offset: usize },
    #[error("Erlang-runtime term tag {tag} not supported at offset {offset}")]
    ErlangRuntimeTermNotSupported { tag: u8, offset: usize },
    #[error("unexpected EOF at offset {offset}: needed {needed}, available {available}")]
    UnexpectedEof {
        needed: usize,
        available: usize,
        offset: usize,
    },
    #[error("big-int digit count {declared} exceeds ceiling {ceiling} at offset {offset}")]
    BigDigitCeilingExceeded {
        declared: u32,
        ceiling: u32,
        offset: usize,
    },
    #[error("LIST_EXT tail tag {tail_tag} != NIL_EXT at offset {offset}")]
    ImproperList { tail_tag: u8, offset: usize },
    #[error("nesting depth {depth} exceeds max_depth {ceiling} at offset {offset}")]
    MaxDepthExceeded {
        depth: u32,
        ceiling: u32,
        offset: usize,
    },
    #[error("atom UTF-8 validation failed at offset {offset}: {source}")]
    AtomInvalidUtf8 {
        offset: usize,
        #[source]
        source: Utf8Error,
    },
    #[error("FLOAT_EXT legacy ASCII float parse failed at offset {offset}")]
    LegacyFloatParseFailed { offset: usize },
}

impl DecodeError {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::BadVersion { .. } => "bad_version",
            Self::UnknownTag { .. } => "unknown_tag",
            Self::CompressedNotSupported { .. } => "compressed_not_supported",
            Self::BitBinaryNotSupported { .. } => "bit_binary_not_supported",
            Self::ErlangRuntimeTermNotSupported { .. } => "erlang_runtime_term_not_supported",
            Self::UnexpectedEof { .. } => "unexpected_eof",
            Self::BigDigitCeilingExceeded { .. } => "big_digit_ceiling_exceeded",
            Self::ImproperList { .. } => "improper_list",
            Self::MaxDepthExceeded { .. } => "max_depth_exceeded",
            Self::AtomInvalidUtf8 { .. } => "atom_invalid_utf8",
            Self::LegacyFloatParseFailed { .. } => "legacy_float_parse_failed",
        }
    }
}

pub fn decode(bytes: &[u8]) -> Result<(Term, usize), DecodeError> {
    decode_with(bytes, &DecodeConfig::default())
}

pub fn decode_with(bytes: &[u8], cfg: &DecodeConfig) -> Result<(Term, usize), DecodeError> {
    let mut reader = Reader::new(bytes, cfg);
    let version = reader.read_u8()?;
    if version != VERSION_BYTE {
        return Err(DecodeError::BadVersion { found: version });
    }
    let term = decode_term(&mut reader, 0)?;
    Ok((term, reader.offset))
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    cfg: &'a DecodeConfig,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], cfg: &'a DecodeConfig) -> Self {
        Self {
            bytes,
            offset: 0,
            cfg,
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn require(&self, n: usize) -> Result<(), DecodeError> {
        if self.remaining() < n {
            Err(DecodeError::UnexpectedEof {
                needed: n,
                available: self.remaining(),
                offset: self.offset,
            })
        } else {
            Ok(())
        }
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        self.require(1)?;
        let b = self.bytes[self.offset];
        self.offset += 1;
        Ok(b)
    }

    fn read_u16_be(&mut self) -> Result<u16, DecodeError> {
        self.require(2)?;
        let arr: [u8; 2] = self.bytes[self.offset..self.offset + 2]
            .try_into()
            .expect("require(2) succeeded above");
        self.offset += 2;
        Ok(u16::from_be_bytes(arr))
    }

    fn read_u32_be(&mut self) -> Result<u32, DecodeError> {
        self.require(4)?;
        let arr: [u8; 4] = self.bytes[self.offset..self.offset + 4]
            .try_into()
            .expect("require(4) succeeded above");
        self.offset += 4;
        Ok(u32::from_be_bytes(arr))
    }

    fn read_i32_be(&mut self) -> Result<i32, DecodeError> {
        self.require(4)?;
        let arr: [u8; 4] = self.bytes[self.offset..self.offset + 4]
            .try_into()
            .expect("require(4) succeeded above");
        self.offset += 4;
        Ok(i32::from_be_bytes(arr))
    }

    fn read_u64_be(&mut self) -> Result<u64, DecodeError> {
        self.require(8)?;
        let arr: [u8; 8] = self.bytes[self.offset..self.offset + 8]
            .try_into()
            .expect("require(8) succeeded above");
        self.offset += 8;
        Ok(u64::from_be_bytes(arr))
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        self.require(n)?;
        let slice = &self.bytes[self.offset..self.offset + n];
        self.offset += n;
        Ok(slice)
    }
}

fn decode_term(reader: &mut Reader, depth: u32) -> Result<Term, DecodeError> {
    if depth > reader.cfg.max_depth {
        return Err(DecodeError::MaxDepthExceeded {
            depth,
            ceiling: reader.cfg.max_depth,
            offset: reader.offset,
        });
    }
    let tag_offset = reader.offset;
    let tag = reader.read_u8()?;
    match tag {
        NEW_FLOAT_EXT => decode_new_float(reader),
        SMALL_INTEGER_EXT => decode_small_integer(reader),
        INTEGER_EXT => decode_integer(reader),
        FLOAT_EXT => decode_legacy_float(reader, tag_offset),
        ATOM_EXT => decode_atom_latin1_long(reader),
        SMALL_TUPLE_EXT => decode_small_tuple(reader, depth),
        LARGE_TUPLE_EXT => decode_large_tuple(reader, depth),
        NIL_EXT => Ok(Term::List(Vec::new())),
        STRING_EXT => decode_string_ext(reader),
        LIST_EXT => decode_list(reader, depth),
        BINARY_EXT => decode_binary(reader),
        SMALL_BIG_EXT => decode_small_big(reader, tag_offset),
        LARGE_BIG_EXT => decode_large_big(reader, tag_offset),
        SMALL_ATOM_EXT => decode_small_atom_latin1(reader),
        MAP_EXT => decode_map(reader, depth),
        ATOM_UTF8_EXT => decode_atom_utf8(reader),
        SMALL_ATOM_UTF8_EXT => decode_small_atom_utf8(reader),
        COMPRESSED => Err(DecodeError::CompressedNotSupported { offset: tag_offset }),
        BIT_BINARY_EXT => Err(DecodeError::BitBinaryNotSupported { offset: tag_offset }),
        REFERENCE_EXT | PORT_EXT | PID_EXT | NEW_FUN_EXT | EXPORT_EXT | NEW_REFERENCE_EXT
        | FUN_EXT => Err(DecodeError::ErlangRuntimeTermNotSupported {
            tag,
            offset: tag_offset,
        }),
        _ => Err(DecodeError::UnknownTag {
            tag,
            offset: tag_offset,
        }),
    }
}

fn decode_new_float(reader: &mut Reader) -> Result<Term, DecodeError> {
    let bits = reader.read_u64_be()?;
    Ok(Term::Float(f64::from_bits(bits)))
}

fn decode_small_integer(reader: &mut Reader) -> Result<Term, DecodeError> {
    let n = reader.read_u8()?;
    Ok(Term::Integer(i32::from(n)))
}

fn decode_integer(reader: &mut Reader) -> Result<Term, DecodeError> {
    let n = reader.read_i32_be()?;
    Ok(Term::Integer(n))
}

fn decode_legacy_float(reader: &mut Reader, tag_offset: usize) -> Result<Term, DecodeError> {
    let bytes = reader.read_bytes(31)?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let prefix = &bytes[..end];
    let s = core::str::from_utf8(prefix)
        .map_err(|_| DecodeError::LegacyFloatParseFailed { offset: tag_offset })?;
    s.trim()
        .parse::<f64>()
        .map(Term::Float)
        .map_err(|_| DecodeError::LegacyFloatParseFailed { offset: tag_offset })
}

fn decode_atom_latin1_long(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = usize::from(reader.read_u16_be()?);
    let bytes = reader.read_bytes(len)?;
    Ok(Term::Atom(latin1_to_string(bytes)))
}

fn decode_small_atom_latin1(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = usize::from(reader.read_u8()?);
    let bytes = reader.read_bytes(len)?;
    Ok(Term::Atom(latin1_to_string(bytes)))
}

fn decode_atom_utf8(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = usize::from(reader.read_u16_be()?);
    let payload_offset = reader.offset;
    let bytes = reader.read_bytes(len)?;
    let s = core::str::from_utf8(bytes).map_err(|source| DecodeError::AtomInvalidUtf8 {
        offset: payload_offset,
        source,
    })?;
    Ok(Term::Atom(s.to_string()))
}

fn decode_small_atom_utf8(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = usize::from(reader.read_u8()?);
    let payload_offset = reader.offset;
    let bytes = reader.read_bytes(len)?;
    let s = core::str::from_utf8(bytes).map_err(|source| DecodeError::AtomInvalidUtf8 {
        offset: payload_offset,
        source,
    })?;
    Ok(Term::Atom(s.to_string()))
}

fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

fn decode_small_tuple(reader: &mut Reader, depth: u32) -> Result<Term, DecodeError> {
    let arity = usize::from(reader.read_u8()?);
    decode_tuple_elements(reader, arity, depth)
}

fn decode_large_tuple(reader: &mut Reader, depth: u32) -> Result<Term, DecodeError> {
    let arity = reader.read_u32_be()? as usize;
    decode_tuple_elements(reader, arity, depth)
}

fn decode_tuple_elements(reader: &mut Reader, arity: usize, depth: u32) -> Result<Term, DecodeError> {
    let cap = arity.min(reader.remaining());
    let mut elems = Vec::with_capacity(cap);
    for _ in 0..arity {
        elems.push(decode_term(reader, depth + 1)?);
    }
    Ok(Term::Tuple(elems))
}

fn decode_string_ext(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = usize::from(reader.read_u16_be()?);
    let bytes = reader.read_bytes(len)?;
    let elems: Vec<Term> = bytes.iter().map(|&b| Term::Integer(i32::from(b))).collect();
    Ok(Term::List(elems))
}

fn decode_list(reader: &mut Reader, depth: u32) -> Result<Term, DecodeError> {
    let arity = reader.read_u32_be()? as usize;
    let cap = arity.min(reader.remaining());
    let mut elems = Vec::with_capacity(cap);
    for _ in 0..arity {
        elems.push(decode_term(reader, depth + 1)?);
    }
    let tail_offset = reader.offset;
    let tail_tag = reader.read_u8()?;
    if tail_tag != NIL_EXT {
        return Err(DecodeError::ImproperList {
            tail_tag,
            offset: tail_offset,
        });
    }
    Ok(Term::List(elems))
}

fn decode_binary(reader: &mut Reader) -> Result<Term, DecodeError> {
    let len = reader.read_u32_be()? as usize;
    let bytes = reader.read_bytes(len)?;
    Ok(Term::Binary(bytes.to_vec()))
}

fn decode_small_big(reader: &mut Reader, tag_offset: usize) -> Result<Term, DecodeError> {
    let n = u32::from(reader.read_u8()?);
    if n > reader.cfg.big_digit_ceiling {
        return Err(DecodeError::BigDigitCeilingExceeded {
            declared: n,
            ceiling: reader.cfg.big_digit_ceiling,
            offset: tag_offset,
        });
    }
    let needed = 1usize.saturating_add(n as usize);
    reader.require(needed)?;
    let sign_byte = reader.read_u8()?;
    let sign = sign_byte != 0;
    let digits_slice = reader.read_bytes(n as usize)?;
    Ok(Term::Big {
        sign,
        digits: digits_slice.to_vec(),
    })
}

fn decode_large_big(reader: &mut Reader, tag_offset: usize) -> Result<Term, DecodeError> {
    let n = reader.read_u32_be()?;
    if n > reader.cfg.big_digit_ceiling {
        return Err(DecodeError::BigDigitCeilingExceeded {
            declared: n,
            ceiling: reader.cfg.big_digit_ceiling,
            offset: tag_offset,
        });
    }
    let needed = 1usize.saturating_add(n as usize);
    reader.require(needed)?;
    let sign_byte = reader.read_u8()?;
    let sign = sign_byte != 0;
    let digits_slice = reader.read_bytes(n as usize)?;
    Ok(Term::Big {
        sign,
        digits: digits_slice.to_vec(),
    })
}

fn decode_map(reader: &mut Reader, depth: u32) -> Result<Term, DecodeError> {
    let arity = reader.read_u32_be()? as usize;
    let cap = arity.min(reader.remaining() / 2);
    let mut entries = Vec::with_capacity(cap);
    for _ in 0..arity {
        let k = decode_term(reader, depth + 1)?;
        let v = decode_term(reader, depth + 1)?;
        entries.push((k, v));
    }
    Ok(Term::Map(entries))
}
